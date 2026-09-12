//! radial-dial — Native Wayland radial menu daemon for Hyprland.
//!
//! Replaces the QML + Python radialMenu with a high-performance, low-memory
//! (~8-12 MB) standalone Rust daemon.

pub mod font;
pub mod ipc;
pub mod renderer;
pub mod state;
pub mod wayland;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use smithay_client_toolkit as sctk;
use sctk::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler as SctkSeatHandler, SeatState,
    },
    shell::wlr_layer::{
        LayerShell, LayerShellHandler, LayerSurface,
        LayerSurfaceConfigure,
    },
    shm::{Shm, ShmHandler},
};
use sctk::reexports::client::{
    protocol::{
        wl_keyboard::WlKeyboard,
        wl_output::{Transform, WlOutput},
        wl_pointer::WlPointer,
        wl_seat::WlSeat,
        wl_surface::WlSurface,
    },
    Connection, QueueHandle,
};
use tiny_skia::{
    Color, GradientStop, Paint, Pixmap, Point, RadialGradient, SpreadMode, Stroke,
    Transform as SkTransform,
};
use tokio::sync::watch;
use wayland_client::globals::registry_queue_init;

use font::FontRenderer;
use ipc::hypr::HyprContext;
use renderer::customizer::CustomizerState;
use renderer::folder_browser::FolderBrowserState;
use renderer::hub::draw_center_hub;
use renderer::pie::{
    build_main_ring_paths_animated, get_slice_displacement, ICON_RADIUS, SLICE_OUTER_R,
    TOTAL_RADIUS,
};
use renderer::subring::draw_sub_ring_with_icons;
use renderer::text::{draw_icon, draw_number_badge};
use state::actions::execute as execute_action;
use state::anim::Easing;
use state::config::RadialConfig;
use state::gpu::{detect_gpu_profile, GpuProfile};
use state::menu::{MenuPhase, MenuState};
use wayland::layer_surface::RadialSurface;
use wayland::seat::{ButtonState, KeyState, SeatHandler};

const SOCKET_PATH: &str = "/tmp/radial-dial.sock";

// ─────────────────────────────────────────────────────────────────────────────
// App
// ─────────────────────────────────────────────────────────────────────────────

pub struct App {
    // SCT states
    pub registry_state: RegistryState,
    pub compositor_state: CompositorState,
    pub layer_shell: LayerShell,
    pub shm: Shm,
    pub seat_state: SeatState,
    pub output_state: OutputState,

    // Surface & Input
    pub surface: Option<RadialSurface>,
    pub seat_handler: SeatHandler,
    pub font_renderer: FontRenderer,

    // Dial State
    pub menu: MenuState,
    pub customizer: Option<CustomizerState>,
    pub folder_browser: Option<FolderBrowserState>,

    // Wayland inputs
    pub pointer: Option<WlPointer>,
    pub keyboard: Option<WlKeyboard>,

    // Primary output
    pub primary_output: Option<WlOutput>,

    // Surface configuration status
    pub surface_configured: bool,
    pub pending_open_ctx: Option<HyprContext>,

    // Frame & Rendering throttle
    pub dirty: bool,
    pub waiting_for_frame: bool,
    pub render_pixmap: Option<Pixmap>,

    // Animation runner state
    pub last_frame_time: std::time::Instant,
}

sctk::delegate_compositor!(App);
sctk::delegate_output!(App);
sctk::delegate_shm!(App);
sctk::delegate_seat!(App);
sctk::delegate_layer!(App);
sctk::delegate_registry!(App);
sctk::delegate_pointer!(App);
sctk::delegate_keyboard!(App);

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    sctk::registry_handlers![OutputState, SeatState];
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        new_factor: i32,
    ) {
        if let Some(surf) = &mut self.surface {
            surf.scale = new_factor;
        }
        self.dirty = true;
    }
    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_transform: Transform,
    ) {}
    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _time: u32,
    ) {
        self.waiting_for_frame = false;
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        if self.primary_output.is_none() {
            self.primary_output = Some(output);
        }
    }
    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {}
    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        if self.primary_output.as_ref() == Some(&output) {
            self.primary_output = None;
        }
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SctkSeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {}
    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self.seat_state.get_pointer(qh, &seat).ok();
            self.pointer = pointer;
        }
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            let keyboard = self
                .seat_state
                .get_keyboard(qh, &seat, None)
                .ok();
            self.keyboard = keyboard;
        }
    }
    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            self.pointer = None;
        }
        if capability == Capability::Keyboard {
            self.keyboard = None;
        }
    }
    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {}
}

