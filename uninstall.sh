#!/usr/bin/env bash
# ==============================================================================
# Radial Dial (Rust Standalone Daemon) - Uninstaller
# Author: Y-astro (https://github.com/Y-astro)
# ==============================================================================
set -euo pipefail

GREEN="\033[1;32m"
BLUE="\033[1;34m"
RESET="\033[0m"

echo -e "${BLUE}[*] Uninstalling Radial Dial (Rust)...${RESET}"

systemctl --user stop radial-dial.service 2>/dev/null || true
systemctl --user disable radial-dial.service 2>/dev/null || true
rm -f "${HOME}/.config/systemd/user/radial-dial.service"
systemctl --user daemon-reload 2>/dev/null || true

rm -f "${HOME}/.local/bin/radial-dial"
rm -f "${HOME}/.local/bin/radial-tabs-host"
rm -f "${HOME}/.local/bin/radial-nuke"
rm -f "${HOME}/.mozilla/native-messaging-hosts/radial_tabs.json"
rm -f "${HOME}/.config/google-chrome/NativeMessagingHosts/radial_tabs.json"
rm -f "${HOME}/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/radial_tabs.json"
rm -f "${HOME}/.config/chromium/NativeMessagingHosts/radial_tabs.json"
rm -f "/tmp/radial-dial.sock"

pkill -9 -f radial-dial 2>/dev/null || true
pkill -9 -f radial-tabs-host 2>/dev/null || true

echo -e "${GREEN}[*] Radial Dial (Rust) uninstalled successfully.${RESET}"
