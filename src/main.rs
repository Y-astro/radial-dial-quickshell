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
        pointer::{
            CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
        },
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
use renderer::customizer::{CustomizerColors, CustomizerMode, CustomizerState};
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
use state::actions::SubTierType;
use state::menu::{MenuPhase, MenuState};
use wayland::layer_surface::RadialSurface;
use wayland::seat::{
    is_escape, is_left_button, is_right_button, keysym_to_char, ButtonState, KeyState, SeatHandler,
};

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
    pub themed_pointer: Option<ThemedPointer>,
    pub current_cursor_icon: Option<CursorIcon>,
    pub keyboard: Option<WlKeyboard>,

    // Primary output & all outputs
    pub outputs: Vec<WlOutput>,
    pub primary_output: Option<WlOutput>,
    pub current_output_name: Option<String>,

    // Surface configuration status
    pub surface_configured: bool,
    pub pending_open_ctx: Option<HyprContext>,

    // Frame & Rendering throttle
    pub dirty: bool,
    pub waiting_for_frame: bool,
    pub render_pixmap: Option<Pixmap>,

    // Modal click isolation
    pub modal_click_active: bool,

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
        if !self.outputs.contains(&output) {
            self.outputs.push(output.clone());
        }
        if self.primary_output.is_none() {
            self.primary_output = Some(output);
        }
    }
    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {}
    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        self.outputs.retain(|o| o != &output);
        if self.primary_output.as_ref() == Some(&output) {
            self.primary_output = self.outputs.first().cloned();
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
    fn new_seat(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, seat: WlSeat) {
        if self.themed_pointer.is_none() {
            let surface = self.compositor_state.create_surface(qh);
            self.themed_pointer = self.seat_state.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                surface,
                ThemeSpec::default(),
            ).ok();
            self.pointer = self.themed_pointer.as_ref().map(|tp| tp.pointer().clone());
        }
        if self.keyboard.is_none() {
            self.keyboard = self.seat_state.get_keyboard(qh, &seat, None).ok();
        }
    }
    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.themed_pointer.is_none() {
            let surface = self.compositor_state.create_surface(qh);
            let themed_pointer = self.seat_state.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                surface,
                ThemeSpec::default(),
            ).ok();
            self.pointer = themed_pointer.as_ref().map(|tp| tp.pointer().clone());
            self.themed_pointer = themed_pointer;
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
            self.themed_pointer = None;
            self.pointer = None;
            self.current_cursor_icon = None;
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
        self.current_output_name = None;
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
        conn: &Connection,
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
                PointerEventKind::Enter { .. } => {
                    if let Some(tp) = &self.themed_pointer {
                        let _ = tp.set_cursor(conn, CursorIcon::Default);
                        self.current_cursor_icon = Some(CursorIcon::Default);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.current_cursor_icon = None;
                }
                PointerEventKind::Motion { .. } => {
                    let (x, y) = event.position;
                    let desired_icon = if self.is_text_input_hovered(x as f32, y as f32) {
                        CursorIcon::Text
                    } else {
                        CursorIcon::Default
                    };
                    if self.current_cursor_icon != Some(desired_icon) {
                        if let Some(tp) = &self.themed_pointer {
                            if tp.set_cursor(conn, desired_icon).is_ok() {
                                self.current_cursor_icon = Some(desired_icon);
                            }
                        }
                    }
                    if self.customizer.is_some() || self.folder_browser.is_some() {
                        self.seat_handler.last_x = x as f32;
                        self.seat_handler.last_y = y as f32;
                    } else {
                        self.seat_handler.handle_pointer_motion(&mut self.menu, x as f32, y as f32);
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    let (x, y) = event.position;
                    self.handle_pointer_press(button, x as f32, y as f32);
                }
                PointerEventKind::Release { button, .. } => {
                    self.handle_pointer_release(button);
                }
                PointerEventKind::Axis { vertical, .. } => {
                    self.handle_pointer_axis(vertical.absolute);
                }
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
        if self.handle_customizer_or_folder_key(keysym) {
            return;
        }
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
        if self.customizer.is_some() || self.folder_browser.is_some() {
            return;
        }
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
        let mut seat_state = SeatState::new(&globals, qh);
        let output_state = OutputState::new(&globals, qh);

        let config = RadialConfig::load();
        let is_low_end = detect_gpu_profile(&config) == GpuProfile::LowEnd;
        let menu = MenuState::new(config, is_low_end);
        let font_renderer = FontRenderer::new();

        let mut themed_pointer = None;
        let mut pointer = None;
        let mut keyboard = None;
        for seat in seat_state.seats() {
            if themed_pointer.is_none() {
                let surface = compositor_state.create_surface(qh);
                themed_pointer = seat_state.get_pointer_with_theme(
                    qh,
                    &seat,
                    shm.wl_shm(),
                    surface,
                    ThemeSpec::default(),
                ).ok();
                pointer = themed_pointer.as_ref().map(|tp| tp.pointer().clone());
            }
            if keyboard.is_none() {
                keyboard = seat_state.get_keyboard(qh, &seat, None).ok();
            }
        }

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
            pointer,
            themed_pointer,
            current_cursor_icon: None,
            keyboard,
            outputs: Vec::new(),
            primary_output: None,
            current_output_name: None,
            surface_configured: false,
            pending_open_ctx: None,
            dirty: false,
            waiting_for_frame: false,
            render_pixmap: None,
            modal_click_active: false,
            last_frame_time: std::time::Instant::now(),
        })
    }

    /// Handle pointer press events with modal customizer/folder-browser routing
    fn handle_pointer_press(&mut self, button: u32, x: f32, y: f32) {
        // If folder browser modal is open:
        if self.folder_browser.is_some() {
            self.modal_click_active = true;
            if is_right_button(button) {
                if let Some(fb) = &mut self.folder_browser {
                    fb.begin_close(); // Phase 4: animate out
                }
                self.dirty = true;
                return;
            }
            if is_left_button(button) {
                self.handle_folder_browser_click(x, y);
                return;
            }
            return;
        }

        // If customizer modal is open:
        if self.customizer.is_some() {
            self.modal_click_active = true;
            if is_right_button(button) {
                if let Some(c) = &mut self.customizer {
                    c.begin_close(); // Phase 4: animate out
                }
                self.seat_handler.customizer_open = false;
                self.dirty = true;
                return;
            }
            if is_left_button(button) {
                self.handle_customizer_click(x, y);
                return;
            }
            return;
        }

        // Neither modal is open:
        // Check if outer sub-ring slice is clicked (e.g. FileJump target or + Add)
        if self.menu.active_sub_tier == Some(SubTierType::FileJump) && self.menu.outer_hovered_index >= 0 {
            let sub_idx = self.menu.outer_hovered_index as usize;
            if sub_idx < self.menu.sub_slices.len() {
                let sub = &self.menu.sub_slices[sub_idx];
                if sub.is_add_button {
                    // Left OR right click on + Add opens Add Folder Target modal!
                    let mut c = CustomizerState::new();
                    c.open_file_add();
                    self.customizer = Some(c);
                    self.seat_handler.customizer_open = true;
                    self.menu.hovered_index = -1;
                    self.menu.outer_hovered_index = -1;
                    self.menu.center_hovered = false;
                    self.dirty = true;
                    return;
                } else if is_right_button(button) {
                    if let Some(target_idx) = sub.target_index {
                        let mut c = CustomizerState::new();
                        c.open_file_edit(target_idx, &sub.label, sub.target_path.as_deref().unwrap_or(""), &sub.icon);
                        self.customizer = Some(c);
                        self.seat_handler.customizer_open = true;
                        self.menu.hovered_index = -1;
                        self.menu.outer_hovered_index = -1;
                        self.menu.center_hovered = false;
                        self.dirty = true;
                        return;
                    }
                }
            }
        }

        if is_right_button(button) {
            if self.menu.hovered_index >= 0 && (self.menu.hovered_index as usize) < self.menu.current_slices.len() {
                // Right click on main ring slice -> open SliceSwap
                let slot_idx = self.menu.hovered_index as usize;
                let mut c = CustomizerState::new();
                c.open_slice_swap(slot_idx);
                c.selected_item = Some(self.menu.current_slices[slot_idx].id.clone());
                self.customizer = Some(c);
                self.seat_handler.customizer_open = true;
                self.menu.hovered_index = -1;
                self.menu.outer_hovered_index = -1;
                self.menu.center_hovered = false;
                self.dirty = true;
                return;
            } else {
                // Right click in empty space / center hub -> close dial
                self.menu.transition_close_animated(None);
                self.dirty = true;
                return;
            }
        }

        // Standard left-click on radial dial
        if let Some(action) = self.seat_handler.handle_pointer_button(
            &mut self.menu,
            button,
            ButtonState::Pressed,
        ) {
            self.menu.transition_close_animated(Some(action));
        }
    }

    fn handle_pointer_release(&mut self, button: u32) {
        if self.modal_click_active {
            self.modal_click_active = false;
            return;
        }
        if self.customizer.is_some() || self.folder_browser.is_some() {
            return;
        }
        self.dirty = true;
        if let Some(action) = self.seat_handler.handle_pointer_button(
            &mut self.menu,
            button,
            ButtonState::Released,
        ) {
            self.menu.transition_close_animated(Some(action));
        }
    }

    fn handle_pointer_axis(&mut self, delta: f64) {
        if let Some(customizer) = &mut self.customizer {
            if customizer.mode.is_swap() {
                let catalogue = state::actions::function_catalogue();
                let items = customizer.filtered_catalogue(&catalogue);
                let max_scroll = (items.len() as f32 * 54.0 - 376.0).max(0.0);
                customizer.scroll_offset = (customizer.scroll_offset + delta as f32 * 2.0).clamp(0.0, max_scroll);
                self.dirty = true;
                return;
            }
            if customizer.mode.is_select_position() {
                let count = if let CustomizerMode::SelectPosition { is_replace_mode, .. } = &customizer.mode {
                    if *is_replace_mode {
                        self.menu.current_slices.len()
                    } else {
                        self.menu.current_slices.len() + 1
                    }
                } else {
                    0
                };
                let max_scroll = (count as f32 * 56.0 - 396.0).max(0.0);
                customizer.scroll_offset = (customizer.scroll_offset + delta as f32 * 2.0).clamp(0.0, max_scroll);
                self.dirty = true;
                return;
            }
        }
        if let Some(folder_browser) = &mut self.folder_browser {
            let folders = folder_browser.filtered_folders();
            let card_h = 540.0_f32;
            let list_y = 180.0_f32;
            let bottom_bar_h = 64.0_f32;
            let list_h = card_h - list_y - bottom_bar_h;
            let max_scroll = (folders.len() as f32 * 38.0 - list_h + 8.0).max(0.0);
            folder_browser.scroll_offset = (folder_browser.scroll_offset + delta as f32 * 2.0).clamp(0.0, max_scroll);
            self.dirty = true;
            return;
        }
        if let Some(action) = self.seat_handler.handle_pointer_axis(&mut self.menu, delta) {
            execute_action(&action);
        }
    }

    fn handle_customizer_click(&mut self, x: f32, y: f32) {
        let (w, h) = if let Some(surf) = &self.surface {
            (surf.width.max(544) as f32, surf.height.max(544) as f32)
        } else {
            (1920.0, 1080.0)
        };

        let card_w = renderer::customizer::CARD_W.min(w - 20.0);
        let card_h = renderer::customizer::CARD_H.min(h - 20.0);
        let anchor_x = self.menu.center_x;
        let anchor_y = self.menu.center_y;
        let preferred_x = anchor_x - card_w / 2.0 + (w / 2.0 - anchor_x).signum() * 60.0;
        let preferred_y = anchor_y - card_h / 2.0 + (h / 2.0 - anchor_y).signum() * 60.0;
        let card_x = preferred_x.clamp(10.0, w - card_w - 10.0);
        let card_y = preferred_y.clamp(10.0, h - card_h - 10.0);

        // 1. Outside card -> begin close animation
        if x < card_x || x > card_x + card_w || y < card_y || y > card_y + card_h {
            if let Some(c) = &mut self.customizer {
                c.begin_close();
            }
            self.seat_handler.customizer_open = false;
            self.dirty = true;
            return;
        }

        let content_x = card_x + 18.0;
        let content_w = card_w - 36.0;

        // 2. Top-right header buttons
        let close_size = 32.0;
        let close_x = content_x + content_w - close_size;
        let close_y = card_y + 19.0;
        if x >= close_x && x <= close_x + close_size && y >= close_y && y <= close_y + close_size {
            if let Some(c) = &mut self.customizer {
                c.begin_close();
            }
            self.seat_handler.customizer_open = false;
            self.dirty = true;
            return;
        }

        let mode = match &self.customizer {
            Some(c) => c.mode.clone(),
            None => return,
        };

        // Back button (left icon box) when in SelectPosition mode
        if mode.is_select_position() {
            let icon_box_x = content_x;
            let icon_box_y = card_y + 16.0;
            let icon_box_size = 38.0;
            if x >= icon_box_x && x <= icon_box_x + icon_box_size && y >= icon_box_y && y <= icon_box_y + icon_box_size {
                if let Some(c) = &mut self.customizer {
                    c.mode = CustomizerMode::SliceSwap { slot_index: 0 };
                    c.scroll_offset = 0.0;
                }
                self.dirty = true;
                return;
            }
        }

        // Reset to Defaults button (in header, next to close button)
        if mode.is_swap() {
            let reset_x = close_x - 38.0;
            if x >= reset_x && x <= reset_x + close_size && y >= close_y && y <= close_y + close_size {
                self.menu.config.reset_context(&self.menu.context.to_string());
                let _ = self.menu.config.save();
                self.menu.refresh_current_slices();
                self.customizer = None;
                self.seat_handler.customizer_open = false;
                self.dirty = true;
                return;
            }
        }

        match mode {
            CustomizerMode::SliceSwap { slot_index: _ } => {
                // Category tabs (y: card_y + 66.0 .. card_y + 92.0)
                let tabs_y = card_y + 66.0;
                let tab_h = 26.0;
                let tab_gap = 5.0;
                let num_tabs = renderer::customizer::CATEGORIES.len() as f32;
                let tab_w = (content_w - (num_tabs - 1.0) * tab_gap) / num_tabs;

                if y >= tabs_y && y <= tabs_y + tab_h {
                    for (i, cat) in renderer::customizer::CATEGORIES.iter().enumerate() {
                        let tx = content_x + i as f32 * (tab_w + tab_gap);
                        if x >= tx && x <= tx + tab_w {
                            if let Some(c) = &mut self.customizer {
                                c.active_category = cat.to_string();
                                c.scroll_offset = 0.0;
                            }
                            self.dirty = true;
                            return;
                        }
                    }
                }

                // Search bar & clear icon (y: card_y + 102.0 .. card_y + 136.0)
                let search_y = card_y + 102.0;
                let search_h = 34.0;
                if y >= search_y && y <= search_y + search_h && x >= content_x && x <= content_x + content_w {
                    if let Some(c) = &mut self.customizer {
                        if !c.search_query.is_empty() && x >= content_x + content_w - 32.0 {
                            c.search_query.clear();
                            c.scroll_offset = 0.0;
                        } else {
                            c.search_focused = true;
                        }
                    }
                    self.dirty = true;
                    return;
                }

                // Action list items (y: card_y + 146.0 .. card_y + 522.0)
                let list_y = card_y + 146.0;
                let list_h = 376.0;
                if y >= list_y && y <= list_y + list_h && x >= content_x && x <= content_x + content_w {
                    let item_h = 50.0;
                    let item_gap = 4.0;
                    let step = item_h + item_gap;
                    let scroll_offset = self.customizer.as_ref().map(|c| c.scroll_offset).unwrap_or(0.0);
                    let rel_y = y - (list_y - scroll_offset);
                    if rel_y >= 0.0 {
                        let idx = (rel_y / step) as usize;
                        let offset_in_item = rel_y % step;
                        if offset_in_item <= item_h {
                            let catalogue = state::actions::function_catalogue();
                            // Build same ordered list as render: active first, then filtered inactive
                            let active_ids: Vec<String> = self.menu.current_slices.iter().map(|s| s.id.clone()).collect();
                            let filtered = if let Some(c) = &self.customizer {
                                c.filtered_catalogue(&catalogue)
                            } else {
                                Vec::new()
                            };
                            // ordered: (action_id: &str, is_active: bool)
                            let mut ordered: Vec<(&str, bool)> = Vec::new();
                            for active_id in &active_ids {
                                if let Some(def) = catalogue.iter().find(|d| d.id == active_id.as_str()) {
                                    let passes = filtered.iter().any(|f| f.id == def.id);
                                    if passes {
                                        ordered.push((def.id, true));
                                    }
                                }
                            }
                            for def in &filtered {
                                let is_act = active_ids.iter().any(|id| id.as_str() == def.id);
                                if !is_act {
                                    ordered.push((def.id, false));
                                }
                            }

                            if idx < ordered.len() {
                                let (item_id, is_active) = ordered[idx];
                                if is_active {
                                    // Clicked on active item: check if remove button was clicked
                                    let rm_r = 14.0;
                                    let rm_btn_x = content_x + content_w - 10.0 - rm_r * 2.0;
                                    if x >= rm_btn_x {
                                        // Remove this slice
                                        self.menu.config.remove_slice(&self.menu.context.to_string(), item_id);
                                        let _ = self.menu.config.save();
                                        self.menu.refresh_current_slices();
                                        self.dirty = true;
                                        return;
                                    }
                                } else {
                                    // Inactive: open SelectPosition mode so user can choose insertion position or replacement
                                    if let Some(c) = &mut self.customizer {
                                        c.open_select_position(item_id, false);
                                    }
                                    self.dirty = true;
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            CustomizerMode::SelectPosition { action_id, is_replace_mode } => {
                // 1. Mode Switcher Tabs (y: card_y + 66.0 .. card_y + 94.0)
                let tabs_y = card_y + 66.0;
                let tab_h = 28.0;
                let tab_gap = 8.0;
                let tab_w = (content_w - tab_gap) / 2.0;

                if y >= tabs_y && y <= tabs_y + tab_h {
                    // Tab 0: Insert as New Slice
                    if x >= content_x && x <= content_x + tab_w {
                        if is_replace_mode {
                            if let Some(c) = &mut self.customizer {
                                c.mode = CustomizerMode::SelectPosition {
                                    action_id: action_id.clone(),
                                    is_replace_mode: false,
                                };
                                c.scroll_offset = 0.0;
                            }
                            self.dirty = true;
                        }
                        return;
                    }
                    // Tab 1: Replace Existing Slice
                    if x >= content_x + tab_w + tab_gap && x <= content_x + content_w {
                        if !is_replace_mode {
                            if let Some(c) = &mut self.customizer {
                                c.mode = CustomizerMode::SelectPosition {
                                    action_id: action_id.clone(),
                                    is_replace_mode: true,
                                };
                                c.scroll_offset = 0.0;
                            }
                            self.dirty = true;
                        }
                        return;
                    }
                }

                // 2. Choice List Items (y: card_y + 124.0 .. card_y + 520.0)
                let list_y = card_y + 124.0;
                let list_h = 396.0;
                if y >= list_y && y <= list_y + list_h && x >= content_x && x <= content_x + content_w {
                    let item_h = 50.0;
                    let item_gap = 6.0;
                    let step = item_h + item_gap;
                    let scroll_offset = self.customizer.as_ref().map(|c| c.scroll_offset).unwrap_or(0.0);
                    let rel_y = y - (list_y - scroll_offset);
                    if rel_y >= 0.0 {
                        let idx = (rel_y / step) as usize;
                        let offset_in_item = rel_y % step;
                        if offset_in_item <= item_h {
                            if !is_replace_mode {
                                let total_positions = self.menu.current_slices.len() + 1;
                                if idx < total_positions {
                                    self.menu.config.insert_slice(&self.menu.context.to_string(), idx, &action_id);
                                    let _ = self.menu.config.save();
                                    self.menu.refresh_current_slices();
                                    self.customizer = None;
                                    self.seat_handler.customizer_open = false;
                                    self.dirty = true;
                                    return;
                                }
                            } else {
                                let total_positions = self.menu.current_slices.len();
                                if idx < total_positions {
                                    self.menu.config.swap_slice(&self.menu.context.to_string(), idx, &action_id);
                                    let _ = self.menu.config.save();
                                    self.menu.refresh_current_slices();
                                    self.customizer = None;
                                    self.seat_handler.customizer_open = false;
                                    self.dirty = true;
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            CustomizerMode::FileTargetEdit { .. } | CustomizerMode::FileTargetAdd => {
                let is_edit = mode.is_file_edit();
                let target_index_opt = if let CustomizerMode::FileTargetEdit { target_index } = mode {
                    Some(target_index)
                } else {
                    None
                };

                // Field 0: Display Name box (y: card_y + 84.0 .. card_y + 122.0)
                let label_box_y = card_y + 84.0;
                let label_box_h = 38.0;
                if y >= label_box_y && y <= label_box_y + label_box_h && x >= content_x && x <= content_x + content_w {
                    if let Some(c) = &mut self.customizer {
                        c.focused_field = 0;
                    }
                    self.dirty = true;
                    return;
                }

                // Field 1: Path box & Browse button (y: card_y + 154.0 .. card_y + 192.0)
                let path_box_y = card_y + 154.0;
                let path_box_h = 38.0;
                let browse_w = 90.0;
                let path_box_w = content_w - browse_w - 8.0;

                if y >= path_box_y && y <= path_box_y + path_box_h {
                    if x >= content_x && x <= content_x + path_box_w {
                        if let Some(c) = &mut self.customizer {
                            c.focused_field = 1;
                        }
                        self.dirty = true;
                        return;
                    } else if x >= content_x + path_box_w + 8.0 && x <= content_x + content_w {
                        // Browse button clicked! Open folder browser modal
                        let mut fb = FolderBrowserState::new();
                        let start_path = if let Some(c) = &self.customizer {
                            if c.input_path.is_empty() {
                                shellexpand::tilde("~").to_string()
                            } else {
                                shellexpand::tilde(&c.input_path).to_string()
                            }
                        } else {
                            shellexpand::tilde("~").to_string()
                        };
                        let listing = crate::ipc::folder::list_dir_sync(&start_path);
                        fb.set_listing(listing);
                        fb.open(&start_path);
                        self.folder_browser = Some(fb);
                        self.menu.hovered_index = -1;
                        self.menu.outer_hovered_index = -1;
                        self.menu.center_hovered = false;
                        self.dirty = true;
                        return;
                    }
                }

                // Icon picker grid (Flow layout: 10 columns matching shell reference)
                let grid_y = card_y + 226.0;
                let cols = 10;
                let chip_w = 36.0;
                let chip_h = 36.0;
                let gap = 8.0;

                if y >= grid_y && y <= grid_y + 2.0 * (chip_h + gap) && x >= content_x && x <= content_x + content_w {
                    let col = ((x - content_x) / (chip_w + gap)).floor() as usize;
                    let row = ((y - grid_y) / (chip_h + gap)).floor() as usize;
                    if col < cols {
                        let offset_x = (x - content_x) % (chip_w + gap);
                        let offset_y = (y - grid_y) % (chip_h + gap);
                        if offset_x <= chip_w && offset_y <= chip_h {
                            let idx = row * cols + col;
                            if idx < renderer::customizer::AVAILABLE_ICONS.len() {
                                if let Some(c) = &mut self.customizer {
                                    c.input_icon = renderer::customizer::AVAILABLE_ICONS[idx].to_string();
                                }
                                self.dirty = true;
                                return;
                            }
                        }
                    }
                }

                // Action buttons row (y: card_y + 486.0 .. card_y + 522.0)
                let btns_y = card_y + 486.0;
                let btns_h = 36.0;
                if y >= btns_y && y <= btns_y + btns_h {
                    // Delete button (visible only in Edit mode, width 90.0)
                    if is_edit {
                        let del_w = 90.0;
                        if x >= content_x && x <= content_x + del_w {
                            if let Some(t_idx) = target_index_opt {
                                self.menu.config.remove_file_jump_target(t_idx);
                                let _ = self.menu.config.save();
                                self.menu.refresh_sub_slices();
                                self.customizer = None;
                                self.seat_handler.customizer_open = false;
                                self.dirty = true;
                                return;
                            }
                        }
                    }

                    // Save and Cancel buttons on the right
                    let cancel_w = 80.0;
                    let save_w = 106.0;
                    let save_x = content_x + content_w - save_w;
                    let cancel_x = save_x - cancel_w - 8.0;

                    // Cancel
                    if x >= cancel_x && x <= cancel_x + cancel_w {
                        self.customizer = None;
                        self.seat_handler.customizer_open = false;
                        self.dirty = true;
                        return;
                    }

                    // Save
                    if x >= save_x && x <= save_x + save_w {
                        if let Some(c) = &self.customizer {
                            let mut label = c.input_label.trim().to_string();
                            let path = c.input_path.trim().to_string();
                            let icon = c.input_icon.clone();
                            if label.is_empty() && !path.is_empty() {
                                let p = std::path::Path::new(&path);
                                label = p.file_name().and_then(|n| n.to_str()).unwrap_or("Folder").to_string();
                            }
                            if !path.is_empty() {
                                if let Some(t_idx) = target_index_opt {
                                    self.menu.config.update_file_jump_target(t_idx, &label, &path, &icon);
                                } else {
                                    self.menu.config.add_file_jump_target(&label, &path, &icon);
                                }
                                let _ = self.menu.config.save();
                                self.menu.refresh_sub_slices();
                                self.customizer = None;
                                self.seat_handler.customizer_open = false;
                                self.dirty = true;
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_folder_browser_click(&mut self, x: f32, y: f32) {
        let (w, h) = if let Some(surf) = &self.surface {
            (surf.width.max(544) as f32, surf.height.max(544) as f32)
        } else {
            (1920.0, 1080.0)
        };

        let card_w = 480.0_f32.min(w - 20.0);
        let card_h = 540.0_f32.min(h - 20.0);
        let anchor_x = self.menu.center_x;
        let anchor_y = self.menu.center_y;
        let preferred_x = anchor_x - card_w / 2.0 + (w / 2.0 - anchor_x).signum() * 60.0;
        let preferred_y = anchor_y - card_h / 2.0 + (h / 2.0 - anchor_y).signum() * 60.0;
        let card_x = preferred_x.clamp(10.0, w - card_w - 10.0);
        let card_y = preferred_y.clamp(10.0, h - card_h - 10.0);
        let margin = 18.0;

        // Outside card -> close
        if x < card_x || x > card_x + card_w || y < card_y || y > card_y + card_h {
            self.folder_browser = None;
            self.dirty = true;
            return;
        }

        // Close button (top-right, 32x32)
        let close_size = 32.0;
        let close_x = card_x + card_w - margin - close_size;
        let close_y = card_y + 18.0;
        if x >= close_x && x <= close_x + close_size && y >= close_y && y <= close_y + close_size {
            self.folder_browser = None;
            self.dirty = true;
            return;
        }

        // Up / Back button (top-left, 36x36)
        let back_btn_x = card_x + margin;
        let back_btn_y = card_y + 16.0;
        let back_btn_size = 36.0;
        if x >= back_btn_x && x <= back_btn_x + back_btn_size && y >= back_btn_y && y <= back_btn_y + back_btn_size {
            if let Some(fb) = &mut self.folder_browser {
                if let Some(listing) = &fb.listing {
                    let parent = listing.parent.clone();
                    if !parent.is_empty() && parent != fb.current_path {
                        let l = crate::ipc::folder::list_dir_sync(&parent);
                        fb.set_listing(l);
                        self.dirty = true;
                        return;
                    }
                }
            }
        }

        // Path bar refresh icon (card_y + 60.0 .. card_y + 96.0)
        let path_bar_y = card_y + 60.0;
        let path_bar_h = 36.0;
        let refresh_x = card_x + card_w - margin - 36.0;
        if y >= path_bar_y && y <= path_bar_y + path_bar_h && x >= refresh_x && x <= card_x + card_w - margin {
            if let Some(fb) = &mut self.folder_browser {
                let cur = fb.current_path.clone();
                let l = crate::ipc::folder::list_dir_sync(&cur);
                fb.set_listing(l);
                self.dirty = true;
                return;
            }
        }

        // Places Quick Navigation Chips (y: card_y + 104.0 .. card_y + 132.0)
        let places_y = card_y + 104.0;
        let chip_h = 28.0;
        if y >= places_y && y <= places_y + chip_h {
            if let Some(fb) = &mut self.folder_browser {
                let mut chip_x = card_x + margin;
                let places_clone = fb.places.clone();
                for place in &places_clone {
                    let chip_w = (place.name.len() as f32 * 6.5 + 28.0).clamp(58.0, 92.0);
                    if chip_x + chip_w > card_x + card_w - margin {
                        break;
                    }
                    if x >= chip_x && x <= chip_x + chip_w {
                        let l = crate::ipc::folder::list_dir_sync(&place.path);
                        fb.set_listing(l);
                        self.dirty = true;
                        return;
                    }
                    chip_x += chip_w + 6.0;
                }
            }
        }

        // Filter subfolders input box (y: card_y + 140.0 .. card_y + 174.0)
        let filter_y = card_y + 140.0;
        let filter_h = 34.0;
        if y >= filter_y && y <= filter_y + filter_h && x >= card_x + margin && x <= card_x + card_w - margin {
            if let Some(fb) = &mut self.folder_browser {
                if !fb.search_query.is_empty() && x >= card_x + card_w - margin - 30.0 {
                    fb.search_query.clear();
                    fb.scroll_offset = 0.0;
                } else {
                    fb.search_focused = true;
                }
                self.dirty = true;
                return;
            }
        }

        // Folder list items (y: card_y + 180.0 .. card_y + card_h - 64.0)
        let list_y = card_y + 180.0;
        let bottom_bar_h = 64.0;
        let list_h = card_h - (list_y - card_y) - bottom_bar_h;
        if y >= list_y && y <= list_y + list_h && x >= card_x + margin && x <= card_x + card_w - margin {
            let item_step = 38.0;
            let scroll_offset = self.folder_browser.as_ref().map(|fb| fb.scroll_offset).unwrap_or(0.0);
            let rel_y = y - (list_y - scroll_offset);
            if rel_y >= 0.0 {
                let idx = (rel_y / item_step) as usize;
                let clicked_folder = if let Some(fb) = &self.folder_browser {
                    let folders = fb.filtered_folders();
                    if idx < folders.len() {
                        Some(folders[idx].path.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(path) = clicked_folder {
                    let l = crate::ipc::folder::list_dir_sync(&path);
                    if let Some(fb) = &mut self.folder_browser {
                        fb.set_listing(l);
                        self.dirty = true;
                        return;
                    }
                }
            }
        }

        // Bottom Action buttons (bar_y: card_y + card_h - 52.0 .. card_y + card_h - 16.0)
        let bar_y = card_y + card_h - 52.0;
        let btn_h = 36.0;
        if y >= bar_y && y <= bar_y + btn_h {
            let cancel_w = 90.0;
            let cancel_x = card_x + margin;

            // Cancel button (left)
            if x >= cancel_x && x <= cancel_x + cancel_w {
                self.folder_browser = None;
                self.dirty = true;
                return;
            }

            // Select button (right)
            let current_name = self.folder_browser.as_ref().map(|fb| fb.current_folder_name()).unwrap_or_default();
            let select_label = if !current_name.is_empty() {
                format!("Select \"{}\"", current_name)
            } else {
                "Select Folder".to_string()
            };
            let select_w = (select_label.len() as f32 * 7.0 + 44.0).clamp(140.0, 220.0);
            let select_x = card_x + card_w - margin - select_w;

            if x >= select_x && x <= select_x + select_w {
                let path = self.folder_browser.as_ref().map(|fb| fb.current_path.clone());
                if let Some(p) = path {
                    if let Some(c) = &mut self.customizer {
                        c.input_path = p;
                        if c.input_label.is_empty() && !current_name.is_empty() {
                            c.input_label = current_name.to_string();
                        }
                    }
                }
                self.folder_browser = None;
                self.dirty = true;
                return;
            }
        }
    }

    fn handle_customizer_or_folder_key(&mut self, keysym: u32) -> bool {
        if let Some(fb) = &mut self.folder_browser {
            if is_escape(keysym) {
                self.folder_browser = None;
                self.dirty = true;
                return true;
            }
            if keysym == 0xff08 || keysym == 8 {
                if !fb.search_query.is_empty() {
                    fb.search_query.pop();
                    fb.scroll_offset = 0.0;
                } else if let Some(listing) = &fb.listing {
                    let parent = listing.parent.clone();
                    if !parent.is_empty() && parent != fb.current_path {
                        let l = crate::ipc::folder::list_dir_sync(&parent);
                        fb.set_listing(l);
                    }
                }
                self.dirty = true;
                return true;
            }
            if let Some(ch) = keysym_to_char(keysym) {
                if !ch.is_control() {
                    fb.search_query.push(ch);
                    fb.search_focused = true;
                    fb.scroll_offset = 0.0;
                    self.dirty = true;
                    return true;
                }
            }
            return true;
        }

        if let Some(customizer) = &mut self.customizer {
            // 1. Escape: close customizer modal or clear search query / return from select position
            if is_escape(keysym) {
                if customizer.mode.is_select_position() {
                    customizer.mode = CustomizerMode::SliceSwap { slot_index: 0 };
                    customizer.scroll_offset = 0.0;
                    self.dirty = true;
                    return true;
                }
                if customizer.mode.is_swap() && !customizer.search_query.is_empty() {
                    customizer.search_query.clear();
                    customizer.scroll_offset = 0.0;
                    self.dirty = true;
                    return true;
                }
                self.customizer = None;
                self.seat_handler.customizer_open = false;
                self.dirty = true;
                return true;
            }

            // 2. Tab: toggle focus between fields in file target mode
            if keysym == 0xff09 || keysym == 9 {
                if customizer.mode.is_file_edit() || customizer.mode.is_file_add() {
                    customizer.focused_field = (customizer.focused_field + 1) % 2;
                    self.dirty = true;
                    return true;
                }
            }

            // 3. Backspace: delete character or return from select position
            if keysym == 0xff08 || keysym == 8 {
                if customizer.mode.is_select_position() {
                    customizer.mode = CustomizerMode::SliceSwap { slot_index: 0 };
                    customizer.scroll_offset = 0.0;
                } else if customizer.mode.is_swap() {
                    customizer.search_query.pop();
                    customizer.scroll_offset = 0.0;
                } else if customizer.focused_field == 0 {
                    customizer.input_label.pop();
                } else {
                    customizer.input_path.pop();
                }
                self.dirty = true;
                return true;
            }

            // 4. Enter / Return: Save in file target edit/add mode
            if keysym == 0xff0d || keysym == 13 {
                if customizer.mode.is_file_edit() || customizer.mode.is_file_add() {
                    let mut label = customizer.input_label.trim().to_string();
                    let path = customizer.input_path.trim().to_string();
                    let icon = customizer.input_icon.clone();
                    if label.is_empty() && !path.is_empty() {
                        let p = std::path::Path::new(&path);
                        label = p.file_name().and_then(|n| n.to_str()).unwrap_or("Folder").to_string();
                    }
                    if !path.is_empty() {
                        match customizer.mode {
                            CustomizerMode::FileTargetEdit { target_index } => {
                                self.menu.config.update_file_jump_target(target_index, &label, &path, &icon);
                            }
                            CustomizerMode::FileTargetAdd => {
                                self.menu.config.add_file_jump_target(&label, &path, &icon);
                            }
                            _ => {}
                        }
                        let _ = self.menu.config.save();
                        if self.menu.active_sub_tier == Some(SubTierType::FileJump) {
                            self.menu.refresh_sub_slices();
                        }
                        self.customizer = None;
                        self.seat_handler.customizer_open = false;
                        self.dirty = true;
                        return true;
                    }
                }
            }

            // 5. Printable characters
            if let Some(ch) = keysym_to_char(keysym) {
                if !ch.is_control() {
                    if customizer.mode.is_select_position() {
                        if ch.is_ascii_digit() && ch != '0' {
                            let slot_num = ch.to_digit(10).unwrap() as usize;
                            let target_idx = slot_num - 1;
                            if let CustomizerMode::SelectPosition { action_id, is_replace_mode } = &customizer.mode {
                                let act_id = action_id.clone();
                                let replace = *is_replace_mode;
                                if !replace && target_idx <= self.menu.current_slices.len() {
                                    self.menu.config.insert_slice(&self.menu.context.to_string(), target_idx, &act_id);
                                    let _ = self.menu.config.save();
                                    self.menu.refresh_current_slices();
                                    self.customizer = None;
                                    self.seat_handler.customizer_open = false;
                                    self.dirty = true;
                                    return true;
                                } else if replace && target_idx < self.menu.current_slices.len() {
                                    self.menu.config.swap_slice(&self.menu.context.to_string(), target_idx, &act_id);
                                    let _ = self.menu.config.save();
                                    self.menu.refresh_current_slices();
                                    self.customizer = None;
                                    self.seat_handler.customizer_open = false;
                                    self.dirty = true;
                                    return true;
                                }
                            }
                        }
                    } else if customizer.mode.is_swap() {
                        customizer.search_query.push(ch);
                        customizer.search_focused = true;
                        customizer.scroll_offset = 0.0;
                    } else if customizer.focused_field == 0 {
                        customizer.input_label.push(ch);
                    } else {
                        customizer.input_path.push(ch);
                    }
                    self.dirty = true;
                    return true;
                }
            }

            return true;
        }

        false
    }

    /// Returns true if pointer coordinate (x, y) is currently hovering a text input box
    fn is_text_input_hovered(&self, x: f32, y: f32) -> bool {
        if self.folder_browser.is_some() {
            let (w, h) = if let Some(surf) = &self.surface {
                (surf.width.max(544) as f32, surf.height.max(544) as f32)
            } else {
                (1920.0, 1080.0)
            };
            let card_w = 480.0_f32.min(w - 20.0);
            let card_h = 540.0_f32.min(h - 20.0);
            let anchor_x = self.menu.center_x;
            let anchor_y = self.menu.center_y;
            let preferred_x = anchor_x - card_w / 2.0 + (w / 2.0 - anchor_x).signum() * 60.0;
            let preferred_y = anchor_y - card_h / 2.0 + (h / 2.0 - anchor_y).signum() * 60.0;
            let card_x = preferred_x.clamp(10.0, w - card_w - 10.0);
            let card_y = preferred_y.clamp(10.0, h - card_h - 10.0);
            let margin = 18.0;

            let filter_y = card_y + 140.0;
            let filter_h = 34.0;
            if y >= filter_y && y <= filter_y + filter_h && x >= card_x + margin && x <= card_x + card_w - margin {
                return true;
            }
            return false;
        }

        if let Some(c) = &self.customizer {
            let (w, h) = if let Some(surf) = &self.surface {
                (surf.width.max(544) as f32, surf.height.max(544) as f32)
            } else {
                (1920.0, 1080.0)
            };
            let card_w = renderer::customizer::CARD_W.min(w - 20.0);
            let card_h = renderer::customizer::CARD_H.min(h - 20.0);
            let anchor_x = self.menu.center_x;
            let anchor_y = self.menu.center_y;
            let preferred_x = anchor_x - card_w / 2.0 + (w / 2.0 - anchor_x).signum() * 60.0;
            let preferred_y = anchor_y - card_h / 2.0 + (h / 2.0 - anchor_y).signum() * 60.0;
            let card_x = preferred_x.clamp(10.0, w - card_w - 10.0);
            let card_y = preferred_y.clamp(10.0, h - card_h - 10.0);
            let content_x = card_x + 18.0;
            let content_w = card_w - 36.0;

            if c.mode.is_swap() {
                let search_y = card_y + 102.0;
                let search_h = 34.0;
                if y >= search_y && y <= search_y + search_h && x >= content_x && x <= content_x + content_w {
                    return true;
                }
            } else if c.mode.is_select_position() {
                return false;
            } else {
                let label_box_y = card_y + 84.0;
                let label_box_h = 38.0;
                if y >= label_box_y && y <= label_box_y + label_box_h && x >= content_x && x <= content_x + content_w {
                    return true;
                }
                let path_box_y = card_y + 154.0;
                let path_box_h = 38.0;
                let browse_w = 90.0;
                let path_box_w = content_w - browse_w - 8.0;
                if y >= path_box_y && y <= path_box_y + path_box_h && x >= content_x && x <= content_x + path_box_w {
                    return true;
                }
            }
            return false;
        }

        false
    }

    /// Find WlOutput by monitor name (e.g. "eDP-1", "DP-1")
    pub fn find_output_by_name(&self, name: &str) -> Option<WlOutput> {
        self.outputs
            .iter()
            .find(|o| {
                if let Some(info) = self.output_state.info(o) {
                    if let Some(n) = &info.name {
                        return n == name;
                    }
                }
                false
            })
            .cloned()
    }

    /// Open or map radial surface and transition to Opening phase
    pub fn open_menu(&mut self, ctx: HyprContext, qh: &QueueHandle<Self>) -> Result<()> {
        if self.menu.config.reload_system_colors() {
            self.dirty = true;
        }
        self.customizer = None;
        self.folder_browser = None;
        self.seat_handler.customizer_open = false;
        self.current_cursor_icon = None;

        let target_output = ctx
            .target_monitor_name
            .as_deref()
            .and_then(|name| self.find_output_by_name(name))
            .or_else(|| self.primary_output.clone());

        // If surface already exists on a different output, drop it first to rebuild on target output
        if self.surface.is_some() && self.current_output_name != ctx.target_monitor_name {
            self.surface = None;
            self.surface_configured = false;
            self.current_output_name = None;
        }

        if self.surface.is_none() {
            let surface = RadialSurface::new(
                &self.layer_shell,
                &self.shm,
                &self.compositor_state,
                qh,
                target_output.as_ref(),
            )?;
            self.surface = Some(surface);
            self.current_output_name = ctx.target_monitor_name.clone();
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
        let surface_hex = self.menu.config.colors.as_ref().map(|c| c.surface_hex()).unwrap_or("#141313");
        let subtext_hex = self.menu.config.colors.as_ref().map(|c| c.subtext_hex()).unwrap_or("#948f94");
        let on_surface_hex = self.menu.config.colors.as_ref().map(|c| c.on_surface_hex()).unwrap_or("#e3e2e2");

        let primary_col = parse_hex_color(primary_hex);
        let on_primary_col = parse_hex_color(on_primary_hex);
        let surface_col = parse_hex_color(surface_hex);
        let subtext_col = parse_hex_color(subtext_hex);
        let on_surface_col = parse_hex_color(on_surface_hex);

        let slice_count = self.menu.current_slices.len();
        if slice_count == 0 {
            return true;
        }

        let hovered = if self.menu.drag.is_dragging {
            self.menu.drag.target_index
        } else {
            self.menu.hovered_index
        };
        let hover_factor = if self.menu.drag.is_dragging {
            1.0
        } else {
            self.menu.anim.hover_factor
        };

        // Phase 2: Global entrance scale + opacity (applied as Transform + alpha multiplier)
        // Also apply closing_opacity fade during close phase
        let global_opacity = (self.menu.anim.overall_opacity * self.menu.anim.closing_opacity).clamp(0.0, 1.0);
        let global_scale = self.menu.anim.overall_scale.clamp(0.0, 1.5);
        // Build a scale transform centered at dial center
        let global_xform = if (global_scale - 1.0).abs() > 0.001 {
            SkTransform::from_scale(global_scale, global_scale)
                .post_translate(cx * (1.0 - global_scale), cy * (1.0 - global_scale))
        } else {
            SkTransform::identity()
        };

        let modal_active = self.folder_browser.is_some() || self.customizer.is_some();

        if !modal_active {
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

                    // Apply global_opacity to gradient stops
                    let a = global_opacity;
                    let light_faded = Color::from_rgba(light_col.red(), light_col.green(), light_col.blue(), light_col.alpha() * a).unwrap_or(light_col);
                    let primary_faded = Color::from_rgba(primary_col.red(), primary_col.green(), primary_col.blue(), primary_col.alpha() * a).unwrap_or(primary_col);
                    let dark_faded = Color::from_rgba(dark_col.red(), dark_col.green(), dark_col.blue(), dark_col.alpha() * a).unwrap_or(dark_col);

                    let grad = RadialGradient::new(
                        Point::from_xy(cx, cy),
                        Point::from_xy(cx, cy),
                        SLICE_OUTER_R + 10.0,
                        vec![
                            GradientStop::new(0.0, light_faded),
                            GradientStop::new(0.35, primary_faded),
                            GradientStop::new(1.0, dark_faded),
                        ],
                        SpreadMode::Pad,
                        SkTransform::identity(),
                    );
                    if let Some(shader) = grad {
                        paint.shader = shader;
                    } else {
                        paint.set_color(primary_faded);
                    }
                    pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, global_xform, None);

                    let mut stroke_paint = Paint::default();
                    let stroke_col = Color::from_rgba(light_col.red(), light_col.green(), light_col.blue(), light_col.alpha() * a).unwrap_or(light_col);
                    stroke_paint.set_color(stroke_col);
                    stroke_paint.anti_alias = true;
                    let stroke = Stroke { width: 1.6, ..Default::default() };
                    pixmap.stroke_path(&path, &stroke_paint, &stroke, global_xform, None);
                } else {
                    // Neutral frosted glass gradient softly harmonized with system surface colors:
                    let (stop0_raw, stop1_raw) = if self.menu.is_low_end_gpu {
                        (
                            Color::from_rgba(
                                (surface_col.red() * 1.2).clamp(0.08, 0.25),
                                (surface_col.green() * 1.2).clamp(0.08, 0.25),
                                (surface_col.blue() * 1.2).clamp(0.08, 0.25),
                                0.88,
                            ).unwrap_or(Color::BLACK),
                            Color::from_rgba(
                                (surface_col.red() * 0.8).clamp(0.05, 0.20),
                                (surface_col.green() * 0.8).clamp(0.05, 0.20),
                                (surface_col.blue() * 0.8).clamp(0.05, 0.20),
                                0.82,
                            ).unwrap_or(Color::BLACK),
                        )
                    } else {
                        (
                            Color::from_rgba(
                                (surface_col.red() * 1.3).clamp(0.06, 0.22),
                                (surface_col.green() * 1.3).clamp(0.06, 0.22),
                                (surface_col.blue() * 1.3).clamp(0.07, 0.25),
                                0.44,
                            ).unwrap_or(Color::BLACK),
                            Color::from_rgba(
                                (surface_col.red() * 0.9).clamp(0.03, 0.18),
                                (surface_col.green() * 0.9).clamp(0.03, 0.18),
                                (surface_col.blue() * 0.9).clamp(0.04, 0.20),
                                0.34,
                            ).unwrap_or(Color::BLACK),
                        )
                    };

                    // Modulate by global opacity
                    let a = global_opacity;
                    let stop0 = Color::from_rgba(stop0_raw.red(), stop0_raw.green(), stop0_raw.blue(), stop0_raw.alpha() * a).unwrap_or(stop0_raw);
                    let stop1 = Color::from_rgba(stop1_raw.red(), stop1_raw.green(), stop1_raw.blue(), stop1_raw.alpha() * a).unwrap_or(stop1_raw);

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
                    pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, global_xform, None);

                    // Subtle white sheen
                    let mut sheen_paint = Paint::default();
                    sheen_paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.04 * a).unwrap_or(Color::WHITE));
                    sheen_paint.anti_alias = true;
                    pixmap.fill_path(&path, &sheen_paint, tiny_skia::FillRule::Winding, global_xform, None);

                    // Crisp soft white translucent border
                    let mut stroke_paint = Paint::default();
                    stroke_paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.22 * a).unwrap_or(Color::WHITE));
                    stroke_paint.anti_alias = true;
                    let stroke = Stroke { width: 1.0, ..Default::default() };
                    pixmap.stroke_path(&path, &stroke_paint, &stroke, global_xform, None);
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

                // Apply global scale to icon position (scale from cx,cy)
                let scaled_icon_x = cx + (icon_x - cx) * global_scale;
                let scaled_icon_y = cy + (icon_y - cy) * global_scale;

                let is_hov = i as i32 == hovered || (i as i32 == self.menu.parent_slice_index && self.menu.active_sub_tier.is_some());
                let icon_base_col = if is_hov { on_primary_col } else { Color::from_rgba8(255, 255, 255, 240) };
                let icon_col = Color::from_rgba(
                    icon_base_col.red(),
                    icon_base_col.green(),
                    icon_base_col.blue(),
                    icon_base_col.alpha() * p * global_opacity,
                ).unwrap_or(icon_base_col);

                let icon_size = (if is_hov { 28.0 } else { 24.0 }) * ease;
                draw_icon(&mut self.font_renderer, pixmap, &slice.icon, scaled_icon_x, scaled_icon_y, icon_size, icon_col);

                if i < 9 && p >= 0.5 {
                    let badge_x = scaled_icon_x + 12.0;
                    let badge_y = scaled_icon_y - 12.0;
                    draw_number_badge(&mut self.font_renderer, pixmap, i + 1, badge_x, badge_y, is_hov, primary_col, on_primary_col);
                }
            }

            // 3. Draw Sub-Ring if active OR if it's still animating closed
            let sub_is_visible = (self.menu.active_sub_tier.is_some() && !self.menu.sub_slices.is_empty())
                || (self.menu.anim.sub_reveal_progress > 0.01 && !self.menu.sub_slices.is_empty());
            if sub_is_visible {
                let sub_start_deg = self.menu.sub_start_angle();
                let sub_width_deg = self.menu.sub_slice_width();
                // Compute closing progress: 0.0 = fully open, 1.0 = fully closed
                let sub_closing_progress = if self.menu.anim.sub_closing_active {
                    let total_span = 100.0_f32 + (self.menu.sub_slices.len() as f32 - 1.0).max(0.0) * 28.0;
                    (self.menu.anim.sub_closing_elapsed / total_span).clamp(0.0, 1.0)
                } else {
                    0.0
                };
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
                    sub_closing_progress,
                    primary_col,
                    on_primary_col,
                    self.menu.is_low_end_gpu,
                );
            }

            // 4. Draw Center Hub
            // Phase 2: hub_hover_factor drives smooth scale-up (was instant 1.06 jump)
            let hub_scale_with_hover = self.menu.anim.hub_scale
                * (1.0 + 0.06 * self.menu.anim.hub_hover_factor.clamp(0.0, 1.0))
                * global_scale.max(0.5); // apply global entrance scale
            let (hub_icon, hub_label) = self.menu.active_hover_hub_content();
            draw_center_hub(
                &mut self.font_renderer,
                pixmap,
                cx,
                cy,
                44.0,
                hub_icon.as_deref(),
                &hub_label,
                self.menu.center_hovered,
                hub_scale_with_hover,
                primary_col,
                self.menu.is_low_end_gpu,
            );
        }

        // 5. Draw Folder Browser modal or Customizer modal if active
        let customizer_colors = CustomizerColors {
            primary: primary_col,
            on_primary: on_primary_col,
            on_surface: on_surface_col,
            subtext: subtext_col,
            card_bg: surface_col,
            surface_base: surface_col,
        };

        if let Some(folder_state) = &self.folder_browser {
            renderer::folder_browser::render_folder_browser_with_font(
                folder_state,
                pixmap,
                w as f32,
                h as f32,
                self.menu.center_x,
                self.menu.center_y,
                &mut self.font_renderer,
                customizer_colors,
            );
        } else if let Some(customizer_state) = &self.customizer {
            let cat = state::actions::function_catalogue();
            let active_ids: Vec<String> = self.menu.current_slices.iter().map(|s| s.id.clone()).collect();
            renderer::customizer::render_customizer_with_font(
                customizer_state,
                &cat,
                &active_ids,
                pixmap,
                w as f32,
                h as f32,
                self.menu.center_x,
                self.menu.center_y,
                &mut self.font_renderer,
                customizer_colors,
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
                let rev_dur = if self.menu.is_low_end_gpu { 70.0 } else { (slice_count * 28.0).max(160.0) };

                // Phase 2: Global entrance — overall_scale 0.88->1.0 (100ms OutBack(1.15))
                //          and overall_opacity 0->1 (70ms OutCubic), run simultaneously from t=0
                if !self.menu.is_low_end_gpu {
                    let t_scale = (self.menu.anim.opening_elapsed / 100.0).clamp(0.0, 1.0);
                    self.menu.anim.overall_scale = 0.88 + 0.12 * Easing::OutBack(1.15).value(t_scale);
                    let t_opacity = (self.menu.anim.opening_elapsed / 70.0).clamp(0.0, 1.0);
                    self.menu.anim.overall_opacity = Easing::OutCubic.value(t_opacity);
                } else {
                    self.menu.anim.overall_scale = 1.0;
                    self.menu.anim.overall_opacity = 1.0;
                }

                // 1. Hub pops up first with OutBack(1.3)
                let t_hub = (self.menu.anim.opening_elapsed / hub_dur).clamp(0.0, 1.0);
                self.menu.anim.hub_scale = if self.menu.is_low_end_gpu {
                    t_hub
                } else {
                    Easing::OutBack(1.3).value(t_hub)
                };

                // Phase 2: Hub micro-pulse — after hub_dur, briefly scale to 1.08 and back
                if !self.menu.is_low_end_gpu && self.menu.anim.opening_elapsed >= hub_dur {
                    if !self.menu.anim.hub_pulse_active && !self.menu.anim.hub_pulse_elapsed.is_nan() && self.menu.anim.hub_pulse_elapsed < 80.0 {
                        self.menu.anim.hub_pulse_active = true;
                    }
                    if self.menu.anim.hub_pulse_active {
                        self.menu.anim.hub_pulse_elapsed = (self.menu.anim.hub_pulse_elapsed + dt).min(80.0);
                        let t_pulse = (self.menu.anim.hub_pulse_elapsed / 80.0).clamp(0.0, 1.0);
                        // OutBack(2.0) creates a big spring; map 0..1 to scale 1.0..1.08..1.0
                        let pulse_raw = Easing::OutBack(2.0).value(t_pulse);
                        // pulse_raw goes 0 -> overshoot -> 1; we want 1.0 -> 1.08 -> 1.0
                        let peak = (pulse_raw - 1.0).abs(); // distance from 1.0 during overshoot
                        self.menu.anim.hub_scale = (1.0 + peak * 0.08).max(1.0);
                        if self.menu.anim.hub_pulse_elapsed >= 80.0 {
                            self.menu.anim.hub_scale = 1.0;
                            self.menu.anim.hub_pulse_active = false;
                        }
                    }
                }

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
                    self.menu.anim.overall_scale = 1.0;
                    self.menu.anim.overall_opacity = 1.0;
                    self.menu.anim.reveal_progress = slice_count;
                    self.menu.phase = MenuPhase::Open;
                }
            }
            MenuPhase::Open => {
                // Phase 1: Main ring hover spring — 110ms OutBack(1.35) (was 160ms OutBack(1.25))
                if self.menu.hovered_index >= 0 {
                    self.menu.anim.hover_elapsed = (self.menu.anim.hover_elapsed + dt).min(110.0);
                    let t = (self.menu.anim.hover_elapsed / 110.0).clamp(0.0, 1.0);
                    let next = if self.menu.is_low_end_gpu { 1.0 } else { Easing::OutBack(1.35).value(t) };
                    if (next - self.menu.anim.hover_factor).abs() > 0.002 {
                        self.menu.anim.hover_factor = next;
                        self.dirty = true;
                    }
                } else if self.menu.anim.hover_factor > 0.002 {
                    // Phase 1: Hover fade — 80ms OutCubic (was 120ms OutQuad)
                    self.menu.anim.hover_elapsed = (self.menu.anim.hover_elapsed + dt).min(80.0);
                    let t = (self.menu.anim.hover_elapsed / 80.0).clamp(0.0, 1.0);
                    let fade_start = if self.menu.anim.hover_fade_start > 0.001 { self.menu.anim.hover_fade_start } else { 1.0 };
                    let next = (fade_start * (1.0 - Easing::OutCubic.value(t))).max(0.0);
                    if (next - self.menu.anim.hover_factor).abs() > 0.002 {
                        self.menu.anim.hover_factor = next;
                        self.dirty = true;
                    }
                } else {
                    self.menu.anim.hover_factor = 0.0;
                }

                // Phase 2: Hub hover scale spring
                if self.menu.center_hovered {
                    self.menu.anim.hover_elapsed = (self.menu.anim.hover_elapsed + dt).min(80.0);
                    let t = (self.menu.anim.hover_elapsed / 80.0).clamp(0.0, 1.0);
                    let next = if self.menu.is_low_end_gpu { 1.0 } else { Easing::OutBack(1.4).value(t) };
                    if (next - self.menu.anim.hub_hover_factor).abs() > 0.002 {
                        self.menu.anim.hub_hover_factor = next;
                        self.dirty = true;
                    }
                } else if self.menu.anim.hub_hover_factor > 0.002 {
                    let next = (self.menu.anim.hub_hover_factor - dt / 60.0).max(0.0);
                    if (next - self.menu.anim.hub_hover_factor).abs() > 0.002 {
                        self.menu.anim.hub_hover_factor = next;
                        self.dirty = true;
                    }
                } else {
                    self.menu.anim.hub_hover_factor = 0.0;
                }

                // Phase 1: Outer sub-ring hover spring — 95ms OutBack(1.3) (was 140ms OutBack(1.2))
                if self.menu.outer_hovered_index >= 0 {
                    self.menu.anim.outer_hover_elapsed = (self.menu.anim.outer_hover_elapsed + dt).min(95.0);
                    let t = (self.menu.anim.outer_hover_elapsed / 95.0).clamp(0.0, 1.0);
                    let next = if self.menu.is_low_end_gpu { 1.0 } else { Easing::OutBack(1.3).value(t) };
                    if (next - self.menu.anim.outer_hover_factor).abs() > 0.002 {
                        self.menu.anim.outer_hover_factor = next;
                        self.dirty = true;
                    }
                } else if self.menu.anim.outer_hover_factor > 0.002 {
                    // Phase 1: Outer hover fade — 65ms OutCubic (was 100ms OutQuad)
                    self.menu.anim.outer_hover_elapsed = (self.menu.anim.outer_hover_elapsed + dt).min(65.0);
                    let t = (self.menu.anim.outer_hover_elapsed / 65.0).clamp(0.0, 1.0);
                    let fade_start = if self.menu.anim.outer_hover_fade_start > 0.001 { self.menu.anim.outer_hover_fade_start } else { 1.0 };
                    let next = (fade_start * (1.0 - Easing::OutCubic.value(t))).max(0.0);
                    if (next - self.menu.anim.outer_hover_factor).abs() > 0.002 {
                        self.menu.anim.outer_hover_factor = next;
                        self.dirty = true;
                    }
                } else {
                    self.menu.anim.outer_hover_factor = 0.0;
                }

                // Phase 3: Sub-tier staggered reveal — flat 150ms (was per-count scaled)
                if self.menu.active_sub_tier.is_some() && !self.menu.sub_slices.is_empty() {
                    let sub_count = self.menu.sub_slices.len() as f32;
                    let sub_dur = if self.menu.is_low_end_gpu { 80.0 } else { 150.0 };
                    self.menu.anim.sub_elapsed = (self.menu.anim.sub_elapsed + dt).min(sub_dur);
                    let t = (self.menu.anim.sub_elapsed / sub_dur).clamp(0.0, 1.0);
                    let next = Easing::OutCubic.value(t) * sub_count;
                    if (next - self.menu.anim.sub_reveal_progress).abs() > 0.002 {
                        self.menu.anim.sub_reveal_progress = next;
                        self.dirty = true;
                    }
                } else if self.menu.anim.sub_reveal_progress > 0.0 {
                    // Sub-ring collapse — reverse-stagger (matches CLOSE_WEDGE_MS=100, CLOSE_STAGGER_MS=28 in subring.rs)
                    if !self.menu.anim.sub_closing_active {
                        self.menu.anim.sub_closing_active = true;
                        self.menu.anim.sub_closing_elapsed = 0.0;
                    }
                    let sub_count = self.menu.sub_slices.len();
                    // Total close span: CLOSE_WEDGE_MS + (m-1)*CLOSE_STAGGER_MS
                    let total_span = 100.0 + (sub_count as f32 - 1.0).max(0.0) * 28.0;
                    self.menu.anim.sub_closing_elapsed =
                        (self.menu.anim.sub_closing_elapsed + dt).min(total_span + 20.0);
                    // Drain reveal_progress proportionally so wedges disappear in sync
                    let t = (self.menu.anim.sub_closing_elapsed / total_span).clamp(0.0, 1.0);
                    // Smooth the progress drain: OutCubic so it starts slow and accelerates
                    let drain = {
                        let inv = 1.0 - t;
                        inv * inv * inv
                    };
                    let next = self.menu.anim.sub_reveal_progress.max(0.0) * drain;
                    if next < 0.01 || self.menu.anim.sub_closing_elapsed >= total_span + 10.0 {
                        self.menu.anim.sub_reveal_progress = 0.0;
                        self.menu.anim.sub_closing_active = false;
                        self.menu.anim.sub_closing_elapsed = 0.0;
                    } else {
                        self.menu.anim.sub_reveal_progress = next;
                    }
                    self.dirty = true;
                } else {
                    self.menu.anim.sub_closing_active = false;
                }
            }
            MenuPhase::ClosingAnimated { ref pending_action } => {
                self.dirty = true;
                self.menu.anim.closing_elapsed += dt;

                let collapse_dur = if self.menu.is_low_end_gpu { 40.0 } else { 90.0 };
                let hub_dur = if self.menu.is_low_end_gpu { 20.0 } else { 50.0 };
                let total_dur = collapse_dur + hub_dur;

                // Phase 2: closing_opacity fades 1.0->0.0 over entire close duration (InQuad)
                let t_fade = (self.menu.anim.closing_elapsed / total_dur).clamp(0.0, 1.0);
                self.menu.anim.closing_opacity = (1.0 - Easing::InQuad.value(t_fade)).max(0.0);

                // 1. Slices collapse inward (InCubic)
                let t_collapse = (self.menu.anim.closing_elapsed / collapse_dur).clamp(0.0, 1.0);
                let ease_collapse = 1.0 - Easing::InCubic.value(t_collapse);
                let slice_count = self.menu.current_slices.len() as f32;
                self.menu.anim.reveal_progress = ease_collapse * slice_count;
                self.menu.anim.sub_reveal_progress = ease_collapse * self.menu.sub_slices.len() as f32;

                // 2. Hub pops down (InQuad) — simultaneous with close now
                let t_hub = (self.menu.anim.closing_elapsed / total_dur).clamp(0.0, 1.0);
                self.menu.anim.hub_scale = (1.0 - Easing::InQuad.value(t_hub)).max(0.0);

                if self.menu.anim.closing_elapsed >= total_dur {
                    let action_opt = pending_action.clone();
                    self.menu.phase = MenuPhase::Hidden;
                    self.dirty = false;
                    self.waiting_for_frame = false;
                    self.surface = None;
                    self.surface_configured = false;
                    self.current_output_name = None;
                    self.pending_open_ctx = None;
                    if let Some(action) = action_opt {
                        execute_action(&action);
                    }
                }
            }
            _ => {}
        }

        // Phase 4: Modal open — 160ms OutBack(1.1) spring (was 180ms OutCubic)
        // Phase 4: Modal close — 120ms InBack(0.9) exit animation
        if let Some(c) = &mut self.customizer {
            if c.is_closing {
                // Close animation: 120ms InBack(0.9) — scale+fade out
                c.close_elapsed = (c.close_elapsed + dt).min(120.0);
                let t = (c.close_elapsed / 120.0).clamp(0.0, 1.0);
                let next = (1.0 - Easing::InBack(0.9).value(t)).max(0.0);
                c.anim_progress = next;
                self.dirty = true;
                if c.close_elapsed >= 120.0 {
                    // Animation done, remove modal
                    self.customizer = None;
                }
            } else if c.anim_progress < 1.0 {
                // Open animation: 160ms OutBack(1.1)
                c.anim_elapsed = (c.anim_elapsed + dt).min(160.0);
                let t = (c.anim_elapsed / 160.0).clamp(0.0, 1.0);
                let next = Easing::OutBack(1.1).value(t).clamp(0.0, 1.1); // allow slight overshoot
                if (next - c.anim_progress).abs() > 0.002 {
                    c.anim_progress = next;
                    self.dirty = true;
                }
                if c.anim_elapsed >= 160.0 {
                    c.anim_progress = 1.0;
                    self.dirty = true;
                }
            }
        }

        if let Some(fb) = &mut self.folder_browser {
            if fb.is_closing {
                // Close animation: 120ms InBack(0.9)
                fb.close_elapsed = (fb.close_elapsed + dt).min(120.0);
                let t = (fb.close_elapsed / 120.0).clamp(0.0, 1.0);
                let next = (1.0 - Easing::InBack(0.9).value(t)).max(0.0);
                fb.anim_progress = next;
                self.dirty = true;
                if fb.close_elapsed >= 120.0 {
                    self.folder_browser = None;
                }
            } else if fb.anim_progress < 1.0 {
                // Open animation: 160ms OutBack(1.1)
                fb.anim_elapsed = (fb.anim_elapsed + dt).min(160.0);
                let t = (fb.anim_elapsed / 160.0).clamp(0.0, 1.0);
                let next = Easing::OutBack(1.1).value(t).clamp(0.0, 1.1);
                if (next - fb.anim_progress).abs() > 0.002 {
                    fb.anim_progress = next;
                    self.dirty = true;
                }
                if fb.anim_elapsed >= 160.0 {
                    fb.anim_progress = 1.0;
                    self.dirty = true;
                }
            }
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

    // Start background system theme colors watcher
    let (colors_tx, mut colors_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let _colors_watcher = {
        use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
        let watch_dir_str = shellexpand::tilde("~/.local/state/quickshell/user/generated").to_string();
        let watch_dir = std::path::Path::new(&watch_dir_str);
        if !watch_dir.exists() {
            let _ = std::fs::create_dir_all(watch_dir);
        }
        let tx = colors_tx.clone();
        match RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let is_colors = event.paths.iter().any(|p| {
                        p.file_name().and_then(|n| n.to_str()).map_or(false, |name| name == "colors.json")
                    });
                    if is_colors {
                        let _ = tx.send(());
                    }
                }
            },
            Config::default(),
        ) {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(watch_dir, RecursiveMode::NonRecursive) {
                    log::warn!("Failed to watch directory {} for theme colors: {e}", watch_dir.display());
                }
                Some(watcher)
            }
            Err(e) => {
                log::warn!("Failed to create notify watcher for theme colors: {e}");
                None
            }
        }
    };

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

            // Real-time system theme colors reload from Matugen / switchwall
            Some(_) = colors_rx.recv() => {
                if app.menu.config.reload_system_colors() {
                    log::info!("Live theme colors updated from system!");
                    app.dirty = true;
                }
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
    use crate::state::actions::{ActionId, SliceItem};

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

    #[test]
    fn test_modal_click_isolation_prevents_action_bleed() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        let mut term = SliceItem::new("term", "Terminal", "terminal", 0);
        term.action = Some(ActionId::Terminal);
        menu.current_slices = vec![term];

        // Simulate opening customizer on slice 0: hovered_index is cleared to -1
        menu.hovered_index = -1;
        let mut modal_click_active = false;
        let mut customizer_open = true;

        // User clicks inside customizer (e.g. at remove or select position)
        // Modal consumes press
        modal_click_active = true;
        // Modal finishes action and closes
        customizer_open = false;

        // User releases mouse button: release must be consumed by modal_click_active!
        let action = if modal_click_active {
            modal_click_active = false;
            None
        } else if customizer_open {
            None
        } else {
            seat.handle_pointer_button(&mut menu, 0x110, ButtonState::Released)
        };

        // No action triggered! Menu stays open, terminal is NOT executed!
        assert_eq!(action, None);
        assert_eq!(menu.phase, MenuPhase::Open);
    }

    #[test]
    fn test_drag_reorder_persists_config() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        let s0 = SliceItem::new("item0", "Item 0", "icon0", 0);
        let s1 = SliceItem::new("item1", "Item 1", "icon1", 1);
        menu.current_slices = vec![s0, s1];
        menu.config.global_slices = vec!["item0".into(), "item1".into()];

        menu.drag.start_drag(0, 200.0, 100.0);
        menu.drag.is_dragging = true;
        menu.drag.target_index = 1;

        let reordered = menu.finish_drag_reorder();
        assert!(reordered);
        assert_eq!(menu.current_slices[0].id, "item1");
        assert_eq!(menu.current_slices[1].id, "item0");
        assert_eq!(menu.config.global_slices[0], "item1");
        assert_eq!(menu.config.global_slices[1], "item0");
        assert_eq!(menu.hovered_index, 1);
    }
}