impl LayerShellHandler for App {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.surface = None;
        self.surface_configured = false;
        self.pending_open_ctx = None;
        self.menu.phase = MenuPhase::Hidden;
        self.dirty = false;
    }
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        log::info!("Layer configure received from Hyprland: new_size = {:?}", configure.new_size);
        if let Some(surf) = &mut self.surface {
            surf.handle_configure(configure);
            self.surface_configured = true;
            if let Some(ctx) = self.pending_open_ctx.take() {
                self.menu.transition_open(ctx);
                self.dirty = true;
                self.waiting_for_frame = false;
            }
        }
    }
}

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        if self.menu.phase == MenuPhase::Hidden || self.surface.is_none() || !self.surface_configured {
            return;
        }
        for event in events {
            self.dirty = true;
            match event.kind {
                PointerEventKind::Motion { .. } => {
                    let (x, y) = event.position;
                    self.seat_handler.handle_pointer_motion(&mut self.menu, x as f32, y as f32);
                }
                PointerEventKind::Press { button, .. } => {
                    if let Some(action) = self.seat_handler.handle_pointer_button(
                        &mut self.menu,
                        button,
                        ButtonState::Pressed,
                    ) {
                        self.menu.transition_close_animated(Some(action));
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if let Some(action) = self.seat_handler.handle_pointer_button(
                        &mut self.menu,
                        button,
                        ButtonState::Released,
                    ) {
                        self.menu.transition_close_animated(Some(action));
                    }
                }
                PointerEventKind::Axis { vertical, .. } => {
                    if let Some(action) = self
                        .seat_handler
                        .handle_pointer_axis(&mut self.menu, vertical.absolute)
                    {
                        execute_action(&action);
                    }
                }
                _ => {}
            }
        }
    }
}

