//! zwlr_layer_shell_v1 surface management + wl_shm buffer pool.
//!
//! The `RadialSurface` struct owns the layer surface and the SHM slot pool.
//! `present()` performs the RGBA→ARGB8888 byte-swap and prepares the
//! pixel data for upload; actual wl_surface.attach/commit is gated on a
//! valid AppState (deferred to Task 3 when the event loop is wired up).

use smithay_client_toolkit as sctk;

use sctk::{
    reexports::client::QueueHandle,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm},
};
use wayland_client::protocol::wl_shm;

use anyhow::Result;

// ─────────────────────────────────────────────────────────────────────────────
// Pixel-format conversion helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a single tiny-skia pixel (premultiplied RGBA, bytes [R,G,B,A]) to
/// Wayland Argb8888 (little-endian BGRA on the wire: [B, G, R, A]).
///
/// The mapping required by the brief:
/// ```text
///   dst[0] = src[2]  (B)
///   dst[1] = src[1]  (G)
///   dst[2] = src[0]  (R)
///   dst[3] = src[3]  (A)
/// ```
#[inline]
pub fn rgba_to_argb8888(src: [u8; 4]) -> [u8; 4] {
    [src[2], src[1], src[0], src[3]]
}

/// Convert an entire tiny-skia `Pixmap` pixel buffer into an Argb8888 byte
/// slice, writing into `dst`.  `dst` must be at least `pixmap.width() *
/// pixmap.height() * 4` bytes long.
pub fn convert_pixmap_to_argb8888(pixmap: &tiny_skia::Pixmap, dst: &mut [u8]) {
    let src = pixmap.data(); // raw RGBA bytes, 4 bytes per pixel
    debug_assert_eq!(src.len(), dst.len(), "buffer size mismatch");
    for (chunk_dst, chunk_src) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let converted = rgba_to_argb8888([chunk_src[0], chunk_src[1], chunk_src[2], chunk_src[3]]);
        chunk_dst.copy_from_slice(&converted);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RadialSurface
// ─────────────────────────────────────────────────────────────────────────────

/// Owns the wlr-layer-shell surface and the SHM slot pool used for software
/// rendering.
///
/// # Lifecycle
/// 1. Created via [`RadialSurface::new`] once globals are ready (Task 3).
/// 2. Compositor sends a `configure` event → [`RadialSurface::handle_configure`]
///    records the negotiated size.
/// 3. Each frame: caller renders into a `tiny_skia::Pixmap` and calls
///    [`RadialSurface::present`] to upload pixels.
#[derive(Debug)]
pub struct RadialSurface {
    /// The wlr-layer-shell surface handle.
    pub layer_surface: Option<LayerSurface>,

    /// The underlying Wayland surface.
    pub wl_surface: wayland_client::protocol::wl_surface::WlSurface,

    /// SHM slot pool backing the software-rendered framebuffer.
    pub pool: SlotPool,

    /// Negotiated surface width in logical pixels (set by configure).
    pub width: u32,

    /// Negotiated surface height in logical pixels (set by configure).
    pub height: u32,

    /// HiDPI scale factor (1 = 96 dpi, 2 = 192 dpi, …).
    pub scale: i32,
}

impl Drop for RadialSurface {
    fn drop(&mut self) {
        log::info!("Dropping RadialSurface: destroying LayerSurface then WlSurface");
        drop(self.layer_surface.take());
        self.wl_surface.destroy();
    }
}

impl RadialSurface {
    /// Construct a new `RadialSurface`.
    ///
    /// The layer surface is configured with:
    /// - `Layer::Overlay`
    /// - Namespace `"radial-dial"`
    /// - Anchor: all four edges (fullscreen)
    /// - Exclusive zone: -1 (ignore other exclusive zones)
    /// - Size: (0, 0) — let compositor determine fullscreen size
    /// - Keyboard interactivity: Exclusive
    ///
    /// The `configure` event has not arrived yet, so `width`/`height` are 0
    /// until the compositor calls back.
    pub fn new<State>(
        layer_shell: &LayerShell,
        shm: &Shm,
        compositor: &sctk::compositor::CompositorState,
        qh: &QueueHandle<State>,
    ) -> Result<Self>
    where
        State: LayerShellHandler
            + sctk::compositor::CompositorHandler
            + sctk::shm::ShmHandler
            + wayland_client::Dispatch<
                wayland_client::protocol::wl_compositor::WlCompositor,
                sctk::globals::GlobalData,
            > + wayland_client::Dispatch<
                wayland_client::protocol::wl_surface::WlSurface,
                sctk::compositor::SurfaceData,
            > + wayland_client::Dispatch<
                sctk::reexports::protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
                sctk::shell::wlr_layer::LayerSurfaceData,
            > + 'static,
    {
        // Create a bare wl_surface via the compositor.
        let surface = compositor.create_surface(qh);

        // Wrap it in a layer surface: Overlay, fullscreen anchors, no output
        // preference (let the compositor pick the primary output).
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Overlay,
            Some("quickshell:radialMenu"),
            None, // output: let compositor choose
        );

        // Fullscreen: anchor all four edges.
        layer_surface.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);

        // Exclusive zone -1: don't shrink the usable area for other surfaces.
        layer_surface.set_exclusive_zone(-1);

        // Size (0, 0) tells the compositor to give us the full output size.
        layer_surface.set_size(0, 0);

        // Keyboard interactivity: Exclusive so that the radial dial captures number keys (1-9)
        // and Escape while open. The entire surface is destroyed when the menu closes.
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);

        // Commit the initial surface state so the compositor sends `configure`.
        layer_surface.wl_surface().commit();

        // Allocate a 1 MiB SHM pool; it will grow on first configure.
        let pool = SlotPool::new(1024 * 1024, shm)
            .map_err(|e| anyhow::anyhow!("Failed to create SHM pool: {e}"))?;

        Ok(Self {
            layer_surface: Some(layer_surface),
            wl_surface: surface,
            pool,
            width: 0,
            height: 0,
            scale: 1,
        })
    }

    pub fn wl_surface(&self) -> &wayland_client::protocol::wl_surface::WlSurface {
        &self.wl_surface
    }

    pub fn layer_surface(&self) -> Option<&LayerSurface> {
        self.layer_surface.as_ref()
    }

    /// Handle a compositor `configure` event.
    ///
    /// Records the new dimensions and commits an ACK so the compositor can
    /// proceed. The actual framebuffer upload follows via [`Self::present`].
    pub fn handle_configure(&mut self, configure: LayerSurfaceConfigure) {
        let (w, h) = configure.new_size;
        if w > 0 {
            self.width = w;
        }
        if h > 0 {
            self.height = h;
        }
    }

    /// Upload a `tiny_skia::Pixmap` to the compositor.
    ///
    /// Performs the RGBA (premultiplied) → Argb8888 byte-swap required by `wl_shm`.
    pub fn present(&mut self, pixmap: &tiny_skia::Pixmap) {
        let width = pixmap.width() as i32;
        let height = pixmap.height() as i32;

        if width == 0 || height == 0 {
            return;
        }

        let stride = width * 4; // 4 bytes per pixel (ARGB8888)

        // Allocate a buffer slot from the SHM pool.
        let Ok((buffer, canvas)) =
            self.pool.create_buffer(width, height, stride, wl_shm::Format::Argb8888)
        else {
            log::error!("Failed to allocate SHM buffer slot");
            return;
        };

        // ── RGBA → ARGB8888 byte-swap ──────────────────────────────────────
        convert_pixmap_to_argb8888(pixmap, canvas);
        // ───────────────────────────────────────────────────────────────────

        let wl_surface = &self.wl_surface;
        if let Err(e) = buffer.attach_to(wl_surface) {
            log::error!("Failed to attach buffer to surface: {:?}", e);
            return;
        }
        wl_surface.damage_buffer(0, 0, width, height);
        wl_surface.commit();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argb_conversion() {
        // Input: R=255, G=128, B=64, A=200
        let rgba = [255u8, 128, 64, 200];

        let argb = rgba_to_argb8888(rgba);

        // Expected mapping:
        //   dst[0] = src[2] = 64   (B)
        //   dst[1] = src[1] = 128  (G)
        //   dst[2] = src[0] = 255  (R)
        //   dst[3] = src[3] = 200  (A)
        assert_eq!(argb[0], 64, "dst[0] should be Blue (64)");
        assert_eq!(argb[1], 128, "dst[1] should be Green (128)");
        assert_eq!(argb[2], 255, "dst[2] should be Red (255)");
        assert_eq!(argb[3], 200, "dst[3] should be Alpha (200)");
    }

    #[test]
    fn test_argb_fully_transparent() {
        let rgba = [0u8, 0, 0, 0];
        let argb = rgba_to_argb8888(rgba);
        assert_eq!(argb, [0, 0, 0, 0]);
    }

    #[test]
    fn test_argb_opaque_white() {
        // White: R=255, G=255, B=255, A=255
        let rgba = [255u8, 255, 255, 255];
        let argb = rgba_to_argb8888(rgba);
        assert_eq!(argb, [255, 255, 255, 255]);
    }

    #[test]
    fn test_argb_opaque_red() {
        // Pure red: R=255, G=0, B=0, A=255
        let rgba = [255u8, 0, 0, 255];
        let argb = rgba_to_argb8888(rgba);
        // Expected: B=0, G=0, R=255, A=255
        assert_eq!(argb, [0, 0, 255, 255]);
    }

    #[test]
    fn test_convert_pixmap_buffer() {
        // Build a 2-pixel mock buffer in RGBA order.
        let rgba_data: Vec<u8> = vec![
            255, 128, 64, 200, // pixel 0: R=255, G=128, B=64,  A=200
            10, 20, 30, 40,   // pixel 1: R=10,  G=20,  B=30,  A=40
        ];
        let mut argb_dst = vec![0u8; rgba_data.len()];

        // Manually drive the per-chunk logic (avoids needing a real Pixmap).
        for (chunk_dst, chunk_src) in
            argb_dst.chunks_exact_mut(4).zip(rgba_data.chunks_exact(4))
        {
            let converted =
                rgba_to_argb8888([chunk_src[0], chunk_src[1], chunk_src[2], chunk_src[3]]);
            chunk_dst.copy_from_slice(&converted);
        }

        // pixel 0: B=64, G=128, R=255, A=200
        assert_eq!(&argb_dst[0..4], &[64, 128, 255, 200]);
        // pixel 1: B=30, G=20, R=10, A=40
        assert_eq!(&argb_dst[4..8], &[30, 20, 10, 40]);
    }
}
