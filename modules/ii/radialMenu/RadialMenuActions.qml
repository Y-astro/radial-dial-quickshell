pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenuActions — Context resolver, function catalogue & persistent config
// Compatible with any Quickshell configuration & Hyprland
// ─────────────────────────────────────────────────────────────────────────────
Singleton {
    id: root

    signal configChanged()
    signal directoryPicked(string path)

    readonly property string ipcScriptPath: {
        const u = Qt.resolvedUrl("hypr_ipc.py").toString()
        return u.startsWith("file://") ? u.substring(7) : u
    }

    property var cachedClips: []
    property var cachedSinks: []
    property var cachedClients: []

    Process {
        id: fetchClipsProc
        command: ["python3", root.ipcScriptPath, "clipboard"]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                try { root.cachedClips = JSON.parse(text.trim()) } catch(e) {}
            }
        }
    }
    function refreshClipboard() { fetchClipsProc.running = true }

    Process {
        id: fetchSinksProc
        command: ["python3", root.ipcScriptPath, "audio_sinks"]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                try { root.cachedSinks = JSON.parse(text.trim()) } catch(e) {}
            }
        }
    }
    function refreshAudioSinks() { fetchSinksProc.running = true }

    Process {
        id: fetchClientsProc
        command: ["python3", root.ipcScriptPath, "clients"]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                try { root.cachedClients = JSON.parse(text.trim()) } catch(e) {}
            }
        }
    }
    function refreshClients() { fetchClientsProc.running = true }

    function focusWindow(addr: string) {
        if (!addr) return
        try {
            Hyprland.dispatch('hl.dsp.focus({ window = "address:' + addr + '" })')
        } catch(e) {}
        try {
            Hyprland.dispatch('focuswindow address:' + addr)
        } catch(e) {}
    }

    function resolveAppIcon(appClass: string) {
        const c = (appClass || "").toLowerCase()
        if (c.includes("firefox") || c.includes("zen") || c.includes("chrome") || c.includes("chromium") || c.includes("brave") || c.includes("browser")) return "globe"
        if (c.includes("kitty") || c.includes("terminal") || c.includes("alacritty") || c.includes("foot") || c.includes("konsole") || c.includes("xterm")) return "terminal"
        if (c.includes("code") || c.includes("cursor") || c.includes("antigravity") || c.includes("nvim") || c.includes("studio") || c.includes("dev")) return "code"
        if (c.includes("discord") || c.includes("vesktop") || c.includes("telegram") || c.includes("slack") || c.includes("whatsapp")) return "chat"
        if (c.includes("spotify") || c.includes("music") || c.includes("rhythmbox")) return "music_note"
        if (c.includes("dolphin") || c.includes("nautilus") || c.includes("thunar") || c.includes("nemo") || c.includes("file")) return "folder"
        if (c.includes("mpv") || c.includes("vlc") || c.includes("video") || c.includes("media")) return "movie"
        if (c.includes("calc") || c.includes("calculator") || c.includes("kcalc")) return "calculate"
        if (c.includes("settings") || c.includes("control")) return "settings"
        return "window"
    }

    function adjustVolume(deltaPercent: int) {
        const sign = deltaPercent >= 0 ? "+" : "-"
        const mag = Math.abs(deltaPercent)
        exec("wpctl set-volume @DEFAULT_AUDIO_SINK@ " + mag + "%" + sign + " || pactl set-sink-volume @DEFAULT_SINK@ " + sign + mag + "%")
    }

    function adjustBrightness(deltaPercent: int) {
        const sign = deltaPercent >= 0 ? "+" : "-"
        const mag = Math.abs(deltaPercent)
        exec("brightnessctl s " + mag + "%" + sign)
    }

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
        if (/code|cursor|antigravity|vscodium|nvim|neovim/.test(c)) return "code"
        if (/mpv|spotify|vlc/.test(c)) return "media"
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
        const addr = winAddress ? 'address:' + winAddress : ''
        const focusCmd = addr ? 'hyprctl dispatch focuswindow "' + addr + '" 2>/dev/null; ' : ''
        const cmd = focusCmd + 'sleep 0.05; ' + keyCmd
        exec(cmd)
    }

    // Switch to browser tab by index (1-based)
    function switchToBrowserTab(tabIdx: int, winAddress: string) {
        let keyCmd = ''
        if (tabIdx >= 1 && tabIdx <= 8) {
            keyCmd = 'wtype -M alt -k ' + tabIdx + ' -m alt'
        } else if (tabIdx === 9) {
            keyCmd = 'wtype -M alt -k 9 -m alt'
        } else {
            keyCmd = 'wtype -M alt -k 1 -m alt'
            for (let i = 1; i < tabIdx; i++) {
                keyCmd += ' && sleep 0.04 && wtype -M ctrl -k Tab -m ctrl'
            }
        }
        sendBrowserKey(winAddress, keyCmd)
    }

    // Resolve Terminal (Kitty/Konsole) CWD and open default file manager
    function openTerminalCwd(pid) {
        const p = pid || 0
        const cmd = 'PID="' + p + '"; TARGET_PID="$PID"; while true; do NEXT_PID=$(pgrep -P "$TARGET_PID" 2>/dev/null | tail -n 1); if [ -n "$NEXT_PID" ] && [ -d "/proc/$NEXT_PID/cwd" ]; then TARGET_PID="$NEXT_PID"; else break; fi; done; CWD=$(readlink -f "/proc/$TARGET_PID/cwd" 2>/dev/null || echo "$HOME"); (dolphin "$CWD" || xdg-open "$CWD" || nautilus "$CWD" || thunar "$CWD") &'
        exec(cmd)
    }

    // Spawn a new Kitty window inheriting the current working directory in the active workspace
    function openTerminalNewWindow(pid) {
        const p = pid || 0
        const cmd = 'PID="' + p + '"; TARGET_PID="$PID"; while true; do NEXT_PID=$(pgrep -P "$TARGET_PID" 2>/dev/null | tail -n 1); if [ -n "$NEXT_PID" ] && [ -d "/proc/$NEXT_PID/cwd" ]; then TARGET_PID="$NEXT_PID"; else break; fi; done; CWD=$(readlink -f "/proc/$TARGET_PID/cwd" 2>/dev/null || echo "$HOME"); (kitty --directory "$CWD" || alacritty --working-directory "$CWD" || foot -D "$CWD") &'
        exec(cmd)
    }

    // Universal File Jump handler: auto-mount removable drives if required, copy/move URIs from clipboard, or open folder
    function performFileJump(destPath: string) {
        const script = `
import os, shutil, sys, urllib.parse, subprocess
dest = os.path.expanduser("${destPath}")

# If destination path is on an unmounted removable/internal drive, attempt auto-mounting via udisksctl
if not os.path.isdir(dest):
    # Search for matching block devices if path is under /run/media or /media
    if "/media" in dest:
        parts = dest.split("/media/", 1)[-1].split("/")
        vol_name = parts[1] if len(parts) > 1 else parts[0]
        try:
            p = subprocess.run(["lsblk", "-rno", "NAME,LABEL,UUID"], capture_output=True, text=True)
            for line in p.stdout.splitlines():
                fields = line.strip().split()
                if len(fields) >= 2 and (vol_name in fields[1] or (len(fields) >= 3 and vol_name in fields[2])):
                    dev_node = "/dev/" + fields[0]
                    subprocess.run(["udisksctl", "mount", "-b", dev_node], capture_output=True, text=True, timeout=5)
                    break
        except Exception:
            pass

try:
    os.makedirs(dest, exist_ok=True)
except Exception:
    pass

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

# Always open file manager navigating to destination folder
try:
    subprocess.Popen(['dolphin', dest])
except Exception:
    try:
        subprocess.Popen(['xdg-open', dest])
    except Exception:
        pass
`
        Quickshell.execDetached(["python3", "-c", script])
    }

    // Launch btop in floating terminal
    function launchBtop() {
        exec("kitty -e btop || alacritty -e btop || foot btop &")
    }

    // Launch terminal
    function launchTerminal() {
        exec("kitty || alacritty || foot || konsole || xterm &")
    }

    // Launch calculator
    function launchCalculator() {
        exec("kcalc || qalculate-gtk || gnome-calculator &")
    }

    // Launch file search
    function launchFileSearch() {
        exec("fuzzel || krunner || rofi -show drun &")
    }

    // Send active window to default special workspace (Super+S) via Hyprland Lua dispatch
    function sendToScratchpad() {
        Hyprland.dispatch("hl.dsp.window.move({ workspace = 'special:special' })")
    }

    // Move window to specified workspace via Hyprland Lua dispatch
    function moveToWorkspace(wsNumber) {
        Hyprland.dispatch('hl.dsp.window.move({ workspace = ' + wsNumber + ' })')
    }

    // Scratchpad slice generator (context-aware: send to special or expand 8-workspace ring)
    function getScratchpadSlice(win) {
        const inSpecial = isInSpecialWorkspace(win)
        return {
            slotIndex: 0,
            functionId: "scratchpad",
            label: inSpecial ? "Move from Scratchpad" : "To Scratchpad",
            icon: inSpecial ? "unarchive" : "move_to_inbox",
            hasSubTier: inSpecial,
            subTierType: "scratchpad",
            action: inSpecial ? null : () => sendToScratchpad()
        }
    }

    // ── Function Registry: Comprehensive Catalogue of Available Functions ────
    readonly property var functionRegistry: ({
        // Apps & Launchers
        "terminal": {
            id: "terminal",
            label: "Terminal",
            icon: "terminal",
            category: "Apps",
            desc: "Open terminal emulator",
            action: () => launchTerminal()
        },
        "calc": {
            id: "calc",
            label: "Calculator",
            icon: "calculate",
            category: "Apps",
            desc: "Open calculator app",
            action: () => launchCalculator()
        },
        "filesearch": {
            id: "filesearch",
            label: "File Search",
            icon: "search",
            category: "Apps",
            desc: "Search files with launcher (fuzzel/krunner/rofi)",
            action: () => launchFileSearch()
        },
        "dolphin": {
            id: "dolphin",
            label: "File Manager",
            icon: "folder_open",
            category: "Apps",
            desc: "Open default file manager",
            action: () => exec("dolphin || nautilus || thunar || xdg-open ~ &")
        },
        "browser": {
            id: "browser",
            label: "Web Browser",
            icon: "globe",
            category: "Apps",
            desc: "Launch default web browser",
            action: () => exec("xdg-open 'https://' &")
        },
        "editor": {
            id: "editor",
            label: "Code Editor",
            icon: "code",
            category: "Apps",
            desc: "Open code editor (Code/Cursor/Neovim)",
            action: () => exec("code || cursor || kitty -e nvim || xdg-open ~ &")
        },
        "btop": {
            id: "btop",
            label: "Task Manager",
            icon: "monitoring",
            category: "Apps",
            desc: "Open Btop system resource monitor",
            action: () => launchBtop()
        },
        "settings": {
            id: "settings",
            label: "Settings",
            icon: "settings",
            category: "Apps",
            desc: "Open system settings",
            action: () => exec("systemsettings || gnome-control-center &")
        },

        // Window Management & Active Apps
        "active_apps": {
            id: "active_apps",
            label: "Active Apps",
            icon: "apps",
            category: "Window",
            desc: "Switch to any running application across all workspaces",
            hasSubTier: true,
            subTierType: "active_apps",
            action: null
        },

        // Category 1 Optional Features (Clipboard, Audio Output, Snip, OCR)
        "clipboard": {
            id: "clipboard",
            label: "Clipboard",
            icon: "content_paste",
            category: "Tools",
            desc: "Quick paste recent clipboard history snippets",
            hasSubTier: true,
            subTierType: "clipboard",
            action: null
        },
        "audio_sink": {
            id: "audio_sink",
            label: "Audio Output",
            icon: "volume_up",
            category: "Media",
            desc: "Switch default audio output device (Headphones/Speakers)",
            hasSubTier: true,
            subTierType: "audio_sink",
            action: null
        },
        "color_picker": {
            id: "color_picker",
            label: "Color Picker",
            icon: "palette",
            category: "Tools",
            desc: "Inspect on-screen color and copy HEX code to clipboard",
            action: () => exec("hyprpicker -a &")
        },
        "screen_snip": {
            id: "screen_snip",
            label: "Screen Snip",
            icon: "screenshot_region",
            category: "Capture",
            desc: "Interactive area screenshot directly to clipboard",
            action: () => exec("grim -g \"$(slurp)\" - | wl-copy && notify-send -a 'Radial Menu' 'Screenshot' 'Area copied to clipboard' &")
        },
        "screen_ocr": {
            id: "screen_ocr",
            label: "Screen OCR",
            icon: "document_scanner",
            category: "Tools",
            desc: "Optical character recognition: grab unselectable text from screen",
            action: () => exec("tmp=\"/tmp/ocr_$$\"; grim -g \"$(slurp)\" \"$tmp.png\" && tesseract \"$tmp.png\" \"$tmp\" -l eng 2>/dev/null && cat \"$tmp.txt\" | tr -d '\\f' | wl-copy && rm -f \"$tmp\"* && notify-send -a 'Radial Menu' 'Screen OCR' 'Recognized text copied to clipboard' &")
        },

        // Code Editor Functions (Category 3: 8)
        "code_git_status": {
            id: "code_git_status",
            label: "Git Changes",
            icon: "commit",
            category: "Tools",
            desc: "Open Git source control in editor",
            action: () => exec("wtype -M ctrl -M shift -k g -m shift -m ctrl &")
        },
        "code_terminal": {
            id: "code_terminal",
            label: "Editor Terminal",
            icon: "terminal",
            category: "Tools",
            desc: "Toggle integrated terminal in editor",
            action: () => exec("wtype -M ctrl -k grave -m ctrl &")
        },
        "code_format": {
            id: "code_format",
            label: "Format Document",
            icon: "format_align_left",
            category: "Tools",
            desc: "Format document with code formatter",
            action: () => exec("wtype -M shift -M alt -k f -m alt -m shift &")
        },
        "code_palette": {
            id: "code_palette",
            label: "Command Palette",
            icon: "terminal",
            category: "Tools",
            desc: "Open editor command palette",
            action: () => exec("wtype -M ctrl -M shift -k p -m shift -m ctrl &")
        },
        "code_run": {
            id: "code_run",
            label: "Run File",
            icon: "play_arrow",
            category: "Tools",
            desc: "Run file without debugging in editor",
            action: () => exec("wtype -M ctrl -k F5 -m ctrl &")
        },

        // Media Controls (Category 3: 9)
        "media_play_pause": {
            id: "media_play_pause",
            label: "Play / Pause",
            icon: "play_arrow",
            category: "Media",
            desc: "Toggle media playback",
            action: () => exec("playerctl play-pause &")
        },
        "media_next": {
            id: "media_next",
            label: "Next Track",
            icon: "skip_next",
            category: "Media",
            desc: "Skip to next media track",
            action: () => exec("playerctl next &")
        },
        "media_prev": {
            id: "media_prev",
            label: "Previous Track",
            icon: "skip_previous",
            category: "Media",
            desc: "Skip to previous media track",
            action: () => exec("playerctl previous &")
        },
        "media_mute": {
            id: "media_mute",
            label: "Mute Toggle",
            icon: "volume_off",
            category: "Media",
            desc: "Toggle audio mute",
            action: () => exec("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle &")
        },
        "volume_up": {
            id: "volume_up",
            label: "Volume +5%",
            icon: "volume_up",
            category: "Media",
            desc: "Increase master volume",
            action: () => adjustVolume(5)
        },
        "volume_down": {
            id: "volume_down",
            label: "Volume -5%",
            icon: "volume_down",
            category: "Media",
            desc: "Decrease master volume",
            action: () => adjustVolume(-5)
        },
        "brightness_up": {
            id: "brightness_up",
            label: "Brightness +",
            icon: "brightness_high",
            category: "Tools",
            desc: "Increase display brightness",
            action: () => adjustBrightness(5)
        },
        "brightness_down": {
            id: "brightness_down",
            label: "Brightness -",
            icon: "brightness_low",
            category: "Tools",
            desc: "Decrease display brightness",
            action: () => adjustBrightness(-5)
        },

        // Tools & Radial Features
        "filejump": {
            id: "filejump",
            label: "File Jump",
            icon: "drive_file_move",
            category: "Tools",
            desc: "Customizable quick folder jump & drop",
            hasSubTier: true,
            subTierType: "filejump",
            action: null
        },
        "wallpapers": {
            id: "wallpapers",
            label: "Wallpapers",
            icon: "wallpaper",
            category: "Tools",
            desc: "Open wallpaper selector",
            action: () => {
                if (typeof GlobalStates !== "undefined" && GlobalStates) {
                    GlobalStates.wallpaperSelectorTarget = "wallpaper"
                    GlobalStates.wallpaperSelectorOpen = true
                } else {
                    exec("waypaper || swww-daemon &")
                }
            }
        },
        "colorpicker": {
            id: "colorpicker",
            label: "Color Picker",
            icon: "colorize",
            category: "Tools",
            desc: "Pick screen color with hyprpicker",
            action: () => exec("hyprpicker -a &")
        },
        "clipboard": {
            id: "clipboard",
            label: "Clipboard",
            icon: "content_paste",
            category: "Tools",
            desc: "Clipboard history manager",
            action: () => exec("cliphist list | (fuzzel -d || rofi -dmenu) | cliphist decode | wl-copy &")
        },
        "emoji": {
            id: "emoji",
            label: "Emoji Picker",
            icon: "sentiment_satisfied",
            category: "Tools",
            desc: "Search and insert emojis",
            action: () => exec("rofimoji || fuzzel &")
        },
        "scratchpad": {
            id: "scratchpad",
            label: "Scratchpad",
            icon: "move_to_inbox",
            category: "Tools",
            desc: "Send or retrieve window from scratchpad",
            isDynamic: true
        },

        // Audio & Media Controls
        "volume_up": {
            id: "volume_up",
            label: "Volume +5%",
            icon: "volume_up",
            category: "Media",
            desc: "Increase audio volume",
            action: () => exec("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+ || pamixer -i 5 &")
        },
        "volume_down": {
            id: "volume_down",
            label: "Volume -5%",
            icon: "volume_down",
            category: "Media",
            desc: "Decrease audio volume",
            action: () => exec("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%- || pamixer -d 5 &")
        },
        "volume_mute": {
            id: "volume_mute",
            label: "Mute Audio",
            icon: "volume_off",
            category: "Media",
            desc: "Toggle audio mute",
            action: () => exec("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle || pamixer -t &")
        },
        "media_play": {
            id: "media_play",
            label: "Play / Pause",
            icon: "play_arrow",
            category: "Media",
            desc: "Toggle active media playback",
            action: () => exec("playerctl play-pause &")
        },
        "media_next": {
            id: "media_next",
            label: "Next Track",
            icon: "skip_next",
            category: "Media",
            desc: "Skip to next media track",
            action: () => exec("playerctl next &")
        },
        "media_prev": {
            id: "media_prev",
            label: "Prev Track",
            icon: "skip_previous",
            category: "Media",
            desc: "Skip to previous media track",
            action: () => exec("playerctl previous &")
        },

        // Screen & Capture
        "screenshot_area": {
            id: "screenshot_area",
            label: "Snip Area",
            icon: "screenshot_region",
            category: "Capture",
            desc: "Capture selected screen region to clipboard",
            action: () => exec("grimblast --freeze copy area || hyprshot -m region --clipboard-only &")
        },
        "screenshot_full": {
            id: "screenshot_full",
            label: "Screenshot",
            icon: "fullscreen",
            category: "Capture",
            desc: "Capture entire screen to clipboard",
            action: () => exec("grimblast copy output || hyprshot -m output --clipboard-only &")
        },
        "screenrecord": {
            id: "screenrecord",
            label: "Screen Record",
            icon: "videocam",
            category: "Capture",
            desc: "Toggle screen recording",
            action: () => exec('pkill -SIGINT wf-recorder || wf-recorder -g "$(slurp)" -f ~/Videos/recording_$(date +%s).mp4 &')
        },

        // Window Management
        "toggle_float": {
            id: "toggle_float",
            label: "Toggle Float",
            icon: "picture_in_picture_alt",
            category: "Window",
            desc: "Toggle floating mode for active window",
            action: () => Hyprland.dispatch("hl.dsp.window.toggleFloating()")
        },
        "toggle_fullscreen": {
            id: "toggle_fullscreen",
            label: "Fullscreen",
            icon: "fullscreen",
            category: "Window",
            desc: "Toggle fullscreen mode for active window",
            action: () => Hyprland.dispatch("hl.dsp.window.fullscreen(1)")
        },
        "kill_window": {
            id: "kill_window",
            label: "Close Window",
            icon: "close",
            category: "Window",
            desc: "Close the currently focused window",
            action: () => Hyprland.dispatch("hl.dsp.window.kill()")
        },
        "pin_window": {
            id: "pin_window",
            label: "Pin Window",
            icon: "push_pin",
            category: "Window",
            desc: "Pin window across all workspaces",
            action: () => Hyprland.dispatch("hl.dsp.window.pin()")
        },

        // System & Session
        "session": {
            id: "session",
            label: "Session Menu",
            icon: "power_settings_new",
            category: "System",
            desc: "Open power and session dialog",
            action: () => {
                if (typeof GlobalStates !== "undefined" && GlobalStates) {
                    GlobalStates.sessionOpen = true
                } else {
                    exec("wlogout || hyprlock &")
                }
            }
        },
        "lock": {
            id: "lock",
            label: "Lock Screen",
            icon: "lock",
            category: "System",
            desc: "Lock Hyprland session",
            action: () => exec("loginctl lock-session || hyprlock &")
        },
        "nightlight": {
            id: "nightlight",
            label: "Night Light",
            icon: "nightlight",
            category: "System",
            desc: "Toggle blue light filter",
            action: () => exec("pkill gammastep || gammastep -O 4000 || hyprsunset &")
        },

        // Context Specific: Terminal Actions
        "kitty_new_window": {
            id: "kitty_new_window",
            label: "New Window",
            icon: "open_in_new",
            category: "Apps",
            desc: "Open new terminal in current working directory",
            action: () => openTerminalNewWindow(0)
        },
        "kitty_agy": {
            id: "kitty_agy",
            label: "Run agy",
            icon: "robot_2",
            category: "Tools",
            desc: "Run Antigravity AI coding assistant in terminal",
            action: () => exec("sleep 0.05 && wtype 'agy --dangerously-skip-permissions' -k Return")
        },
        "kitty_clear": {
            id: "kitty_clear",
            label: "Clear Terminal",
            icon: "mop",
            category: "Tools",
            desc: "Send Ctrl+L to clear active terminal",
            action: () => exec("sleep 0.05 && wtype -M ctrl -k l -m ctrl")
        },
        "kitty_dolphin": {
            id: "kitty_dolphin",
            label: "Open CWD in Dolphin",
            icon: "folder_open",
            category: "Tools",
            desc: "Open terminal's current directory in file manager",
            action: () => openTerminalCwd(0)
        },

        // Context Specific: Browser Actions
        "browsertabs": {
            id: "browsertabs",
            label: "Switch Tab",
            icon: "tabs",
            category: "Apps",
            desc: "Open localized tab switcher petal fan",
            hasSubTier: true,
            subTierType: "browsertabs",
            action: null
        },
        "browser_new_tab": {
            id: "browser_new_tab",
            label: "New Tab",
            icon: "tab",
            category: "Apps",
            desc: "Open a new browser tab",
            action: () => sendBrowserKey("", "wtype -M ctrl -k t -m ctrl")
        },
        "browser_close_tab": {
            id: "browser_close_tab",
            label: "Close Tab",
            icon: "tab_close",
            category: "Apps",
            desc: "Close the active browser tab",
            action: () => sendBrowserKey("", "wtype -M ctrl -k w -m ctrl")
        },
        "browser_dup_tab": {
            id: "browser_dup_tab",
            label: "Duplicate Tab",
            icon: "tab_duplicate",
            category: "Apps",
            desc: "Duplicate current browser tab",
            action: () => sendBrowserKey("", "wtype -M alt -k d -m alt && sleep 0.06 && wtype -M alt -k Return -m alt")
        },
        "browser_reopen_tab": {
            id: "browser_reopen_tab",
            label: "Reopen Tab",
            icon: "history",
            category: "Apps",
            desc: "Reopen last closed browser tab",
            action: () => sendBrowserKey("", "wtype -M ctrl -M shift -k t -m shift -m ctrl")
        }
    })

    // Returns array of all available functions for customizer picker
    function getFunctionCatalogue() {
        const list = []
        for (const k in root.functionRegistry) {
            list.push(root.functionRegistry[k])
        }
        return list
    }

    // ── Persistent Configuration ──────────────────────────────────────────────
    property var userConfig: ({
        "globalSlices": [
            "scratchpad",
            "terminal",
            "wallpapers",
            "btop",
            "session",
            "filejump",
            "calc"
        ],
        "kittySlices": [
            "scratchpad",
            "kitty_new_window",
            "kitty_agy",
            "kitty_clear",
            "kitty_dolphin"
        ],
        "browserSlices": [
            "scratchpad",
            "browsertabs",
            "browser_new_tab",
            "browser_close_tab",
            "browser_dup_tab",
            "browser_reopen_tab"
        ],
        "fileJumpTargets": [
            { "id": "downloads", "label": "Downloads", "path": "~/Downloads", "icon": "download" },
            { "id": "documents", "label": "Documents", "path": "~/Documents", "icon": "description" },
            { "id": "pictures",  "label": "Pictures",  "path": "~/Pictures",  "icon": "photo" },
            { "id": "music",     "label": "Music",     "path": "~/Music",     "icon": "music_note" },
            { "id": "home",      "label": "Home",      "path": "~",           "icon": "home" },
            { "id": "temp",      "label": "Temp",      "path": "/tmp",        "icon": "folder_delete" }
        ]
    })

    // Load configuration from ~/.config/radialMenu/config.json
    Process {
        id: loadConfigProc
        command: ["bash", "-c", "cat ~/.config/radialMenu/config.json 2>/dev/null || true"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const raw = text.trim()
                    if (raw.length > 2) {
                        const parsed = JSON.parse(raw)
                        root.userConfig = Object.assign({}, root.userConfig, parsed)
                        root.configChanged()
                    }
                } catch(e) {
                    console.log("[RadialMenuActions] Config load notice:", e)
                }
            }
        }
    }

    // Save configuration atomically to ~/.config/radialMenu/config.json
    function saveConfig(cfg) {
        root.userConfig = cfg
        const jsonStr = JSON.stringify(cfg, null, 2)
        const b64 = Qt.btoa(jsonStr)
        const pyScript = 'import base64, os; p = os.path.expanduser("~/.config/radialMenu/config.json"); os.makedirs(os.path.dirname(p), exist_ok=True); data = base64.b64decode("' + b64 + '").decode("utf-8"); open(p, "w").write(data)'
        Quickshell.execDetached(["python3", "-c", pyScript])
        root.configChanged()
    }

    // Native Directory Picker Process (kdialog or zenity)
    Process {
        id: dirPickerProc
        command: ["bash", "-c", "kdialog --getexistingdirectory ~ 2>/dev/null || zenity --file-selection --directory 2>/dev/null"]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                const picked = text.trim()
                if (picked.length > 0) {
                    root.directoryPicked(picked)
                }
            }
        }
    }

    function openDirectoryPicker() {
        dirPickerProc.running = true
    }

    // ── Customization APIs: Slices & File Jump Targets ────────────────────────

    // Get active slice IDs for a given context
    function getActiveSliceIds(context: string) {
        let key = "globalSlices"
        if (context === "kitty") key = "kittySlices"
        else if (context === "browser") key = "browserSlices"
        else if (context === "code") key = "codeSlices"
        else if (context === "media") key = "mediaSlices"

        if (root.userConfig && Array.isArray(root.userConfig[key])) {
            return root.userConfig[key]
        }
        if (key === "kittySlices") return ["scratchpad", "kitty_new_window", "kitty_agy", "kitty_clear", "kitty_dolphin", "active_apps"]
        if (key === "browserSlices") return ["scratchpad", "browsertabs", "browser_new_tab", "browser_close_tab", "browser_dup_tab", "browser_reopen_tab", "active_apps"]
        if (key === "codeSlices") return ["scratchpad", "code_palette", "code_terminal", "code_git_status", "code_format", "code_run", "active_apps"]
        if (key === "mediaSlices") return ["scratchpad", "media_play_pause", "media_prev", "media_next", "volume_up", "volume_down", "audio_sink", "active_apps"]
        return ["scratchpad", "terminal", "wallpapers", "btop", "session", "filejump", "calc", "active_apps"]
    }

    // Add a function to the current dial (modifies dial structure)
    function addSliceToDial(context: string, functionId: string) {
        const cfg = JSON.parse(JSON.stringify(root.userConfig))
        let key = "globalSlices"
        if (context === "kitty") key = "kittySlices"
        else if (context === "browser") key = "browserSlices"
        else if (context === "code") key = "codeSlices"
        else if (context === "media") key = "mediaSlices"

        if (!Array.isArray(cfg[key])) {
            cfg[key] = getActiveSliceIds(context).slice()
        }

        if (!cfg[key].includes(functionId)) {
            cfg[key].push(functionId)
            saveConfig(cfg)
        }
    }

    // Remove a function from the current dial (modifies dial structure)
    function removeSliceFromDial(context: string, functionId: string) {
        const cfg = JSON.parse(JSON.stringify(root.userConfig))
        let key = "globalSlices"
        if (context === "kitty") key = "kittySlices"
        else if (context === "browser") key = "browserSlices"
        else if (context === "code") key = "codeSlices"
        else if (context === "media") key = "mediaSlices"

        if (!Array.isArray(cfg[key])) {
            cfg[key] = getActiveSliceIds(context).slice()
        }

        if (cfg[key].length > 2) {
            const idx = cfg[key].indexOf(functionId)
            if (idx !== -1) {
                cfg[key].splice(idx, 1)
                saveConfig(cfg)
            }
        }
    }

    // Swap any slice on the current wheel
    function swapSlice(context: string, slotIndex: int, newFunctionId: string) {
        const cfg = JSON.parse(JSON.stringify(root.userConfig))
        let key = "globalSlices"
        if (context === "kitty") key = "kittySlices"
        else if (context === "browser") key = "browserSlices"
        else if (context === "code") key = "codeSlices"
        else if (context === "media") key = "mediaSlices"

        if (!Array.isArray(cfg[key])) {
            cfg[key] = []
        }

        if (slotIndex >= 0 && slotIndex < cfg[key].length) {
            cfg[key][slotIndex] = newFunctionId
        } else {
            cfg[key].push(newFunctionId)
        }

        saveConfig(cfg)
    }

    // Reorder slice from fromIndex to toIndex on the current wheel
    function reorderSlice(context: string, fromIndex: int, toIndex: int) {
        if (fromIndex === toIndex || fromIndex < 0 || toIndex < 0) return

        const cfg = JSON.parse(JSON.stringify(root.userConfig))
        let key = "globalSlices"
        if (context === "kitty") key = "kittySlices"
        else if (context === "browser") key = "browserSlices"
        else if (context === "code") key = "codeSlices"
        else if (context === "media") key = "mediaSlices"

        if (!Array.isArray(cfg[key])) {
            cfg[key] = getActiveSliceIds(context).slice()
        }

        if (fromIndex < cfg[key].length && toIndex < cfg[key].length) {
            const item = cfg[key].splice(fromIndex, 1)[0]
            cfg[key].splice(toIndex, 0, item)
            saveConfig(cfg)
        }
    }

    // Add a new target to File Jump
    function addFileJumpTarget(label: string, path: string, icon: string) {
        const cfg = JSON.parse(JSON.stringify(root.userConfig))
        if (!Array.isArray(cfg.fileJumpTargets)) {
            cfg.fileJumpTargets = []
        }
        cfg.fileJumpTargets.push({
            id: "target_" + Date.now(),
            label: label || "Folder",
            path: path || "~",
            icon: icon || "folder"
        })
        saveConfig(cfg)
    }

    // Edit an existing File Jump target
    function updateFileJumpTarget(index: int, label: string, path: string, icon: string) {
        const cfg = JSON.parse(JSON.stringify(root.userConfig))
        if (Array.isArray(cfg.fileJumpTargets) && index >= 0 && index < cfg.fileJumpTargets.length) {
            cfg.fileJumpTargets[index].label = label
            cfg.fileJumpTargets[index].path = path
            cfg.fileJumpTargets[index].icon = icon
            saveConfig(cfg)
        }
    }

    // Remove a File Jump target
    function removeFileJumpTarget(index: int) {
        const cfg = JSON.parse(JSON.stringify(root.userConfig))
        if (Array.isArray(cfg.fileJumpTargets) && index >= 0 && index < cfg.fileJumpTargets.length) {
            cfg.fileJumpTargets.splice(index, 1)
            saveConfig(cfg)
        }
    }

    // ── Active Apps Sub-Slice Generator (All Workspaces) ────────────────────
    function getActiveAppsSubSlices() {
        let clients = []
        if (GlobalStates.activeClientsList && Array.isArray(GlobalStates.activeClientsList) && GlobalStates.activeClientsList.length > 0) {
            clients = GlobalStates.activeClientsList
        } else if (typeof HyprlandData !== "undefined" && HyprlandData.windowList && HyprlandData.windowList.length > 0) {
            clients = HyprlandData.windowList
        } else if (root.cachedClients && root.cachedClients.length > 0) {
            clients = root.cachedClients
        } else {
            refreshClients()
            clients = root.cachedClients || []
        }

        const validClients = clients.filter(c => {
            if (!c || !c.address) return false
            const wsName = c.workspace ? (c.workspace.name || String(c.workspace.id || "")) : (c.workspaceId ? String(c.workspaceId) : "")
            if (wsName.startsWith("special:quickshell")) return false
            return true
        })

        if (validClients.length === 0) {
            return [{
                id: "no_windows",
                label: "No Open Apps",
                icon: "info",
                hasSubTier: false,
                action: () => {}
            }]
        }

        const classCounts = {}
        validClients.forEach(c => {
            const cls = (c.class || "App").toLowerCase()
            classCounts[cls] = (classCounts[cls] || 0) + 1
        })

        const classSeen = {}
        return validClients.map((c, i) => {
            const rawClass = c.class || "App"
            const lowerClass = rawClass.toLowerCase()
            const wsName = c.workspace ? (c.workspace.name || String(c.workspace.id || "")) : (c.workspaceId ? String(c.workspaceId) : "1")
            classSeen[lowerClass] = (classSeen[lowerClass] || 0) + 1

            let displayLabel = rawClass.charAt(0).toUpperCase() + rawClass.slice(1)
            if (classCounts[lowerClass] > 1) {
                displayLabel += " #" + classSeen[lowerClass]
            }
            displayLabel += " [WS " + wsName + "]"

            const iconName = resolveAppIcon(lowerClass)
            const addr = c.address

            return {
                id: "app_" + addr,
                label: displayLabel,
                fullTitle: (c.title ? (c.title + " (" + rawClass + ")") : displayLabel),
                icon: iconName,
                hasSubTier: false,
                action: () => focusWindow(addr)
            }
        })
    }

    // ── Clipboard History Sub-Slice Generator ─────────────────────────────────
    function getClipboardSubSlices() {
        refreshClipboard()
        const clips = (root.cachedClips && root.cachedClips.length > 0) ? root.cachedClips : []
        if (clips.length === 0) {
            return [{
                id: "no_clips",
                label: "Clipboard Empty",
                icon: "content_paste_off",
                hasSubTier: false,
                action: () => {}
            }]
        }
        return clips.map((item, idx) => ({
            id: "clip_" + item.id,
            label: item.preview || ("Item " + (idx + 1)),
            fullTitle: item.full || item.preview,
            icon: "assignment",
            hasSubTier: false,
            action: () => {
                Quickshell.execDetached(["python3", root.ipcScriptPath, "paste", String(item.id)])
            }
        }))
    }

    // ── PipeWire Audio Sinks Sub-Slice Generator ──────────────────────────────
    function getAudioSinksSubSlices() {
        refreshAudioSinks()
        const sinks = (root.cachedSinks && root.cachedSinks.length > 0) ? root.cachedSinks : []
        if (sinks.length === 0) {
            return [{
                id: "no_sinks",
                label: "No Sinks Found",
                icon: "volume_off",
                hasSubTier: false,
                action: () => {}
            }]
        }
        return sinks.map((s, idx) => ({
            id: "sink_" + s.id,
            label: s.name,
            fullTitle: s.fullName + (s.default ? " (Active)" : ""),
            icon: s.default ? "check_circle" : "speaker",
            hasSubTier: false,
            action: () => {
                Quickshell.execDetached(["python3", root.ipcScriptPath, "set_sink", String(s.id)])
            }
        }))
    }

    // ── Helper to resolve slice item ──────────────────────────────────────────
    function resolveSliceItem(fnId: string, slotIdx: int, win) {
        if (fnId === "scratchpad") {
            const sp = getScratchpadSlice(win)
            sp.slotIndex = slotIdx
            sp.functionId = "scratchpad"
            return sp
        }
        const def = root.functionRegistry[fnId]
        if (!def) {
            return {
                slotIndex: slotIdx,
                functionId: fnId,
                label: fnId,
                icon: "extension",
                hasSubTier: false,
                action: () => {}
            }
        }
        return {
            slotIndex: slotIdx,
            functionId: fnId,
            label: def.label,
            icon: def.icon,
            hasSubTier: !!def.hasSubTier,
            subTierType: def.subTierType || "",
            action: def.action
        }
    }

    // ── Slices Generator ──────────────────────────────────────────────────────
    function getSlicesFor(tier: string, context: string, win) {
        // Scratchpad Workspace Selector Ring
        if (tier === "active_apps") {
            return getActiveAppsSubSlices()
        }

        if (tier === "clipboard") {
            return getClipboardSubSlices()
        }

        if (tier === "audio_sink") {
            return getAudioSinksSubSlices()
        }

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

        // Browser Real-time Tabs Ring
        if (tier === "browsertabs") {
            const winAddr = win?.address || ""
            let rawTabs = []
            if (typeof GlobalStates !== "undefined" && GlobalStates && GlobalStates.browserTabsList) {
                rawTabs = GlobalStates.browserTabsList
            }
            if (rawTabs && Array.isArray(rawTabs) && rawTabs.length > 0) {
                return rawTabs.map((t, i) => {
                    const idx = t.index || (i + 1)
                    return {
                        label: t.title || 'Tab ' + idx,
                        icon: idx <= 8 ? 'counter_' + idx : "tab",
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

        // File Jump Sub-Tier (Customizable files/folders)
        if (tier === "filejump") {
            const rawTargets = (root.userConfig && Array.isArray(root.userConfig.fileJumpTargets))
                ? root.userConfig.fileJumpTargets
                : [
                    { id: "downloads", label: "Downloads", path: "~/Downloads", icon: "download" },
                    { id: "documents", label: "Documents", path: "~/Documents", icon: "description" },
                    { id: "pictures",  label: "Pictures",  path: "~/Pictures",  icon: "photo" },
                    { id: "music",     label: "Music",     path: "~/Music",     icon: "music_note" },
                    { id: "home",      label: "Home",      path: "~",           icon: "home" },
                    { id: "temp",      label: "Temp",      path: "/tmp",        icon: "folder_delete" }
                ]

            const slices = rawTargets.map((t, idx) => ({
                id: t.id || 'target_' + idx,
                label: t.label,
                icon: t.icon || "folder",
                hasSubTier: false,
                targetPath: t.path,
                targetIndex: idx,
                action: () => performFileJump(t.path)
            }))

            // Dedicated "+" Petal to easily add new folders/files
            slices.push({
                id: "add_file_target",
                label: "Add Target",
                icon: "add",
                isAddButton: true,
                hasSubTier: false,
                action: null
            })

            return slices
        }

        // Context: Kitty Terminal
        if (context === "kitty") {
            const ids = (root.userConfig && Array.isArray(root.userConfig.kittySlices))
                ? root.userConfig.kittySlices
                : ["scratchpad", "kitty_new_window", "kitty_agy", "kitty_clear", "kitty_dolphin"]

            return ids.map((fnId, slotIdx) => {
                if (fnId === "scratchpad") {
                    const sp = getScratchpadSlice(win)
                    sp.slotIndex = slotIdx
                    sp.functionId = "scratchpad"
                    return sp
                }
                const def = root.functionRegistry[fnId]
                if (!def) {
                    return {
                        slotIndex: slotIdx,
                        functionId: fnId,
                        label: fnId,
                        icon: "extension",
                        hasSubTier: false,
                        action: () => {}
                    }
                }
                return {
                    slotIndex: slotIdx,
                    functionId: fnId,
                    label: def.label,
                    icon: def.icon,
                    hasSubTier: !!def.hasSubTier,
                    subTierType: def.subTierType || "",
                    action: def.action
                }
            })
        }

        // Context: Browser
        if (context === "browser") {
            const winAddr = win?.address || ""
            let tabCount = 0
            if (typeof GlobalStates !== "undefined" && GlobalStates && GlobalStates.browserTabsList) {
                tabCount = GlobalStates.browserTabsList.length
            }
            const tabSliceLabel = tabCount > 0 ? 'Switch Tab (' + tabCount + ')' : "Switch Tab"

            const ids = (root.userConfig && Array.isArray(root.userConfig.browserSlices))
                ? root.userConfig.browserSlices
                : ["scratchpad", "browsertabs", "browser_new_tab", "browser_close_tab", "browser_dup_tab", "browser_reopen_tab"]

            return ids.map((fnId, slotIdx) => {
                if (fnId === "scratchpad") {
                    const sp = getScratchpadSlice(win)
                    sp.slotIndex = slotIdx
                    sp.functionId = "scratchpad"
                    return sp
                }
                if (fnId === "browsertabs") {
                    return {
                        slotIndex: slotIdx,
                        functionId: "browsertabs",
                        label: tabSliceLabel,
                        icon: "tabs",
                        hasSubTier: true,
                        subTierType: "browsertabs",
                        action: null
                    }
                }
                const def = root.functionRegistry[fnId]
                if (!def) {
                    return {
                        slotIndex: slotIdx,
                        functionId: fnId,
                        label: fnId,
                        icon: "extension",
                        hasSubTier: false,
                        action: () => {}
                    }
                }
                return {
                    slotIndex: slotIdx,
                    functionId: fnId,
                    label: def.label,
                    icon: def.icon,
                    hasSubTier: !!def.hasSubTier,
                    subTierType: def.subTierType || "",
                    action: def.action
                }
            })
        }

        // Context: Global / Default
        const ids = (root.userConfig && Array.isArray(root.userConfig.globalSlices))
            ? root.userConfig.globalSlices
            : ["scratchpad", "terminal", "wallpapers", "btop", "session", "filejump", "calc"]

        return ids.map((fnId, slotIdx) => {
            if (fnId === "scratchpad") {
                const sp = getScratchpadSlice(win)
                sp.slotIndex = slotIdx
                sp.functionId = "scratchpad"
                return sp
            }
            const def = root.functionRegistry[fnId]
            if (!def) {
                return {
                    slotIndex: slotIdx,
                    functionId: fnId,
                    label: fnId,
                    icon: "extension",
                    hasSubTier: false,
                    action: () => {}
                }
            }
            return {
                slotIndex: slotIdx,
                functionId: fnId,
                label: def.label,
                icon: def.icon,
                hasSubTier: !!def.hasSubTier,
                subTierType: def.subTierType || "",
                action: def.action
            }
        })
    }
}
