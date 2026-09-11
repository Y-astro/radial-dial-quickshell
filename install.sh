#!/usr/bin/env bash
# ==============================================================================
# Radial Dial for Hyprland & Quickshell (end4-pC / Illogical Impulse)
# One-Command Installer
# Author: Y-astro (https://github.com/Y-astro)
# ==============================================================================

set -e

GREEN="\033[1;32m"
BLUE="\033[1;34m"
YELLOW="\033[1;33m"
CYAN="\033[1;36m"
RED="\033[1;31m"
RESET="\033[0m"

echo -e "${CYAN}====================================================${RESET}"
echo -e "${CYAN}  Radial Dial for Hyprland & Quickshell Installer   ${RESET}"
echo -e "${CYAN}====================================================${RESET}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Parse optional arguments
GPU_MODE="auto"
for arg in "$@"; do
    case $arg in
        --low-end|--low)
            GPU_MODE="low_end"
            ;;
        --high-perf|--high)
            GPU_MODE="high_performance"
            ;;
    esac
done

# 1. Detect GPU Capabilities
IS_LOW_END=0
if [ "$GPU_MODE" = "low_end" ]; then
    IS_LOW_END=1
    echo -e "${YELLOW}[*] Low-End GPU profile manually forced.${RESET}"
elif [ "$GPU_MODE" = "high_performance" ]; then
    IS_LOW_END=0
    echo -e "${GREEN}[*] High-Performance GPU profile manually forced.${RESET}"
else
    echo -e "${BLUE}[*] Detecting GPU hardware capabilities...${RESET}"
    DRM_VENDORS=$(cat /sys/class/drm/card[0-9]*/device/vendor 2>/dev/null || true)
    HAS_DEDICATED=0
    if echo "$DRM_VENDORS" | grep -qi "0x10de"; then
        HAS_DEDICATED=1
    elif lspci -nn 2>/dev/null | grep -iE "vga|3d|display" | grep -qiE "nvidia|geforce|quadro|rtx|arc|radeon rx|radeon pro"; then
        HAS_DEDICATED=1
    fi

    if [ "$HAS_DEDICATED" -eq 1 ]; then
        IS_LOW_END=0
        echo -e "${GREEN}[*] Dedicated high-performance GPU detected.${RESET}"
    else
        IS_LOW_END=1
        echo -e "${YELLOW}[*] Integrated / low-power GPU detected (e.g. Intel UHD/HD).${RESET}"
        echo -e "${YELLOW}[*] Optimizing layer rules and rendering for maximum 60FPS performance.${RESET}"
    fi
fi

# 2. Detect Quickshell Config Directory
QS_DIR=""
if [ -d "$HOME/.config/quickshell/end4-pC" ]; then
    QS_DIR="$HOME/.config/quickshell/end4-pC"
elif [ -d "$HOME/.config/quickshell" ]; then
    QS_DIR="$HOME/.config/quickshell"
else
    echo -e "${RED}[!] Quickshell configuration directory not found.${RESET}"
    echo -e "Please install end4-pC / Illogical Impulse quickshell configuration first."
    exit 1
fi

echo -e "${BLUE}[*] Target Quickshell Directory:${RESET} $QS_DIR"

# 3. Copy radialMenu module
echo -e "${BLUE}[*] Installing radialMenu module...${RESET}"
mkdir -p "$QS_DIR/modules/ii/radialMenu"
cp -r "$SCRIPT_DIR/modules/ii/radialMenu/"* "$QS_DIR/modules/ii/radialMenu/"
chmod +x "$QS_DIR/modules/ii/radialMenu/get_browser_tabs.py" 2>/dev/null || true
chmod +x "$QS_DIR/modules/ii/radialMenu/folder_browser.py" 2>/dev/null || true
chmod +x "$QS_DIR/modules/ii/radialMenu/hypr_ipc.py" 2>/dev/null || true
chmod +x "$QS_DIR/modules/ii/radialMenu/extension/native_host.py" 2>/dev/null || true
echo -e "${GREEN}[*] radialMenu module copied.${RESET}"

