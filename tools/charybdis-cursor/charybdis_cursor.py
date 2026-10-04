#!/usr/bin/env python3
"""
Charybdis Mini — Linux Host Layer Indicator Daemon
Dynamically indicates when Layer 1 (Mouse Layer) is active on the Charybdis Mini
(wired USB or wireless dongle) by tinting the GNOME Shell top panel, date/clock,
and workspace names with an elegant emerald green theme.
Reverts to default panel appearance instantly on all other layers or upon disconnect/exit.
"""

import argparse
import glob
import os
import signal
import sys
import time

# Ensure line-buffered stdout for systemd journal
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(line_buffering=True)
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(line_buffering=True)

try:
    from gi.repository import Gio, GLib
except ImportError as e:
    print(f"Error: Missing required PyGObject dependencies ({e}). Install with: sudo apt install python3-gi", file=sys.stderr)
    sys.exit(1)

# Hardware IDs
CHARYBDIS_VID = "4C4B"
CHARYBDIS_PIDS = ("4643", "4644")  # 0x4643: Wired Central, 0x4644: USB Dongle
VIAL_USAGE_SIGNATURE = b"\x06\x60\xff\x09\x61"  # Usage Page 0xFF60, Usage 0x61

# Space-bar Extension Schema Path
SPACE_BAR_SCHEMA_DIR = os.path.expanduser("~/.local/share/gnome-shell/extensions/space-bar@luchrioh/schemas")
SPACE_BAR_SCHEMA_ID = "org.gnome.shell.extensions.space-bar.appearance"

# Elegant green styling for the top panel, clock, and workspace pills
GREEN_PANEL_CSS = """
#panel {
    background-color: rgba(18, 48, 28, 0.96) !important;
    border-bottom: 2px solid #2ed573 !important;
    box-shadow: 0 2px 12px rgba(46, 213, 115, 0.45) !important;
    transition-duration: 120ms;
}
.clock, .clock-display {
    color: #2ed573 !important;
    font-weight: bold !important;
}
.space-bar-workspace-label.active {
    background-color: rgba(46, 213, 115, 0.85) !important;
    color: #ffffff !important;
    box-shadow: 0 0 8px rgba(46, 213, 115, 0.6) !important;
}
.space-bar-workspace-label.inactive {
    color: rgba(160, 240, 180, 0.9) !important;
}
"""


def find_vial_device() -> str | None:
    """Finds the Charybdis Mini / Dongle Vial raw HID device node (/dev/hidrawX)."""
    for dev_path in sorted(glob.glob("/dev/hidraw*")):
        base = os.path.basename(dev_path)
        syspath = f"/sys/class/hidraw/{base}/device"
        try:
            with open(f"{syspath}/uevent", "r") as f:
                uevent = f.read()
            if f":0000{CHARYBDIS_VID}:" not in uevent:
                continue
            if not any(f":0000{pid}" in uevent for pid in CHARYBDIS_PIDS):
                continue
            with open(f"{syspath}/report_descriptor", "rb") as f:
                desc = f.read()
            if VIAL_USAGE_SIGNATURE in desc:
                return dev_path
        except (OSError, IOError):
            continue
    return None


class PanelLayerIndicator:
    """
    Manages top panel, clock, and workspace indicator styling via GNOME Shell GSettings.
    Provides immediate (0ms), non-intrusive green visual feedback across the entire display.
    """

    def __init__(self):
        self.settings = None
        self.orig_styles = ""
        self.orig_enabled = False
        self.is_layer1_active = False

        if os.path.isdir(SPACE_BAR_SCHEMA_DIR):
            try:
                schema_source = Gio.SettingsSchemaSource.new_from_directory(
                    SPACE_BAR_SCHEMA_DIR,
                    Gio.SettingsSchemaSource.get_default(),
                    False,
                )
                schema = schema_source.lookup(SPACE_BAR_SCHEMA_ID, False)
                if schema:
                    self.settings = Gio.Settings.new_full(schema, None, None)
                    self.orig_enabled = self.settings.get_boolean("custom-styles-enabled")
                    self.orig_styles = self.settings.get_string("custom-styles")
            except Exception as e:
                print(f"Warning: Could not initialize space-bar settings: {e}")

        # Also ensure cursor settings are restored to default (no stuck cursor themes)
        try:
            cur_settings = Gio.Settings.new("org.gnome.desktop.interface")
            if cur_settings.get_string("cursor-theme") == "Yaru-Green":
                cur_settings.set_string("cursor-theme", "Yaru")
            if cur_settings.get_int("cursor-size") == 28:
                cur_settings.set_int("cursor-size", 24)
        except Exception:
            pass

    def activate_layer1(self):
        """Activates green panel, clock, and workspace styling."""
        if not self.is_layer1_active:
            if self.settings:
                self.settings.set_string("custom-styles", GREEN_PANEL_CSS)
                self.settings.set_boolean("custom-styles-enabled", True)
            self.is_layer1_active = True

    def deactivate(self):
        """Restores default panel appearance."""
        if self.is_layer1_active:
            if self.settings:
                self.settings.set_boolean("custom-styles-enabled", self.orig_enabled)
                self.settings.set_string("custom-styles", self.orig_styles)
            self.is_layer1_active = False


