# Radial Dial (Rust)

A standalone, ultra-low-resource native Wayland radial menu daemon designed for **Hyprland**.

Completely migrated from the original 5,179-line QML + Python implementation to pure Rust.

```
┌─────────────────────────────────────────────────────────────┐
│                    Radial Dial Architecture                 │
├──────────────────────────────┬──────────────────────────────┤
│ Wayland Layer Shell          │ smithay-client-toolkit (SCT) │
│ 2D Vector Rasterizer         │ tiny-skia (SIMD-accelerated) │
│ Font Shaping & Text Engine   │ cosmic-text + FreeType       │
│ Asynchronous Runtime & IPC   │ tokio                        │
│ Configuration & Serde        │ serde_json (~/.config/...)   │
└──────────────────────────────┴──────────────────────────────┘
```

---

## Performance Comparison

| Metric | Original (Quickshell QML + Python) | Migrated (Standalone Rust) |
| :--- | :--- | :--- |
| **Idle Memory (RSS)** | **~965 MB** (shared with shell) | **~8–12 MB** |
| **Idle CPU** | **~15%** | **0.0%** |
| **Cold Startup Latency** | ~30–80 ms (Python spawn) | **< 1 ms** |
| **Frame Rate** | 60 FPS (Qt Quick scenegraph) | **125 FPS** (8ms frame ticker) |
| **External Dependencies** | Python 3, Qt6, Quickshell | Pure compiled static binary |

---

## Features

- **5 Dynamic Context Modes**: Automatically adapts dial slices to the focused window:
  - **Desktop / Default**: Launchers, scratchpad, wallpapers, system tools.
  - **Terminal (`kitty`, `alacritty`, `foot`, `konsole`)**: New window, clear terminal, open CWD in file manager, launch CLI agents.
  - **Browser (`firefox`, `zen`, `chrome`, `brave`)**: Live browser tabs, new tab, duplicate tab, reopen closed tab.
  - **Code Editor (`code`, `cursor`, `nvim`)**: Command palette, embedded terminal, git status, code formatting, run build.
  - **Media Player (`mpv`, `spotify`, `vlc`)**: Play/pause, next/previous track, volume adjustment.
- **Hierarchical Sub-Rings**:
  - **Active Windows**: Lists all running client windows with workspace indicators.
  - **Clipboard History**: Query and paste recent `cliphist` clips with UTF-8 preview.
  - **Audio Sinks**: Switch default audio output sink using PipeWire / `wpctl`.
  - **Browser Tabs**: Real-time browser tab switching via WebExtension shared-memory sync.
  - **File Jump**: Configurable quick-jump destinations with in-dial folder picker.
  - **Scratchpad**: Send window to special workspace slots (1–8).
- **Gestures & Controls**:
  - **Hold-and-Flick**: Hold `Super+Tab`, flick towards a slice, release within 80ms to trigger without releasing cursor.
  - **Sticky Mode**: Tap `Super+Tab` quickly to keep menu open until clicked or dismissed.
  - **Mouse Wheel**: Adjust volume over audio slices or brightness over display slices.
  - **Drag-to-Reorder**: Drag any slice to reorder slots on the fly; configuration saves automatically.
  - **Hotkey Badges**: Press `1`–`9` to immediately trigger corresponding slices.
  - **In-Dial Customizer**: Right-click any slice to open the in-menu slice swap and target editor.
  - **Folder Browser**: Built-in modal directory navigator with block device detection and `udisksctl` auto-mounting.

---

## Geometry & Visuals

- Slices rendered as floating concentric arcs with uniform parallel Euclidean gaps (`8.5 px`).
- Physical spring ripple displacement on hover: hovered slice expands radially by 7px and widens by 3.2°, while neighbor slices push outward in a spring wave.
- Material Symbols Rounded font glyph rendering with automatic font discovery from system and user font paths.

---

## Installation

### Prerequisites
- Linux with Wayland compositor (Hyprland recommended)
- Rust toolchain (`cargo`, `rustc` 1.80+)
- System utilities: `socat`, `cliphist`, `wpctl` (PipeWire), `wtype`, `grim`, `slurp`

### Build & Install
```bash
git clone https://github.com/your-username/radial-dial-rust.git
cd radial-dial-rust
./install.sh
```

The installer will:
1. Compile `radial-dial` and `radial-tabs-host` in release mode.
2. Install binaries to `~/.local/bin/`.
3. Register the browser extension native messaging host manifest.
4. Set up systemd user service `radial-dial.service`.

### Enable the Daemon
```bash
systemctl --user enable --now radial-dial.service
```

---

## Hyprland Keybind Configuration

Add this binding to `~/.config/hypr/hyprland/keybinds.lua` (or `hyprland.conf`):

```lua
-- Trigger radial dial at current cursor position
hl.bind("SUPER + Tab", hl.dsp.exec_cmd("radial-dial toggle"),
    { description = "Radial Dial: Toggle at cursor" })

-- Emergency failsafe: immediately kill all radial dial processes
hl.bind("SUPER + ALT + grave", hl.dsp.exec_cmd("radial-nuke"),
    { description = "Radial Dial: Emergency failsafe kill" })
```

---

## Configuration

Settings are saved in `~/.config/radialMenu/config.json` (100% backward-compatible with the QML schema):

```json
{
  "globalSlices": ["scratchpad", "terminal", "wallpapers", "btop", "session", "filejump", "calc", "active_apps"],
  "kittySlices": ["scratchpad", "kitty_new_window", "kitty_agy", "kitty_clear", "kitty_dolphin", "active_apps"],
  "browserSlices": ["scratchpad", "browsertabs", "browser_new_tab", "browser_close_tab", "browser_dup_tab", "browser_reopen_tab", "active_apps"],
  "fileJumpTargets": [
    { "id": "downloads", "label": "Downloads", "path": "~/Downloads", "icon": "download" },
    { "id": "documents", "label": "Documents", "path": "~/Documents", "icon": "description" },
    { "id": "pictures", "label": "Pictures", "path": "~/Pictures", "icon": "photo" },
    { "id": "music", "label": "Music", "path": "~/Music", "icon": "music_note" },
    { "id": "home", "label": "Home", "path": "~", "icon": "home" },
    { "id": "temp", "label": "Temp", "path": "/tmp", "icon": "folder_delete" }
  ]
}
```

---

## Testing

Run the full automated test suite:

```bash
cargo test
```

All 146 unit and integration tests covering geometry math, easing curves, context resolution, config persistence, IPC, input hit-testing, and 2D canvas rendering should pass cleanly.
