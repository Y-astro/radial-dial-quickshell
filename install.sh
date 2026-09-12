#!/usr/bin/env bash
# radial-dial — Automated Installation Script
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
NATIVE_MESSAGING_DIR="${HOME}/.mozilla/native-messaging-hosts"

echo "=== Radial Dial (Rust) Installer ==="

# 1. Ensure directories exist
mkdir -p "${BIN_DIR}"
mkdir -p "${SYSTEMD_USER_DIR}"
mkdir -p "${NATIVE_MESSAGING_DIR}"

# 2. Build binaries in release mode
echo "--> Building binaries in release mode..."
cd "${SCRIPT_DIR}"
cargo build --release

# 3. Install binaries
echo "--> Installing binaries to ${BIN_DIR}..."
cp -f "${SCRIPT_DIR}/target/release/radial-dial" "${BIN_DIR}/radial-dial"
cp -f "${SCRIPT_DIR}/target/release/radial-tabs-host" "${BIN_DIR}/radial-tabs-host"
chmod +x "${BIN_DIR}/radial-dial" "${BIN_DIR}/radial-tabs-host"

# 4. Register WebExtension Native Messaging Host
echo "--> Registering Firefox/Zen and Chrome/Brave Native Messaging Hosts..."
cat <<EOF > "${NATIVE_MESSAGING_DIR}/radial_tabs.json"
{
  "name": "radial_tabs",
  "description": "Native messaging host for radial-dial browser tab sync",
  "path": "${BIN_DIR}/radial-tabs-host",
  "type": "stdio",
  "allowed_extensions": [
    "radial-tabs@local",
    "radial-dial@local",
    "{d4e5f6a7-1234-5678-9abc-def012345678}"
  ]
}
EOF

for chrome_dir in "${HOME}/.config/google-chrome/NativeMessagingHosts" "${HOME}/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts" "${HOME}/.config/chromium/NativeMessagingHosts"; do
  if [ -d "$(dirname "${chrome_dir}")" ]; then
    mkdir -p "${chrome_dir}"
    cat <<EOF > "${chrome_dir}/radial_tabs.json"
{
  "name": "radial_tabs",
  "description": "Native messaging host for radial-dial browser tab sync",
  "path": "${BIN_DIR}/radial-tabs-host",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://*/"
  ]
}
EOF
  fi
done

# 5. Install systemd user service
echo "--> Installing systemd user service..."
cat <<EOF > "${SYSTEMD_USER_DIR}/radial-dial.service"
[Unit]
Description=Radial Dial Wayland Menu Daemon
PartOf=graphical-session.target
After=graphical-session.target

[Service]
ExecStart=${BIN_DIR}/radial-dial
Environment=RUST_LOG=info
Restart=on-failure
RestartSec=1s

[Install]
WantedBy=graphical-session.target
EOF

systemctl --user daemon-reload || true

# 6. Install emergency failsafe
echo "--> Installing radial-nuke emergency failsafe..."
cat <<'EOF' > "${BIN_DIR}/radial-nuke"
#!/usr/bin/env bash
systemctl --user stop radial-dial.service 2>/dev/null || true
pkill -9 -f radial-dial 2>/dev/null || true
pkill -9 -f radial-tabs-host 2>/dev/null || true
rm -f /tmp/radial-dial.sock
notify-send -u critical -a "Radial Dial" "Radial Dial Terminated" "Failsafe triggered: all processes stopped." 2>/dev/null || true
EOF
chmod +x "${BIN_DIR}/radial-nuke"

echo ""
echo "=== Installation Complete! ==="
echo ""
echo "To start the daemon now:"
echo "  systemctl --user enable --now radial-dial.service"
echo "Or run manually in the background:"
echo "  ${BIN_DIR}/radial-dial &"
echo ""
echo "To trigger the radial menu from Hyprland:"
echo "Add this keybind in ~/.config/hypr/hyprland/keybinds.lua:"
echo "  hl.bind(\"SUPER + Tab\", hl.dsp.exec_cmd(\"radial-dial toggle\"))"
echo ""