impl KeyboardHandler for App {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }
    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
    ) {
    }
    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if self.menu.phase == MenuPhase::Hidden || self.surface.is_none() || !self.surface_configured {
            return;
        }
        self.dirty = true;
        let keysym = event.keysym.raw();
        if let Some(action) =
            self.seat_handler.handle_key_event(&mut self.menu, keysym, KeyState::Pressed)
        {
            self.menu.transition_close_animated(Some(action));
        }
    }
    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if self.menu.phase == MenuPhase::Hidden || self.surface.is_none() || !self.surface_configured {
            return;
        }
        self.dirty = true;
        let keysym = event.keysym.raw();
        if let Some(action) =
            self.seat_handler.handle_key_event(&mut self.menu, keysym, KeyState::Released)
        {
            self.menu.transition_close_animated(Some(action));
        }
    }
    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
    ) {
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// App Implementation
// ─────────────────────────────────────────────────────────────────────────────

impl App {
    pub fn new(conn: &Connection, qh: &QueueHandle<Self>) -> Result<Self> {
        let (globals, _) = registry_queue_init::<Self>(conn)?;

        let registry_state = RegistryState::new(&globals);
        let compositor_state = CompositorState::bind(&globals, qh)?;
        let layer_shell = LayerShell::bind(&globals, qh)?;
        let shm = Shm::bind(&globals, qh)?;
        let seat_state = SeatState::new(&globals, qh);
        let output_state = OutputState::new(&globals, qh);

        let config = RadialConfig::load();
        let is_low_end = detect_gpu_profile(&config) == GpuProfile::LowEnd;
        let menu = MenuState::new(config, is_low_end);
        let font_renderer = FontRenderer::new();

        Ok(Self {
            registry_state,
            compositor_state,
            layer_shell,
            shm,
            seat_state,
            output_state,
            surface: None,
            seat_handler: SeatHandler::new(),
            font_renderer,
            menu,
            customizer: None,
            folder_browser: None,
            pointer: None,
            keyboard: None,
            primary_output: None,
            surface_configured: false,
            pending_open_ctx: None,
            dirty: false,
            waiting_for_frame: false,
            render_pixmap: None,
            last_frame_time: std::time::Instant::now(),
        })
    }

    /// Open or map radial surface and transition to Opening phase
    pub fn open_menu(&mut self, ctx: HyprContext, qh: &QueueHandle<Self>) -> Result<()> {
        if self.surface.is_none() {
            let surface = RadialSurface::new(
                &self.layer_shell,
                &self.shm,
                &self.compositor_state,
                qh,
            )?;
            self.surface = Some(surface);
            self.surface_configured = false;
            self.pending_open_ctx = Some(ctx);
        } else if self.surface_configured {
            self.menu.transition_open(ctx);
            self.dirty = true;
            self.waiting_for_frame = false;
        } else {
            self.pending_open_ctx = Some(ctx);
        }

        Ok(())
    }

    /// Render current dial state into the reusable persistent Pixmap.
    /// Returns true if rendering succeeded.
    pub fn render(&mut self) -> bool {
        let (w, h) = if let Some(surf) = &self.surface {
            (surf.width.max(544), surf.height.max(544))
        } else {
            (1920, 1080)
        };
        log::info!("render() called: w = {}, h = {}, phase = {:?}", w, h, self.menu.phase);

        // Reuse existing Pixmap if dimensions match, avoiding any allocations per frame
        let need_realloc = match &self.render_pixmap {
            Some(pm) => pm.width() != w || pm.height() != h,
            None => true,
        };
        if need_realloc {
            self.render_pixmap = Pixmap::new(w, h);
        }

        let pixmap = match &mut self.render_pixmap {
            Some(pm) => pm,
            None => return false,
        };

        pixmap.fill(Color::TRANSPARENT);

        if self.menu.phase == MenuPhase::Hidden {
            return true;
        }

        let mut cx = self.menu.center_x;
        let mut cy = self.menu.center_y;
        if cx <= 0.0 && cy <= 0.0 {
            cx = w as f32 / 2.0;
            cy = h as f32 / 2.0;
        }
        let cx = cx.clamp(TOTAL_RADIUS, w as f32 - TOTAL_RADIUS);
        let cy = cy.clamp(TOTAL_RADIUS, h as f32 - TOTAL_RADIUS);
        self.menu.center_x = cx;
        self.menu.center_y = cy;

        let primary_hex = self.menu.config.colors.as_ref().map(|c| c.primary_hex()).unwrap_or("#cbc4cb");
        let on_primary_hex = self.menu.config.colors.as_ref().map(|c| c.on_primary_hex()).unwrap_or("#322f34");

        let primary_col = parse_hex_color(primary_hex);
        let on_primary_col = parse_hex_color(on_primary_hex);

        let slice_count = self.menu.current_slices.len();
        if slice_count == 0 {
            return true;
        }

        let hovered = self.menu.hovered_index;
        let hover_factor = self.menu.anim.hover_factor;

        // 1. Draw Main Ring Wedges (with blossom animation & frosted glass styling)
        let animated_wedges = build_main_ring_paths_animated(
            cx,
            cy,
            slice_count,
            hovered,
            hover_factor,
            self.menu.anim.reveal_progress,
        );

        for (i, path, _p) in animated_wedges {
            let is_hovered = i as i32 == hovered;
            let is_parent_of_sub = i as i32 == self.menu.parent_slice_index && self.menu.active_sub_tier.is_some();
            let mut paint = Paint::default();
            paint.anti_alias = true;

            if is_hovered || is_parent_of_sub {
                let light_col = lighten_color(primary_col, 1.35);
                let dark_col = darken_color(primary_col, 1.15);

                let grad = RadialGradient::new(
                    Point::from_xy(cx, cy),
                    Point::from_xy(cx, cy),
                    SLICE_OUTER_R + 10.0,
                    vec![
                        GradientStop::new(0.0, light_col),
                        GradientStop::new(0.35, primary_col),
                        GradientStop::new(1.0, dark_col),
                    ],
                    SpreadMode::Pad,
                    SkTransform::identity(),
                );
                if let Some(shader) = grad {
                    paint.shader = shader;
                } else {
                    paint.set_color(primary_col);
                }
                pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, SkTransform::identity(), None);

                let mut stroke_paint = Paint::default();
                stroke_paint.set_color(light_col);
                stroke_paint.anti_alias = true;
                let stroke = Stroke { width: 1.6, ..Default::default() };
                pixmap.stroke_path(&path, &stroke_paint, &stroke, SkTransform::identity(), None);
            } else {
                // Neutral frosted charcoal gradient matching QML:
                // Inner: (0.07, 0.07, 0.08, 0.44), Outer: (0.04, 0.04, 0.05, 0.34)
                let (stop0, stop1) = if self.menu.is_low_end_gpu {
                    (
                        Color::from_rgba(0.10, 0.10, 0.12, 0.88).unwrap_or(Color::BLACK),
                        Color::from_rgba(0.06, 0.06, 0.08, 0.82).unwrap_or(Color::BLACK),
                    )
                } else {
                    (
                        Color::from_rgba(0.07, 0.07, 0.08, 0.44).unwrap_or(Color::BLACK),
                        Color::from_rgba(0.04, 0.04, 0.05, 0.34).unwrap_or(Color::BLACK),
                    )
                };

                let grad = RadialGradient::new(
                    Point::from_xy(cx, cy),
                    Point::from_xy(cx, cy),
                    SLICE_OUTER_R,
                    vec![
                        GradientStop::new(0.0, stop0),
                        GradientStop::new(1.0, stop1),
                    ],
                    SpreadMode::Pad,
                    SkTransform::identity(),
                );
                if let Some(shader) = grad {
                    paint.shader = shader;
                } else {
                    paint.set_color(stop0);
                }
                pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, SkTransform::identity(), None);

                // Subtle white sheen
                let mut sheen_paint = Paint::default();
                sheen_paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.04).unwrap_or(Color::WHITE));
                sheen_paint.anti_alias = true;
                pixmap.fill_path(&path, &sheen_paint, tiny_skia::FillRule::Winding, SkTransform::identity(), None);

                // Crisp soft white translucent border
                let mut stroke_paint = Paint::default();
                stroke_paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.22).unwrap_or(Color::WHITE));
                stroke_paint.anti_alias = true;
                let stroke = Stroke { width: 1.0, ..Default::default() };
                pixmap.stroke_path(&path, &stroke_paint, &stroke, SkTransform::identity(), None);
            }
        }

        // 2. Draw Main Ring Icons & Number Badges
        let slice_angle = 360.0 / slice_count as f32;
        for (i, slice) in self.menu.current_slices.iter().enumerate() {
            let p = (self.menu.anim.reveal_progress - i as f32).clamp(0.0, 1.0);
            if p <= 0.01 {
                continue;
            }
            let ease = if p >= 1.0 { 1.0 } else { 1.0 - (1.0 - p).powi(3) };

            let disp = get_slice_displacement(i as i32, slice_count as i32, hovered, hover_factor);
            let base_start = i as f32 * slice_angle - 90.0;
            let base_end = (i as f32 + 1.0) * slice_angle - 90.0;

            let cur_start = base_start + disp.start_shift;
            let cur_end = base_end + disp.end_shift;
            let mid_rad = ((cur_start + cur_end) / 2.0).to_radians();

            let base_icon_r = ICON_RADIUS * ease;
            let icon_r = base_icon_r + disp.r_shift / 2.0;
            let icon_x = cx + icon_r * mid_rad.cos();
            let icon_y = cy + icon_r * mid_rad.sin();

            let is_hov = i as i32 == hovered || (i as i32 == self.menu.parent_slice_index && self.menu.active_sub_tier.is_some());
            let icon_base_col = if is_hov { on_primary_col } else { Color::from_rgba8(255, 255, 255, 240) };
            let icon_col = Color::from_rgba(
                icon_base_col.red(),
                icon_base_col.green(),
                icon_base_col.blue(),
                icon_base_col.alpha() * p,
            ).unwrap_or(icon_base_col);

            let icon_size = (if is_hov { 28.0 } else { 24.0 }) * ease;
            draw_icon(&mut self.font_renderer, pixmap, &slice.icon, icon_x, icon_y, icon_size, icon_col);

            if i < 9 && p >= 0.5 {
                let badge_x = icon_x + 12.0;
                let badge_y = icon_y - 12.0;
                draw_number_badge(&mut self.font_renderer, pixmap, i + 1, badge_x, badge_y, is_hov, primary_col, on_primary_col);
            }
        }

        // 3. Draw Sub-Ring if active
        if self.menu.active_sub_tier.is_some() && !self.menu.sub_slices.is_empty() {
            let sub_start_deg = self.menu.sub_start_angle();
            let sub_width_deg = self.menu.sub_slice_width();
            draw_sub_ring_with_icons(
                &mut self.font_renderer,
                pixmap,
                cx,
                cy,
                &self.menu.sub_slices,
                sub_start_deg,
                sub_width_deg,
                self.menu.outer_hovered_index,
                self.menu.anim.outer_hover_factor,
                self.menu.anim.sub_reveal_progress,
                primary_col,
                on_primary_col,
                self.menu.is_low_end_gpu,
            );
        }

        // 4. Draw Center Hub
        let hub_label = self.menu.active_hover_label();
        draw_center_hub(
            &mut self.font_renderer,
            pixmap,
            cx,
            cy,
            44.0,
            &hub_label,
            self.menu.center_hovered,
            self.menu.anim.hub_scale,
            primary_col,
            self.menu.is_low_end_gpu,
        );

        // 5. Draw Customizer modal if active
        if let Some(customizer_state) = &self.customizer {
            let cat = state::actions::function_catalogue();
            renderer::customizer::render_customizer_with_font(
                customizer_state,
                &cat,
                pixmap,
                w as f32,
                h as f32,
                &mut self.font_renderer,
            );
        }

        // 6. Draw Folder Browser modal if active
        if let Some(folder_state) = &self.folder_browser {
            renderer::folder_browser::render_folder_browser_with_font(
                folder_state,
                pixmap,
                w as f32,
                h as f32,
                &mut self.font_renderer,
            );
        }

        true
    }

    /// Step animations and transition lifecycle. Sets `dirty = true` when frames change.
    pub fn step_frame(&mut self, dt_ms: u64) {
        if self.menu.phase == MenuPhase::Hidden {
            return;
        }
        let dt = dt_ms as f32;

        match self.menu.phase {
            MenuPhase::Opening => {
                self.dirty = true;
                self.menu.anim.opening_elapsed += dt;

                let hub_dur = if self.menu.is_low_end_gpu { 50.0 } else { 150.0 };
                let slice_count = self.menu.current_slices.len().max(1) as f32;
                let rev_dur = if self.menu.is_low_end_gpu { 70.0 } else { (slice_count * 45.0).max(220.0) };

                // 1. Hub pops up first with OutBack(1.3)
                let t_hub = (self.menu.anim.opening_elapsed / hub_dur).clamp(0.0, 1.0);
                self.menu.anim.hub_scale = if self.menu.is_low_end_gpu {
                    t_hub
                } else {
                    Easing::OutBack(1.3).value(t_hub)
                };

                // 2. Main ring slices blossom out one-by-one clockwise with OutCubic
                if self.menu.anim.opening_elapsed >= hub_dur {
                    let rev_elapsed = self.menu.anim.opening_elapsed - hub_dur;
                    let t_rev = (rev_elapsed / rev_dur).clamp(0.0, 1.0);
                    let ease_rev = Easing::OutCubic.value(t_rev);
                    self.menu.anim.reveal_progress = ease_rev * slice_count;
                } else {
                    self.menu.anim.reveal_progress = 0.0;
                }

                if self.menu.anim.opening_elapsed >= (hub_dur + rev_dur) {
                    self.menu.anim.hub_scale = 1.0;
                    self.menu.anim.reveal_progress = slice_count;
                    self.menu.phase = MenuPhase::Open;
                }
            }
            MenuPhase::Open => {
                // Main ring hover spring (OutBack 1.25 ripple)
                if self.menu.hovered_index >= 0 {
                    self.menu.anim.hover_elapsed = (self.menu.anim.hover_elapsed + dt).min(160.0);
                    let t = (self.menu.anim.hover_elapsed / 160.0).clamp(0.0, 1.0);
                    let next = if self.menu.is_low_end_gpu { 1.0 } else { Easing::OutBack(1.25).value(t) };
                    if (next - self.menu.anim.hover_factor).abs() > 0.002 {
                        self.menu.anim.hover_factor = next;
                        self.dirty = true;
                    }
                } else if self.menu.anim.hover_factor > 0.002 {
                    self.menu.anim.hover_elapsed = (self.menu.anim.hover_elapsed + dt).min(120.0);
                    let t = (self.menu.anim.hover_elapsed / 120.0).clamp(0.0, 1.0);
                    let fade_start = if self.menu.anim.hover_fade_start > 0.001 { self.menu.anim.hover_fade_start } else { 1.0 };
                    let next = (fade_start * (1.0 - Easing::OutQuad.value(t))).max(0.0);
                    if (next - self.menu.anim.hover_factor).abs() > 0.002 {
                        self.menu.anim.hover_factor = next;
                        self.dirty = true;
                    }
                } else {
                    self.menu.anim.hover_factor = 0.0;
                }

                // Outer sub-ring hover spring (OutBack 1.2 ripple)
                if self.menu.outer_hovered_index >= 0 {
                    self.menu.anim.outer_hover_elapsed = (self.menu.anim.outer_hover_elapsed + dt).min(140.0);
                    let t = (self.menu.anim.outer_hover_elapsed / 140.0).clamp(0.0, 1.0);
                    let next = if self.menu.is_low_end_gpu { 1.0 } else { Easing::OutBack(1.2).value(t) };
                    if (next - self.menu.anim.outer_hover_factor).abs() > 0.002 {
                        self.menu.anim.outer_hover_factor = next;
                        self.dirty = true;
                    }
                } else if self.menu.anim.outer_hover_factor > 0.002 {
                    self.menu.anim.outer_hover_elapsed = (self.menu.anim.outer_hover_elapsed + dt).min(100.0);
                    let t = (self.menu.anim.outer_hover_elapsed / 100.0).clamp(0.0, 1.0);
                    let fade_start = if self.menu.anim.outer_hover_fade_start > 0.001 { self.menu.anim.outer_hover_fade_start } else { 1.0 };
                    let next = (fade_start * (1.0 - Easing::OutQuad.value(t))).max(0.0);
                    if (next - self.menu.anim.outer_hover_factor).abs() > 0.002 {
                        self.menu.anim.outer_hover_factor = next;
                        self.dirty = true;
                    }
                } else {
                    self.menu.anim.outer_hover_factor = 0.0;
                }

                // Sub-tier staggered reveal
                if self.menu.active_sub_tier.is_some() && !self.menu.sub_slices.is_empty() {
                    let sub_count = self.menu.sub_slices.len() as f32;
                    let sub_dur = if self.menu.is_low_end_gpu { 80.0 } else { (sub_count * 40.0).max(180.0) };
                    self.menu.anim.sub_elapsed = (self.menu.anim.sub_elapsed + dt).min(sub_dur);
                    let t = (self.menu.anim.sub_elapsed / sub_dur).clamp(0.0, 1.0);
                    let next = Easing::OutCubic.value(t) * sub_count;
                    if (next - self.menu.anim.sub_reveal_progress).abs() > 0.002 {
                        self.menu.anim.sub_reveal_progress = next;
                        self.dirty = true;
                    }
                } else if self.menu.anim.sub_reveal_progress > 0.0 {
                    self.menu.anim.sub_reveal_progress = 0.0;
                    self.dirty = true;
                }
            }
            MenuPhase::ClosingAnimated { ref pending_action } => {
                self.dirty = true;
                self.menu.anim.closing_elapsed += dt;

                let collapse_dur = if self.menu.is_low_end_gpu { 40.0 } else { 90.0 };
                let hub_dur = if self.menu.is_low_end_gpu { 20.0 } else { 50.0 };
                let total_dur = collapse_dur + hub_dur;

                // 1. Slices collapse inward (InCubic)
                let t_collapse = (self.menu.anim.closing_elapsed / collapse_dur).clamp(0.0, 1.0);
                let ease_collapse = 1.0 - Easing::InCubic.value(t_collapse);
                let slice_count = self.menu.current_slices.len() as f32;
                self.menu.anim.reveal_progress = ease_collapse * slice_count;
                self.menu.anim.sub_reveal_progress = ease_collapse * self.menu.sub_slices.len() as f32;

                // 2. Hub pops down (InQuad)
                if self.menu.anim.closing_elapsed >= collapse_dur {
                    let hub_elapsed = self.menu.anim.closing_elapsed - collapse_dur;
                    let t_hub = (hub_elapsed / hub_dur).clamp(0.0, 1.0);
                    self.menu.anim.hub_scale = (1.0 - Easing::InQuad.value(t_hub)).max(0.0);
                } else {
                    self.menu.anim.hub_scale = 1.0;
                }

                if self.menu.anim.closing_elapsed >= total_dur {
                    let action_opt = pending_action.clone();
                    self.menu.phase = MenuPhase::Hidden;
                    self.dirty = false;
                    self.waiting_for_frame = false;
                    self.surface = None;
                    self.surface_configured = false;
                    self.pending_open_ctx = None;
                    if let Some(action) = action_opt {
                        execute_action(&action);
                    }
                }
            }
            _ => {}
        }
    }
}

