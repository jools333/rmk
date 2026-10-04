#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "=== Installing Charybdis Host Layer Cursor Indicator ==="
python3 "${SCRIPT_DIR}/charybdis_cursor.py" --install
echo "=== Installation Complete ==="