def query_active_layer(fd: int) -> int | None:
    """Sends [0x53, 0x03] to query active layer directly from RMK Vial service."""
    req = bytearray(32)
    req[0] = 0x53  # Vial layer stats command
    req[1] = 0x03  # Subcommand 0x03: Get Active Layer
    try:
        os.write(fd, req)
        resp = os.read(fd, 32)
        if len(resp) >= 3 and resp[0] == 0x53 and resp[1] == 0x03:
            return resp[2]
    except (OSError, IOError):
        pass
    return None


def run_daemon():
    """Main daemon loop polling keyboard layer and updating panel indicator."""
    indicator = PanelLayerIndicator()

    print("Charybdis Panel Layer Indicator Daemon started.")
    print("Normal mode: Default top panel & workspace colors")
    print("Layer 1 mode: Emerald green panel tint, green clock & workspace labels")

    state = {
        "fd": None,
        "dev": None,
        "last_layer": None,
        "running": True,
    }

    def poll_keyboard():
        if not state["running"]:
            return False

        # 1. Connect if needed
        if state["fd"] is None:
            dev = find_vial_device()
            if dev:
                try:
                    state["fd"] = os.open(dev, os.O_RDWR)
                    state["dev"] = dev
                    print(f"Connected to Charybdis Vial interface: {dev}")
                except (OSError, IOError):
                    state["fd"] = None
                    state["dev"] = None
            if state["fd"] is None:
                indicator.deactivate()
                return True

        # 2. Query active layer
        layer = query_active_layer(state["fd"])
        if layer is None:
            print(f"Device disconnected or read error on {state['dev']}. Reconnecting...")
            try:
                os.close(state["fd"])
            except OSError:
                pass
            state["fd"] = None
            state["dev"] = None
            state["last_layer"] = None
            indicator.deactivate()
            return True

        # 3. Handle transition
        if layer != state["last_layer"]:
            if layer == 1:
                indicator.activate_layer1()
            else:
                indicator.deactivate()
            state["last_layer"] = layer

        return True

    def handle_signal(signum, frame):
        state["running"] = False
        print("\nShutting down Charybdis Indicator Daemon, restoring panel...")
        indicator.deactivate()
        if state["fd"] is not None:
            try:
                os.close(state["fd"])
            except OSError:
                pass
        loop.quit()

    loop = GLib.MainLoop()

    signal.signal(signal.SIGINT, handle_signal)
    signal.signal(signal.SIGTERM, handle_signal)

    # Poll every 25ms (40 Hz)
    GLib.timeout_add(25, poll_keyboard)

    try:
        loop.run()
    finally:
        indicator.deactivate()


def install_service():
    """Installs and enables systemd user service."""
    script_path = os.path.abspath(__file__)
    systemd_user_dir = os.path.expanduser("~/.config/systemd/user")
    os.makedirs(systemd_user_dir, exist_ok=True)
    service_file = os.path.join(systemd_user_dir, "charybdis-cursor.service")

    service_content = f"""[Unit]
Description=Charybdis Mini Host Layer Indicator
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Type=simple
Environment=PYTHONUNBUFFERED=1
ExecStart=/usr/bin/python3 {script_path} --daemon
Restart=always
RestartSec=2
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=graphical-session.target
"""
    with open(service_file, "w") as f:
        f.write(service_content)

    print(f"Wrote service file: {service_file}")
    os.system("systemctl --user daemon-reload")
    os.system("systemctl --user enable --now charybdis-cursor.service")
    print("Service enabled and started! Status:")
    os.system("systemctl --user status charybdis-cursor.service --no-pager")


def uninstall_service():
    """Stops and removes systemd user service."""
    service_file = os.path.expanduser("~/.config/systemd/user/charybdis-cursor.service")
    os.system("systemctl --user stop charybdis-cursor.service")
    os.system("systemctl --user disable charybdis-cursor.service")
    if os.path.exists(service_file):
        os.remove(service_file)
        print(f"Removed {service_file}")
    os.system("systemctl --user daemon-reload")
    PanelLayerIndicator().deactivate()
    print("Charybdis Indicator service uninstalled.")


def test_toggle():
    """Toggles Layer 1 green panel for 3 seconds to visually test in real-time."""
    indicator = PanelLayerIndicator()
    print("Testing Layer 1 green panel indicator for 3 seconds...")
    indicator.activate_layer1()
    time.sleep(3.0)
    indicator.deactivate()
    print("Restored default panel appearance.")


def main():
    parser = argparse.ArgumentParser(description="Charybdis Mini Host Layer Indicator")
    parser.add_argument("--daemon", action="store_true", help="Run background polling daemon")
    parser.add_argument("--install", action="store_true", help="Install & start systemd user service")
    parser.add_argument("--uninstall", action="store_true", help="Uninstall systemd user service")
    parser.add_argument("--test-toggle", action="store_true", help="Visually test green panel for 3 seconds")
    parser.add_argument("--status", action="store_true", help="Check device and service status")

    args = parser.parse_args()

    if args.install:
        install_service()
        return

    if args.uninstall:
        uninstall_service()
        return

    if args.test_toggle:
        test_toggle()
        return

    if args.status:
        dev = find_vial_device()
        print(f"Vial device: {dev if dev else 'Not found'}")
        if dev:
            try:
                fd = os.open(dev, os.O_RDWR)
                layer = query_active_layer(fd)
                print(f"Active layer: {layer}")
                os.close(fd)
            except Exception as e:
                print(f"Read error: {e}")
        os.system("systemctl --user status charybdis-cursor.service --no-pager")
        return

    run_daemon()


if __name__ == "__main__":
    main()