fn lighten_color(c: Color, factor: f32) -> Color {
    let r = (c.red() * factor).min(1.0);
    let g = (c.green() * factor).min(1.0);
    let b = (c.blue() * factor).min(1.0);
    Color::from_rgba(r, g, b, c.alpha()).unwrap_or(c)
}

fn darken_color(c: Color, factor: f32) -> Color {
    let r = c.red() / factor;
    let g = c.green() / factor;
    let b = c.blue() / factor;
    Color::from_rgba(r, g, b, c.alpha()).unwrap_or(c)
}

fn parse_hex_color(hex: &str) -> Color {
    let clean = hex.trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(203);
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(196);
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(203);
        Color::from_rgba8(r, g, b, 255)
    } else {
        Color::from_rgba8(203, 196, 203, 255)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Entrypoint
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let cmd = &args[1];
        match cmd.as_str() {
            "open" | "close" | "toggle" => {
                use tokio::io::AsyncWriteExt;
                let mut connected = false;
                for _ in 0..3 {
                    if let Ok(mut stream) = tokio::net::UnixStream::connect(SOCKET_PATH).await {
                        let _ = stream.write_all(cmd.as_bytes()).await;
                        let _ = stream.flush().await;
                        connected = true;
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(40)).await;
                }
                if connected {
                    return Ok(());
                }

                // If not running, start the service and retry
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "start", "radial-dial.service"])
                    .status();
                for _ in 0..10 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    if let Ok(mut stream) = tokio::net::UnixStream::connect(SOCKET_PATH).await {
                        let _ = stream.write_all(cmd.as_bytes()).await;
                        let _ = stream.flush().await;
                        return Ok(());
                    }
                }
                eprintln!("Failed to connect to radial-dial daemon on {}", SOCKET_PATH);
                std::process::exit(1);
            }
            "--help" | "-h" => {
                println!("radial-dial: Native Wayland Radial Menu Daemon\n\nUsage:\n  radial-dial          Start daemon\n  radial-dial toggle   Toggle dial visibility\n  radial-dial open     Open dial at cursor\n  radial-dial close    Close dial\n");
                return Ok(());
            }
            _ => {}
        }
    }

    env_logger::init();
    log::info!("Starting radial-dial daemon...");

    // Check Wayland display connection
    let conn = match Connection::connect_to_env() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Could not connect to Wayland display: {:?}. Running in headless/mock mode.", e);
            return run_headless_daemon().await;
        }
    };

    use std::os::fd::AsFd;
    use tokio::io::unix::AsyncFd;
    let async_fd = AsyncFd::new(conn.as_fd())?;

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let mut app = App::new(&conn, &qh)?;

    // Initial Wayland roundtrip to bind globals (surface is created on demand when menu is opened)
    event_queue.roundtrip(&mut app)?;
    let _ = conn.flush();

    // Start background tabs watcher
    let (tabs_tx, mut tabs_rx) = watch::channel(vec![]);
    tokio::spawn(ipc::tabs_shm::watch_tabs(tabs_tx));

    // UNIX Domain Socket Listener for hotkey activation
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = tokio::net::UnixListener::bind(SOCKET_PATH)?;
    log::info!("Listening on UNIX socket: {}", SOCKET_PATH);

    let running = Arc::new(AtomicBool::new(true));
    let r_sig = running.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        r_sig.store(false, Ordering::SeqCst);
    });

    let mut frame_interval = tokio::time::interval(Duration::from_millis(8));

    while running.load(Ordering::SeqCst) {
        tokio::select! {
            // Read incoming Wayland socket events from Hyprland (configure, frame callback, pointer, keys)
            Ok(mut ready) = async_fd.readable() => {
                ready.clear_ready();
                if let Some(guard) = conn.prepare_read() {
                    let _ = guard.read();
                }
                let _ = event_queue.dispatch_pending(&mut app);
            }

            // Incoming socket commands: "open", "close", "toggle"
            Ok((mut stream, _)) = listener.accept() => {
                use tokio::io::AsyncReadExt;
                let mut buf = vec![0u8; 128];
                if let Ok(n) = stream.read(&mut buf).await {
                    let cmd = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                    log::info!("Received socket command: {}", cmd);
                    match cmd.as_str() {
                        "open" => {
                            let ctx = ipc::hypr::get_context().await.unwrap_or_default();
                            let _ = app.open_menu(ctx, &qh);
                            let _ = conn.flush();
                        }
                        "close" => {
                            app.menu.transition_close_animated(None);
                            app.dirty = true;
                            let _ = conn.flush();
                        }
                        "toggle" => {
                            if app.menu.phase == MenuPhase::Hidden || matches!(app.menu.phase, MenuPhase::ClosingAnimated { .. }) {
                                let ctx = ipc::hypr::get_context().await.unwrap_or_default();
                                let _ = app.open_menu(ctx, &qh);
                                let _ = conn.flush();
                            } else {
                                app.menu.transition_close_animated(None);
                                app.dirty = true;
                                let _ = conn.flush();
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Real-time browser tab updates from WebExtension
            _ = tabs_rx.changed() => {
                app.menu.browser_tabs = tabs_rx.borrow().clone();
                app.dirty = true;
            }

            // 125 FPS Frame Step and Render
            _ = frame_interval.tick() => {
                let _ = event_queue.dispatch_pending(&mut app);

                let now = std::time::Instant::now();
                let dt = now.duration_since(app.last_frame_time).as_millis() as u64;
                app.last_frame_time = now;

                let was_not_hidden = app.menu.phase != MenuPhase::Hidden;
                app.step_frame(dt.max(1));

                if was_not_hidden && app.menu.phase == MenuPhase::Hidden {
                    log::info!("Radial menu reached Hidden phase. Flushing destroy events to Wayland compositor.");
                    let _ = conn.flush();
                }

                if app.menu.phase != MenuPhase::Hidden && app.surface_configured && app.surface.is_some() && app.dirty {
                    app.dirty = false;
                    if app.render() {
                        if let Some(surf) = &mut app.surface {
                            if let Some(pixmap) = &app.render_pixmap {
                                surf.present(pixmap);
                                let _ = conn.flush();
                            }
                        }
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_file(SOCKET_PATH);
    log::info!("radial-dial daemon shut down cleanly.");
    Ok(())
}

async fn run_headless_daemon() -> Result<()> {
    log::info!("Running mock socket loop on {}", SOCKET_PATH);
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = tokio::net::UnixListener::bind(SOCKET_PATH)?;
    let config = RadialConfig::load();
    let mut menu = MenuState::new(config, false);

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            use tokio::io::AsyncReadExt;
            let mut buf = vec![0u8; 128];
            if let Ok(n) = stream.read(&mut buf).await {
                let cmd = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                log::info!("Mock received: {}", cmd);
                if cmd == "open" {
                    menu.transition_open(HyprContext::default());
                } else if cmd == "close" {
                    menu.transition_close_animated(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod main_tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        let col = parse_hex_color("#ff0000");
        assert_eq!(col.red(), 1.0);
        assert_eq!(col.green(), 0.0);
        assert_eq!(col.blue(), 0.0);

        let default_col = parse_hex_color("invalid");
        assert_eq!(default_col.alpha(), 1.0);
    }

    #[test]
    fn test_menu_lifecycle_steps() {
        let config = RadialConfig::default();
        let mut menu = MenuState::new(config, false);
        menu.transition_open(HyprContext::default());
        assert_eq!(menu.phase, MenuPhase::Opening);

        let slice_count = menu.current_slices.len().max(1) as f32;
        let hub_dur = 150.0;
        let rev_dur = (slice_count * 45.0).max(220.0);

        for _ in 0..100 {
            menu.anim.opening_elapsed += 16.0;
            let t_hub = (menu.anim.opening_elapsed / hub_dur).clamp(0.0, 1.0);
            menu.anim.hub_scale = Easing::OutBack(1.3).value(t_hub);

            if menu.anim.opening_elapsed >= hub_dur {
                let rev_elapsed = menu.anim.opening_elapsed - hub_dur;
                let t_rev = (rev_elapsed / rev_dur).clamp(0.0, 1.0);
                menu.anim.reveal_progress = Easing::OutCubic.value(t_rev) * slice_count;
            }

            if menu.anim.opening_elapsed >= (hub_dur + rev_dur) {
                menu.anim.hub_scale = 1.0;
                menu.anim.reveal_progress = slice_count;
                menu.phase = MenuPhase::Open;
                break;
            }
        }
        assert_eq!(menu.phase, MenuPhase::Open);
    }
}
