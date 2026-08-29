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
    echo -e "${GREEN}[✓] Removed radialMenu files.${RESET}"
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

echo -e "${GREEN}[✓] Uninstalled successfully.${RESET}"
