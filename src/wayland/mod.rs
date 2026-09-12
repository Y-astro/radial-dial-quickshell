//! Wayland subsystem — module root.
//!
//! Exposes:
//! - [`layer_surface`] — zwlr_layer_shell_v1 + wl_shm buffer management
//! - [`seat`]          — wl_seat pointer/keyboard event scaffolding
//! - [`AppState`]      — top-level application state threaded through the
//!                       smithay-client-toolkit dispatch machinery

pub mod layer_surface;
pub mod seat;

pub use layer_surface::RadialSurface;

// ─────────────────────────────────────────────────────────────────────────────
// AppState
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level application state.
///
/// This struct is threaded through every `smithay-client-toolkit` dispatch
/// call.  Fields are grown incrementally as tasks are completed:
///
/// | Task | Fields added |
/// |------|--------------|
/// | 2    | `surface` |
/// | 3    | `registry_state`, `compositor`, `layer_shell`, `shm`, `seat_handler` |
/// | 4    | `input_state` |
/// | 5    | `renderer` |
#[derive(Debug, Default)]
pub struct AppState {
    /// The layer-shell surface + SHM pool.  `None` until the compositor
    /// responds to the initial `wl_registry.bind` round-trip.
    pub surface: Option<RadialSurface>,

    // ── Placeholder fields for later phases ──────────────────────────────────

    /// (Task 3) Whether the radial menu is currently open.
    pub menu_open: bool,

    /// (Task 3) Monotonic frame counter (for animation tick).
    pub frame_count: u64,

    /// (Task 4) Accumulated seat input handler.
    pub seat_handler: seat::SeatHandler,

    /// (Task 5) Path to the user's config file (resolved at startup).
    pub config_path: Option<std::path::PathBuf>,
}
