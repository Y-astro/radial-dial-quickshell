# Radial Dial for Quickshell

A context-aware radial dial menu for Quickshell and Hyprland with automatic GPU-aware rendering.

---

## Features

- **Interactive In-Menu Customizer**: Right-click any slice to open the configuration modal:
  - Select from 30+ built-in actions across Apps, Tools, Media, Screen Capture, Window Management, and System Session.
  - Active indicators show which actions are mapped to the active dial.
  - Add or remove slices dynamically per context dial.
  - File Jump editor: add, edit, or remove folder destinations with native directory browsing and automatic disk partition mounting via udisksctl.
  - Atomically persists layout and actions to `~/.config/radialMenu/config.json`.
- **Hold-and-Flick Release Activation**: Hold `Super + Tab`, flick the cursor toward any segment, and release the key to trigger the action in under 80ms. If tapped without flicking, or when using the customizer or folder picker, the dial remains open in sticky interactive mode.
- **Active Apps Switcher**: Dedicated segment present across all dials to view and switch to any running application across all workspaces. Multi-instance apps show each window instance distinctly with workspace badges and full window titles on hover.
- **Drag-to-Reorder Layout**: Click and drag any segment around the dial to reposition slices in real time. Slices highlight drop targets with preview indicators and dynamic slot badges.
- **Dynamic Number Hotkeys (1-9)**: Visual number badges on each wedge map to keyboard shortcuts 1 through 9. Hotkeys automatically synchronize whenever slices are reordered, added, or removed.
- **Mouse Wheel Scrubbing on Slices**: Hover over volume or brightness slices and scroll the mouse wheel to smoothly adjust levels without clicking.
- **Direct UNIX Socket IPC**: Sub-millisecond window and cursor queries through direct connection to the Hyprland UNIX socket, eliminating process fork overhead.
- **Optional High-Frequency Tools (Customizer)**:
  - Clipboard History: sub-ring displaying recent cliphist snippets with instant paste.
  - Audio Output Switcher: PipeWire sub-ring to switch between speakers, headphones, and Bluetooth.
  - Color Picker: inspect on-screen pixels with hyprpicker and copy HEX color code.
  - Screen Snip: interactive region capture directly to clipboard.
  - Screen OCR: optical character recognition to extract unselectable text from screen.
- **Context-Aware Dial Modes**: Automatically inspects the active window under the cursor:
  - **Browser Mode (Firefox, Zen, Chrome)**: Real-time tab switching with tab titles, tab close/new/duplicate operations, and scratchpad toggling.
  - **Terminal Mode (Kitty)**: Scratchpad toggle, new terminal in current working directory, terminal clear, AI CLI, and file manager navigation.
  - **Code Editor Mode (VS Code, Cursor, Neovim)**: Command palette, terminal toggle, Git changes, format document, and run file.
  - **Media Player Mode (MPV, Spotify)**: Play/pause, track navigation, volume adjustments, and audio output switching.
  - **Global Desktop Mode**: Scratchpad toggle, terminal launcher, wallpaper picker, system monitor, session controls, file jump, active apps, and calculator.
- **Visual Design**:
  - Segmented floating wedges with 6px rounded corners and radial gaps.
  - Physics ripple effect expanding hovered slices by +7px and +6.4 degrees.
  - Spring-animated blossom entrance and outside-in exit animations.
  - Optional compositor-level frosted glass blur on Hyprland.
- **Scratchpad Routing**: Direct window routing to Hyprland's special workspace, with an 8-workspace destination ring to return scratchpad windows to specific workspaces.

---

## Quick Installation

Run the automated installer:

```bash
git clone https://github.com/Y-astro/radial-dial-quickshell.git
cd radial-dial-quickshell
chmod +x install.sh
./install.sh
```

The installer detects your Quickshell configuration path, copies the module, binds the shortcut, and registers the browser tab synchronization native messaging host. The radial menu owns its own state and shortcut handler; it does not patch the host's `GlobalStates.qml`.

Rendering mode is selected automatically. NVIDIA GPUs, Intel Arc GPUs, and AMD GPUs with at least 2 GiB of dedicated VRAM use the framebuffer renderer with compositor blur. Integrated or unknown graphics use the threaded image renderer without blur. This keeps the expensive fullscreen blur away from low-power systems while preserving the full visual mode on suitable hardware.

### Default Keybinding

Press `Super + Tab` anywhere on your desktop or over any window to toggle the radial dial.

---

## Usage and Controls

- **Hold and Flick**: Press and hold `Super + Tab`, flick mouse toward a slice, and release the key to execute immediately.
- **Sticky Mode**: Tap `Super + Tab` without moving the mouse to keep the menu open.
- **Execute Action**: Left-click any slice or press its corresponding number key (1 through 9).
- **Wheel Scrubbing**: Hover over volume, brightness, or media slices and scroll the mouse wheel to adjust values.
- **Open Sub-Ring**: Left-click slices with sub-tiers (Active Apps, File Jump, Browser Tabs, Clipboard, Audio Output) to expand localized outer petals.
- **Reorder Slices**: Left-click and hold a slice, drag it to the desired position, and release.
- **Configure Slice**: Right-click any slice to open the action customizer.
- **Add / Remove Slices**: In the customizer, use + to append a new slice or - to remove an active one.
- **Cancel / Close**: Press Escape or click the center hub button to close the menu.

---

## Browser Tab Synchronization (Optional)

For zero-latency browser tab switching in Firefox, Zen, or LibreWolf:

1. Navigate to `about:debugging#/runtime/this-firefox` in your browser.
2. Click "Load Temporary Add-on...".
3. Select `manifest.json` located at `modules/ii/radialMenu/extension/manifest.json`.

---

## Manual Installation

To install manually into `~/.config/quickshell/end4-pC/`:

1. Copy the module:
   ```bash
   cp -r modules/ii/radialMenu ~/.config/quickshell/end4-pC/modules/ii/
   chmod +x ~/.config/quickshell/end4-pC/modules/ii/radialMenu/*.py
   ```

2. Optionally add Hyprland layer rules in `~/.config/hypr/hyprland/rules.lua` if you want compositor blur:
   ```lua
   hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, blur = true })
   hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, ignore_alpha = 0.15 })
   hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, xray = false })
   hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, no_anim = true })
   ```

3. Bind the shortcut in `~/.config/hypr/hyprland/keybinds.lua`:
   ```lua
   hl.bind("SUPER + Tab", hl.dsp.global("quickshell:radialMenu"), { description = "Shell: Open radial menu at cursor" })
   ```

4. Instantiate the component in `panelFamilies/IllogicalImpulseFamily.qml`:
   ```qml
   import qs.modules.ii.radialMenu

   RadialMenu {}
   ```

---

## Configuration File

Customizations are stored in JSON format at:
```
~/.config/radialMenu/config.json
```
Deleting this file restores default slices and settings.

---

## License

MIT License.