# 4. Install Native Messaging Host for Firefox / Zen / Librewolf
echo -e "${BLUE}[*] Installing Native Messaging host for browser tab sync...${RESET}"
mkdir -p "$HOME/.mozilla/native-messaging-hosts"
mkdir -p "$HOME/.config/mozilla/native-messaging-hosts"
sed "s|PLACEHOLDER_PATH|$QS_DIR/modules/ii/radialMenu/extension|g" "$QS_DIR/modules/ii/radialMenu/extension/radial_tabs.json" > "$HOME/.mozilla/native-messaging-hosts/radial_tabs.json" 2>/dev/null || true
sed "s|PLACEHOLDER_PATH|$QS_DIR/modules/ii/radialMenu/extension|g" "$QS_DIR/modules/ii/radialMenu/extension/radial_tabs.json" > "$HOME/.config/mozilla/native-messaging-hosts/radial_tabs.json" 2>/dev/null || true
echo -e "${GREEN}[*] Native Messaging host registered.${RESET}"

# 5. Patch GlobalStates.qml
GLOBAL_STATES="$QS_DIR/GlobalStates.qml"
if [ -f "$GLOBAL_STATES" ]; then
    echo -e "${BLUE}[*] Updating radial menu state in GlobalStates.qml...${RESET}"
    python3 -c "
import re

with open('$GLOBAL_STATES', 'r') as f:
    content = f.read()

# Strip any existing radialMenu patch (old or new)
if 'radialMenuOpen' in content:
    content = re.sub(r'\s*// Radial menu state[\s\S]*?// <<< END RADIAL_MENU <<<', '', content)
    content = re.sub(r'\s*// Radial menu state[\s\S]*?(?=property bool|property var|signal [a-zA-Z]|$)', '', content)

patch = '''
    // Radial menu state
    property bool radialMenuOpen: false
    property var  radialMenuScreen: null
    property real radialMenuX: 0
    property real radialMenuY: 0
    property var  radialMenuContextWindow: ({})
    property var  radialMenuGpuProfile: null
    property var  browserTabsList: []
    property var  activeClientsList: []

    // ── Radial menu: cursor-position + active window context reader ───────────
    Process {
        id: radialMenuCursorProc
        command: [\"python3\", \"$QS_DIR/modules/ii/radialMenu/hypr_ipc.py\", \"context\"]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const data = JSON.parse(text.trim())
                    const pos = data.cursor || { x: 0, y: 0 }
                    let screen = null
                    for (let i = 0; i < Quickshell.screens.length; i++) {
                        const s = Quickshell.screens[i]
                        if (pos.x >= s.x && pos.x < s.x + s.width &&
                            pos.y >= s.y && pos.y < s.y + s.height) {
                            screen = s
                            break
                        }
                    }
                    if (!screen) {
                        const name = Hyprland.focusedMonitor?.name
                        screen = Quickshell.screens.find(s => s.name === name) ?? Quickshell.screens[0]
                    }
                    root.radialMenuContextWindow = data.window || {}
                    root.radialMenuGpuProfile = data.gpu || null
                    root.browserTabsList = data.tabs || []
                    root.activeClientsList = data.clients || []
                    root.radialMenuScreen = screen
                    root.radialMenuX = pos.x - screen.x
                    root.radialMenuY = pos.y - screen.y
                    root.radialMenuOpen = true
                } catch (e) {
                    console.warn(\"[RadialMenu] context/cursorpos parse failed:\", e)
                }
            }
        }
    }

    // On-demand tab refresh process
    Process {
        id: refreshTabsProc
        command: [\"python3\", \"$QS_DIR/modules/ii/radialMenu/get_browser_tabs.py\"]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.browserTabsList = JSON.parse(text.trim())
                } catch(e) {}
            }
        }
    }

    function refreshTabs() {
        refreshTabsProc.running = true
    }

    signal radialMenuCloseRequested()

    function requestCloseRadialMenu() {
        radialMenuCloseRequested()
    }

    CompositorGlobalShortcut {
        name: \"radialMenu\"
        description: \"Open radial pie menu at cursor\"
        onPressed: {
            if (root.radialMenuOpen) {
                root.requestCloseRadialMenu()
                return
            }
            radialMenuCursorProc.running = true
        }
    }
    // <<< END RADIAL_MENU <<<
'''

if 'property bool' in content:
    idx = content.find('property bool')
    new_content = content[:idx] + patch.strip() + '\n\n    ' + content[idx:]
    with open('$GLOBAL_STATES', 'w') as f:
        f.write(new_content)
    print('Patched GlobalStates.qml successfully.')
