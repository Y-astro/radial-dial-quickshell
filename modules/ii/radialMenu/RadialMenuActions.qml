pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs
import qs.modules.common

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenuActions — Context resolver and action dispatcher for Radial Menu v2
// ─────────────────────────────────────────────────────────────────────────────
Singleton {
    id: root

    // Execute arbitrary bash command detached
    function exec(cmd: string) {
        Quickshell.execDetached(["bash", "-c", cmd])
    }

    // Determine context based on active window properties
    function resolveContext(win) {
        if (!win) return "default"
        const c = String(win.class || win.initialClass || "").toLowerCase()
        const title = String(win.title || "").toLowerCase()
        if (c.includes("kitty") || c.includes("konsole") || c.includes("alacritty") || c.includes("foot")) return "kitty"
        if (/firefox|brave|chrome|chromium|thorium|zen|floorp|vivaldi|opera|edge|librewolf|waterfox/.test(c) || /firefox|brave|chrome|chromium|zen|librewolf/.test(title)) return "browser"
        return "default"
    }

    // Check if focused window is in a special workspace (e.g. special:scratchpad)
    function isInSpecialWorkspace(win) {
        if (!win || !win.workspace) return false
        const wsName = win.workspace.name || ""
        const wsId = win.workspace.id ?? 0
        return wsName.startsWith("special") || wsId < 0
    }

    // Send keystroke reliably to a target window
    function sendBrowserKey(winAddress: string, keyCmd: string) {
        const addr = winAddress ? `address:${winAddress}` : ""
        const focusCmd = addr ? `hyprctl dispatch focuswindow "${addr}" 2>/dev/null; ` : ""
        const cmd = `${focusCmd}sleep 0.05; ${keyCmd}`
        exec(cmd)
    }

    // Switch to browser tab by index (1-based)
    function switchToBrowserTab(tabIdx: int, winAddress: string) {
        let keyCmd = ""
        if (tabIdx >= 1 && tabIdx <= 8) {
            keyCmd = `wtype -M alt -k ${tabIdx} -m alt`
        } else if (tabIdx === 9) {
            keyCmd = "wtype -M alt -k 9 -m alt"
        } else {
            keyCmd = "wtype -M alt -k 1 -m alt"
            for (let i = 1; i < tabIdx; i++) {
                keyCmd += " && sleep 0.04 && wtype -M ctrl -k Tab -m ctrl"
            }
        }
        sendBrowserKey(winAddress, keyCmd)
    }

    // Resolve Terminal (Kitty/Konsole) CWD and open Dolphin
    function openTerminalCwd(pid) {
        const cmd = `
            PID="${pid || 0}"
            TARGET_PID="$PID"
            while true; do
                NEXT_PID=$(pgrep -P "$TARGET_PID" 2>/dev/null | tail -n 1)
                if [ -n "$NEXT_PID" ] && [ -d "/proc/$NEXT_PID/cwd" ]; then
                    TARGET_PID="$NEXT_PID"
                else
                    break
                fi
            done
            CWD=$(readlink -f "/proc/$TARGET_PID/cwd" 2>/dev/null || echo "$HOME")
            dolphin "$CWD" &
        `
        exec(cmd)
    }

    // Spawn a new Kitty window inheriting the current working directory in the active workspace
    function openTerminalNewWindow(pid) {
        const cmd = `
            PID="${pid || 0}"
            TARGET_PID="$PID"
            while true; do
                NEXT_PID=$(pgrep -P "$TARGET_PID" 2>/dev/null | tail -n 1)
                if [ -n "$NEXT_PID" ] && [ -d "/proc/$NEXT_PID/cwd" ]; then
                    TARGET_PID="$NEXT_PID"
                else
                    break
                fi
            done
            CWD=$(readlink -f "/proc/$TARGET_PID/cwd" 2>/dev/null || echo "$HOME")
            kitty --directory "$CWD" &
        `
        exec(cmd)
    }

    // File Jump handler: copy/move URIs from clipboard if present, or open folder in Dolphin
    function performFileJump(destPath: string) {
        const script = `
import os, shutil, sys, urllib.parse, subprocess
dest = os.path.expanduser("${destPath}")
os.makedirs(dest, exist_ok=True)
try:
    p = subprocess.run(['wl-paste', '-t', 'text/uri-list'], capture_output=True, text=True, timeout=1)
    uris = [line.strip() for line in p.stdout.splitlines() if line.strip().startswith('file://')]
except Exception:
    uris = []

if uris:
    copied = 0
    for uri in uris:
        raw_path = urllib.parse.unquote(uri[7:])
        if os.path.exists(raw_path):
            basename = os.path.basename(raw_path)
            target_path = os.path.join(dest, basename)
            try:
                if os.path.isdir(raw_path):
                    shutil.copytree(raw_path, target_path, dirs_exist_ok=True)
                else:
                    shutil.copy2(raw_path, target_path)
                copied += 1
            except Exception:
                pass
    if copied > 0:
        subprocess.run(['notify-send', '-a', 'Radial Menu', 'File Jump', f'Copied {copied} item(s) to {dest}'])
    else:
        subprocess.run(['dolphin', dest])
else:
    subprocess.run(['dolphin', dest])
`
        Quickshell.execDetached(["python3", "-c", script])
    }

    // Launch btop in floating terminal
    function launchBtop() {
        exec("kitty -e btop &")
    }

    // Launch kitty terminal
    function launchKitty() {
        exec("kitty &")
    }

    // Launch calculator
    function launchCalculator() {
        exec("kcalc || qalculate-gtk &")
    }

    // Launch file search
    function launchFileSearch() {
        exec("fuzzel || krunner &")
    }

    // Send active window to default special workspace (Super+S) via Hyprland Lua dispatch
    function sendToScratchpad() {
        Hyprland.dispatch("hl.dsp.window.move({ workspace = 'special:special' })")
    }

    // Move window to specified workspace via Hyprland Lua dispatch
    function moveToWorkspace(wsNumber) {
        Hyprland.dispatch(`hl.dsp.window.move({ workspace = ${wsNumber} })`)
    }

    // Scratchpad slice generator (context-aware: send to special or expand 8-workspace ring)
    function getScratchpadSlice(win) {
        const inSpecial = isInSpecialWorkspace(win)
        return {
            label: inSpecial ? "Move from Scratchpad" : "To Scratchpad",
            icon: inSpecial ? "unarchive" : "move_to_inbox",
            hasSubTier: inSpecial,
            subTierType: "scratchpad",
            action: inSpecial ? null : () => sendToScratchpad()
        }
    }

    // Get slices for current state
    function getSlicesFor(tier: string, context: string, win) {
        if (tier === "scratchpad") {
            return [
                { label: "Workspace 1", icon: "counter_1", action: () => moveToWorkspace(1) },
                { label: "Workspace 2", icon: "counter_2", action: () => moveToWorkspace(2) },
                { label: "Workspace 3", icon: "counter_3", action: () => moveToWorkspace(3) },
                { label: "Workspace 4", icon: "counter_4", action: () => moveToWorkspace(4) },
                { label: "Workspace 5", icon: "counter_5", action: () => moveToWorkspace(5) },
                { label: "Workspace 6", icon: "counter_6", action: () => moveToWorkspace(6) },
                { label: "Workspace 7", icon: "counter_7", action: () => moveToWorkspace(7) },
                { label: "Workspace 8", icon: "counter_8", action: () => moveToWorkspace(8) }
            ]
        }

        if (tier === "browsertabs") {
            const winAddr = win?.address || ""
            const rawTabs = GlobalStates.browserTabsList
            if (rawTabs && Array.isArray(rawTabs) && rawTabs.length > 0) {
                return rawTabs.map((t, i) => {
                    const idx = t.index || (i + 1)
                    return {
                        label: t.title || `Tab ${idx}`,
                        icon: idx <= 8 ? `counter_${idx}` : "tab",
                        hasSubTier: false,
                        action: () => switchToBrowserTab(idx, winAddr)
                    }
                })
            }
            return [
                { label: "Tab 1", icon: "counter_1", action: () => switchToBrowserTab(1, winAddr) },
                { label: "Tab 2", icon: "counter_2", action: () => switchToBrowserTab(2, winAddr) },
                { label: "Tab 3", icon: "counter_3", action: () => switchToBrowserTab(3, winAddr) },
                { label: "Tab 4", icon: "counter_4", action: () => switchToBrowserTab(4, winAddr) },
                { label: "Tab 5", icon: "counter_5", action: () => switchToBrowserTab(5, winAddr) },
                { label: "Tab 6", icon: "counter_6", action: () => switchToBrowserTab(6, winAddr) },
                { label: "Tab 7", icon: "counter_7", action: () => switchToBrowserTab(7, winAddr) },
                { label: "Tab 8", icon: "counter_8", action: () => switchToBrowserTab(8, winAddr) }
            ]
        }

        if (tier === "filejump") {
            return [
                { label: "Downloads", icon: "download",       action: () => performFileJump("~/Downloads") },
                { label: "Documents", icon: "description",    action: () => performFileJump("~/Documents") },
                { label: "Home",      icon: "home",           action: () => performFileJump("~") },
                { label: "Temp",      icon: "folder_delete",  action: () => performFileJump("/tmp") }
            ]
        }

        // Kitty / Terminal context (with Scratchpad Manager)
        if (context === "kitty") {
            return [
                getScratchpadSlice(win),
                {
                    label: "New Window",
                    icon: "open_in_new",
                    action: () => openTerminalNewWindow(win?.pid)
                },
                {
                    label: "Run agy",
                    icon: "robot_2",
                    action: () => exec("sleep 0.05 && wtype 'agy --dangerously-skip-permissions' -k Return")
                },
                {
                    label: "Clear Terminal",
                    icon: "mop",
                    action: () => exec("sleep 0.05 && wtype -M ctrl -k l -m ctrl")
                },
                {
                    label: "Open CWD in Dolphin",
                    icon: "folder_open",
                    action: () => openTerminalCwd(win?.pid)
                }
            ]
        }

        // Browser context (with Scratchpad Manager & Tabs ring)
        if (context === "browser") {
            const winAddr = win?.address || ""
            const tabCount = GlobalStates.browserTabsList ? GlobalStates.browserTabsList.length : 0
            const tabSliceLabel = tabCount > 0 ? `Switch Tab (${tabCount})` : "Switch Tab"
            return [
                getScratchpadSlice(win),
                {
                    label: tabSliceLabel,
                    icon: "tabs",
                    hasSubTier: true,
                    subTierType: "browsertabs",
                    action: null
                },
                {
                    label: "New Tab",
                    icon: "tab",
                    action: () => sendBrowserKey(winAddr, "wtype -M ctrl -k t -m ctrl")
                },
                {
                    label: "Close Tab",
                    icon: "tab_close",
                    action: () => sendBrowserKey(winAddr, "wtype -M ctrl -k w -m ctrl")
                },
                {
                    label: "Duplicate Tab",
                    icon: "tab_duplicate",
                    action: () => sendBrowserKey(winAddr, "wtype -M alt -k d -m alt && sleep 0.06 && wtype -M alt -k Return -m alt")
                },
                {
                    label: "Reopen Closed Tab",
                    icon: "history",
                    action: () => sendBrowserKey(winAddr, "wtype -M ctrl -M shift -k t -m shift -m ctrl")
                }
            ]
        }

        // Global / Default context
        return [
            getScratchpadSlice(win),
            {
                label: "Terminal",
                icon: "terminal",
                action: () => launchKitty()
            },
            {
                label: "Wallpapers",
                icon: "wallpaper",
                action: () => {
                    GlobalStates.wallpaperSelectorTarget = "wallpaper"
                    GlobalStates.wallpaperSelectorOpen = true
                }
            },
            {
                label: "Launch btop",
                icon: "monitoring",
                action: () => launchBtop()
            },
            {
                label: "Session",
                icon: "power_settings_new",
                action: () => {
                    GlobalStates.sessionOpen = true
                }
            },
            {
                label: "File Jump",
                icon: "drive_file_move",
                hasSubTier: true,
                subTierType: "filejump",
                action: null
            },
            {
                label: "Calculator",
                icon: "calculate",
                action: () => launchCalculator()
            }
        ]
    }
}
