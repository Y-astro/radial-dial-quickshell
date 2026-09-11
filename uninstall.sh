#!/usr/bin/env bash
# ==============================================================================
# Radial Dial for Hyprland & Quickshell - Uninstaller
# Author: Y-astro (https://github.com/Y-astro)
# ==============================================================================

GREEN="\033[1;32m"
BLUE="\033[1;34m"
RED="\033[1;31m"
RESET="\033[0m"

echo -e "${BLUE}[*] Uninstalling Radial Dial module...${RESET}"

QS_DIR=""
if [ -d "$HOME/.config/quickshell/end4-pC" ]; then
    QS_DIR="$HOME/.config/quickshell/end4-pC"
elif [ -d "$HOME/.config/quickshell" ]; then
    QS_DIR="$HOME/.config/quickshell"
fi

if [ -n "$QS_DIR" ] && [ -d "$QS_DIR/modules/ii/radialMenu" ]; then
    rm -rf "$QS_DIR/modules/ii/radialMenu"
    echo -e "${GREEN}[*] Removed radialMenu files.${RESET}"
fi

# Remove native messaging host
rm -f "$HOME/.mozilla/native-messaging-hosts/radial_tabs.json" 2>/dev/null || true
rm -f "$HOME/.config/mozilla/native-messaging-hosts/radial_tabs.json" 2>/dev/null || true
rm -f "/tmp/radial_tabs_cache.json" "/tmp/radial_gpu_profile.json" 2>/dev/null || true

# Remove GlobalStates.qml patches
GLOBAL_STATES="$QS_DIR/GlobalStates.qml"
if [ -f "$GLOBAL_STATES" ]; then
    python3 -c "
import re
with open('$GLOBAL_STATES', 'r') as f:
    content = f.read()
if 'radialMenuOpen' in content:
    content = re.sub(r'\s*// Radial menu state[\s\S]*?// <<< END RADIAL_MENU <<<', '', content)
    content = re.sub(r'\s*// Radial menu state[\s\S]*?(?=property bool|property var|signal [a-zA-Z]|$)', '', content)
    with open('$GLOBAL_STATES', 'w') as f:
        f.write(content)
" 2>/dev/null || true
fi

# Remove IllogicalImpulseFamily.qml instantiation
II_FAMILY="$QS_DIR/panelFamilies/IllogicalImpulseFamily.qml"
if [ -f "$II_FAMILY" ]; then
    python3 -c "
with open('$II_FAMILY', 'r') as f:
    content = f.read()
content = content.replace('import qs.modules.ii.radialMenu\n', '')
content = content.replace('    RadialMenu {}\n', '')
with open('$II_FAMILY', 'w') as f:
    f.write(content)
" 2>/dev/null || true
fi

# Remove Hyprland rules
HYPR_RULES="$HOME/.config/hypr/hyprland/rules.lua"
if [ -f "$HYPR_RULES" ]; then
    sed -i '/quickshell:radialMenu/d' "$HYPR_RULES"
fi

# Remove Hyprland keybinds
HYPR_BINDS="$HOME/.config/hypr/hyprland/keybinds.lua"
if [ -f "$HYPR_BINDS" ]; then
    sed -i '/quickshell:radialMenu/d' "$HYPR_BINDS"
    # Restore overview toggle if commented
    sed -i 's/^-- \(.*SUPER + Tab.*overviewWorkspacesToggle.*\)/\1/' "$HYPR_BINDS"
fi

hyprctl reload >/dev/null 2>&1 || true
killall qs quickshell 2>/dev/null || true
sleep 1
if [ -n "$QS_DIR" ]; then
    qs -c "$(basename "$QS_DIR")" -d >/dev/null 2>&1 &
fi

echo -e "${GREEN}[*] Uninstalled successfully.${RESET}"
