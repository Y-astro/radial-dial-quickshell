# Radial Dial for Quickshell

A GPU-accelerated, context-aware radial dial menu for Quickshell and Hyprland.

---

## Features

- **Interactive In-Menu Customizer**: Right-click any slice to open the configuration modal:
  - Select from 26+ built-in actions across Apps, Tools, Media, Screen Capture, Window Management, and System Session.
  - Active indicators show which actions are mapped to the active dial.
  - Add or remove slices dynamically per context dial.
  - File Jump editor: add, edit, or remove folder destinations with native directory browsing and automatic disk partition mounting via udisksctl.
  - Atomically persists layout and actions to `~/.config/radialMenu/config.json`.
- **Drag-to-Reorder Layout**: Click and drag any segment around the dial to reposition slices in real time. Slices highlight drop targets with preview indicators and dynamic slot badges.
- **Dynamic Number Hotkeys (1-9)**: Visual number badges on each wedge map to keyboard shortcuts 1 through 9. Hotkeys automatically synchronize whenever slices are reordered, added, or removed.
- **Universal Quickshell Compatibility**: Runs on both end4-pC dotfiles and standard standalone Quickshell configurations with built-in fallbacks for colors, fonts, and cursors.
- **Context-Aware Dial Modes**: Automatically inspects the active window under the cursor:
  - **Browser Mode (Firefox, Zen, Chrome)**: Real-time tab switching with tab titles, tab close/new/duplicate operations, and scratchpad toggling.
  - **Terminal Mode (Kitty)**: Scratchpad toggle, new terminal in current working directory, terminal clear, AI CLI, and file manager navigation.
  - **Global Desktop Mode**: Scratchpad toggle, terminal launcher, wallpaper picker, system monitor, session controls, file jump, and calculator.
- **Visual Design**:
  - Segmented floating wedges with 6px rounded corners and radial gaps.
  - Physics ripple effect expanding hovered slices by +7px and +6.4 degrees.
  - Spring-animated blossom entrance and outside-in exit animations.
  - True compositor-level frosted glass blur on Hyprland.
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

The installer detects your Quickshell configuration path, copies required modules, configures Hyprland layer rules, binds the shortcut, and registers the browser tab synchronization native messaging host.

### Default Keybinding

Press `Super + Tab` anywhere on your desktop or over any window to toggle the radial dial.

---

## Usage and Customization

- **Execute Action**: Left-click any slice or press its corresponding number key (1 through 9).
- **Open Sub-Ring**: Left-click slices with sub-tiers (such as File Jump or Browser Tabs) to expand localized outer petals.
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

2. Add Hyprland layer rules in `~/.config/hypr/hyprland/rules.lua`:
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
