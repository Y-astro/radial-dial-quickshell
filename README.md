# Radial Dial for Hyprland & Quickshell

> A modern, GPU-accelerated, context-aware radial menu for **Hyprland** and **Quickshell** (`end4-pC` / *Illogical Impulse*).

---

## ✨ Features

- 🎯 **Context-Aware Dynamic Menus**: Automatically identifies the active window underneath the cursor and adapts the dial:
  - **Browser Context (Firefox, Zen, Chrome, etc.)**: 
    - ⚡ **Real-Time Tab Switcher**: Sub-radial ring displaying all currently open browser tabs with full webpage titles on hover and instant `Alt+1..9` tab switching.
    - 🆕 **Tab Operations**: New Tab, Close Tab, Duplicate Tab, and Reopen Closed Tab.
    - 📥 **Scratchpad Manager**: Move browser windows to/from `special:special`.
  - **Kitty / Terminal Context**: Scratchpad Manager, New Window (same directory & workspace), Clear Terminal (`Ctrl + L`), Run `agy`, and Open CWD in Dolphin file manager.
  - **Global Desktop Context**: Scratchpad Manager, Terminal, Wallpaper Selector, System Monitor (`btop`), Session Power/Lock Menu, File Jump (Quick-copy to Downloads, Documents, Home, Temp), and Calculator.
- 🚀 **Floating Rounded Wedges**: Segmented floating slices with smooth 6px rounded corners and radial gaps separating the inner hub from the outer wedges.
- 🌊 **Organic Ripple Physics**: Active hovered slices expand by $+7\text{px}$ in radius and widen by $+6.4^\circ$, smoothly displacing adjacent slices in a continuous physics ripple wave.
- 🪄 **Fluid Blossom Entrance Animation**: Center hub pops up with an energetic spring bounce, followed by outer slices blossoming clockwise around the clock. Automatically adapts to any slice count.
- 🪟 **True GPU Frosted Glass Blur**: Uses compositor-level Hyprland shader blur across active windows and wallpapers (`xray = false`, `ignore_alpha = 0.15`).
- 📥 **Scratchpad & Workspace Routing**: Send active windows directly to Hyprland's `special` workspace (toggled with `Super + S`), or expand the 8-workspace routing ring to send scratchpad windows back to any desktop.
- ⚡ **Ultra-Low Resource Usage**: Built on GPU `FramebufferObject` rendering with zero CPU shadow bottlenecks.

---

## 📦 Quick Installation

Clone the repository and run the installer:

```bash
git clone https://github.com/Y-astro/radial-dial-hyprland-end4-pC.git
cd radial-dial-hyprland-end4-pC
chmod +x install.sh
./install.sh
```

### Keybinding
Once installed, press **`Super + Tab`** anywhere on your desktop or over any window to open the radial dial.

---

## 🌐 Real-Time Browser Tab Sync (0ms Latency)

For instantaneous (0ms) browser tab sync in Firefox / Zen / Librewolf:

1. Open Firefox and go to: `about:debugging#/runtime/this-firefox`
2. Click **"Load Temporary Add-on..."**
3. Select `manifest.json` located in `modules/ii/radialMenu/extension/manifest.json`.

---

## 🛠️ Manual Installation

If you prefer to install manually into your `~/.config/quickshell/end4-pC/` setup:

1. **Copy Module**:
   ```bash
   cp -r modules/ii/radialMenu ~/.config/quickshell/end4-pC/modules/ii/
   chmod +x ~/.config/quickshell/end4-pC/modules/ii/radialMenu/*.py
   ```

2. **Add Hyprland Layer Rules** (`~/.config/hypr/hyprland/rules.lua`):
   ```lua
   hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, blur = true })
   hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, ignore_alpha = 0.15 })
   hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, xray = false })
   hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, no_anim = true })
   ```

3. **Add Hyprland Keybinding** (`~/.config/hypr/hyprland/keybinds.lua`):
   ```lua
   hl.bind("SUPER + Tab", hl.dsp.global("quickshell:radialMenu"), { description = "Shell: Open radial menu at cursor" })
   ```

4. **Instantiate in Quickshell** (`panelFamilies/IllogicalImpulseFamily.qml`):
   ```qml
   import qs.modules.ii.radialMenu

   // Inside the component:
   RadialMenu {}
   ```

5. **Reload**:
   ```bash
   hyprctl reload
   killall qs quickshell && qs -c end4-pC -d
   ```

---

## ⚙️ Customization

All menu actions, contexts, shortcuts, and ring geometries can be customized in [`RadialMenuActions.qml`](modules/ii/radialMenu/RadialMenuActions.qml) and [`RadialMenuContent.qml`](modules/ii/radialMenu/RadialMenuContent.qml).

---

## 📄 License

MIT License © [Y-astro](https://github.com/Y-astro)