elif 'property' in content:
    idx = content.find('property')
    new_content = content[:idx] + patch.strip() + '\n\n    ' + content[idx:]
    with open('$GLOBAL_STATES', 'w') as f:
        f.write(new_content)
    print('Patched GlobalStates.qml successfully.')
"
fi

# 6. Patch IllogicalImpulseFamily.qml
II_FAMILY="$QS_DIR/panelFamilies/IllogicalImpulseFamily.qml"
if [ -f "$II_FAMILY" ]; then
    if ! grep -q "RadialMenu" "$II_FAMILY"; then
        echo -e "${BLUE}[*] Instantiating RadialMenu in IllogicalImpulseFamily.qml...${RESET}"
        python3 -c "
with open('$II_FAMILY', 'r') as f:
    content = f.read()

import_stmt = 'import qs.modules.ii.radialMenu\n'
if 'import qs.modules.ii.radialMenu' not in content:
    content = import_stmt + content

if 'RadialMenu {}' not in content:
    last_brace = content.rfind('}')
    content = content[:last_brace] + '    RadialMenu {}\n' + content[last_brace:]

with open('$II_FAMILY', 'w') as f:
    f.write(content)
print('Instantiated RadialMenu in IllogicalImpulseFamily.qml.')
"
    fi
fi

# 7. Patch Hyprland layer rules
HYPR_RULES="$HOME/.config/hypr/hyprland/rules.lua"
if [ -f "$HYPR_RULES" ]; then
    sed -i '/quickshell:radialMenu/d' "$HYPR_RULES"
    echo -e "${BLUE}[*] Configuring Hyprland layer rules...${RESET}"
    if [ "$IS_LOW_END" -eq 1 ]; then
        cat << 'EOF' >> "$HYPR_RULES"

-- Quickshell: Radial Menu (Optimized for Integrated / Low-End GPU - Blur disabled for maximum FPS)
-- hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, blur = true})
hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, ignore_alpha = 0.15})
hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, xray = false})
hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, no_anim = true})
EOF
        echo -e "${GREEN}[*] Layer rules configured with blur disabled for low-end hardware.${RESET}"
    else
        cat << 'EOF' >> "$HYPR_RULES"

-- Quickshell: Radial Menu (Frosted Glass Blur for Dedicated GPU)
hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, blur = true})
hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, ignore_alpha = 0.15})
hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, xray = false})
hl.layer_rule({ match = { namespace = "quickshell:radialMenu" }, no_anim = true})
EOF
        echo -e "${GREEN}[*] Frosted glass layer rules configured.${RESET}"
    fi
fi

# 8. Patch Hyprland keybind (Super + Tab)
HYPR_BINDS="$HOME/.config/hypr/hyprland/keybinds.lua"
if [ -f "$HYPR_BINDS" ]; then
    if ! grep -q "quickshell:radialMenu" "$HYPR_BINDS"; then
        echo -e "${BLUE}[*] Binding Super + Tab to quickshell:radialMenu...${RESET}"
        sed -i 's/.*SUPER + Tab.*overviewWorkspacesToggle.*/-- &/' "$HYPR_BINDS" 2>/dev/null || true
        cat << 'EOF' >> "$HYPR_BINDS"

-- Quickshell Radial Menu
hl.bind("SUPER + Tab", hl.dsp.global("quickshell:radialMenu"), { description = "Shell: Open radial menu at cursor" })
EOF
        echo -e "${GREEN}[*] Super + Tab bound to radial menu.${RESET}"
    fi
fi

# 9. Reload and verify
echo -e "${BLUE}[*] Reloading Hyprland and Quickshell...${RESET}"
hyprctl reload >/dev/null 2>&1 || true
killall qs quickshell 2>/dev/null || true
sleep 1
qs -c "$(basename "$QS_DIR")" -d >/dev/null 2>&1 &

echo -e "${GREEN}====================================================${RESET}"
echo -e "${GREEN}  Radial Dial Installed Successfully!               ${RESET}"
echo -e "${GREEN}  Press Super + Tab anywhere to open the radial dial.${RESET}"
echo -e "${GREEN}====================================================${RESET}"
