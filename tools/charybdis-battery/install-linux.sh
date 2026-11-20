#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_SRC="$SCRIPT_DIR/bin/charybdis-battery"
BIN_DST="$HOME/.local/bin/charybdis-battery"
AUTOSTART_DIR="$HOME/.config/autostart"
DESKTOP_FILE="$AUTOSTART_DIR/charybdis-battery.desktop"

mkdir -p "$HOME/.local/bin" "$AUTOSTART_DIR"
cp "$BIN_SRC" "$BIN_DST"
chmod +x "$BIN_DST"

cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Charybdis Battery
Comment=System tray battery monitor for Charybdis Mini
Exec="$BIN_DST"
Terminal=false
X-GNOME-Autostart-enabled=true
EOF

echo "Installed $BIN_DST"
echo "Autostart enabled at $DESKTOP_FILE"
echo "Note: ensure udev permissions allow reading /dev/hidraw* for VID 4C4B (PID 4643/4644)."
