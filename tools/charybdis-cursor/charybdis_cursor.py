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
import select
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

# Polling and retry intervals (milliseconds)
POLL_INTERVAL_ACTIVE_MS = 45     # 40-50 ms (~22 Hz) for instant layer response
POLL_INTERVAL_OFFLINE_MS = 1000  # 1000 ms (1 Hz) backoff when keyboard is sleeping/offline (0xFF)
POLL_INTERVAL_RETRY_MS = 1500    # 1500 ms (1-2s) retry when device is disconnected / I/O error
PRIORITY_CHECK_INTERVAL_SEC = 1.5  # Check for higher-priority wired connection when using dongle

# Space-bar Extension Schema Path
SPACE_BAR_SCHEMA_DIR = os.path.expanduser("~/.local/share/gnome-shell/extensions/space-bar@luchrioh/schemas")
SPACE_BAR_SCHEMA_ID = "org.gnome.shell.extensions.space-bar.appearance"

# Elegant muted green styling for the top panel, clock, and workspace pills
GREEN_PANEL_CSS = """
#panel {
    background-color: #172b1d !important;
    border: none !important;
    border-bottom: none !important;
    box-shadow: none !important;
}
.clock, .clock-display {
    color: #58b383 !important;
}
.space-bar-workspace-label.active {
    background-color: rgba(72, 169, 118, 0.4) !important;
    color: #ffffff !important;
    box-shadow: none !important;
}
.space-bar-workspace-label.inactive {
    color: rgba(185, 220, 198, 0.85) !important;
}
.space-bar-workspace-label.inactive.empty {
    color: rgba(185, 220, 198, 0.45) !important;
}
"""


def find_vial_device() -> tuple[str, str] | None:
    """Finds the Charybdis Mini / Dongle Vial raw HID device node (/dev/hidrawX).
    Prioritizes direct wired connection (PID 4643) over wireless dongle (PID 4644).
    Returns: (dev_path, pid) or None
    """
    candidates = []
    for dev_path in sorted(glob.glob("/dev/hidraw*")):
        base = os.path.basename(dev_path)
        syspath = f"/sys/class/hidraw/{base}/device"
        try:
            with open(f"{syspath}/uevent", "r") as f:
                uevent = f.read().upper()
            if f":0000{CHARYBDIS_VID}:" not in uevent:
                continue
            matched_pid = None
            for pid in CHARYBDIS_PIDS:
                if f":0000{pid}" in uevent:
                    matched_pid = pid
                    break
            if not matched_pid:
                continue
            with open(f"{syspath}/report_descriptor", "rb") as f:
                desc = f.read()
            if VIAL_USAGE_SIGNATURE in desc:
                # Priority: wired (4643) is 0, dongle (4644) is 1
                priority = 0 if matched_pid == "4643" else 1
                candidates.append((priority, dev_path, matched_pid))
        except (OSError, IOError):
            continue
    if candidates:
        candidates.sort(key=lambda x: x[0])
        return (candidates[0][1], candidates[0][2])
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

                    # If previous run was interrupted with green styling active, don't keep green as default
                    if self.orig_styles.strip() == GREEN_PANEL_CSS.strip():
                        self.orig_styles = ""
                        self.orig_enabled = False
                        self.settings.set_string("custom-styles", "")
                        self.settings.set_boolean("custom-styles-enabled", False)
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
        needs_reset = self.is_layer1_active or (
            self.settings
            and self.settings.get_boolean("custom-styles-enabled")
            and self.settings.get_string("custom-styles").strip() == GREEN_PANEL_CSS.strip()
        )
        if needs_reset:
            if self.settings:
                self.settings.set_boolean("custom-styles-enabled", self.orig_enabled)
                self.settings.set_string("custom-styles", self.orig_styles)
            self.is_layer1_active = False


def query_active_layer(fd: int) -> int | None:
    """Sends [0x53, 0x03] to query active layer directly from RMK Vial service.
    
    Returns:
        0..8: active keyboard layer
        -1: dongle is online, but keyboard BLE link is offline/asleep (0xFF response)
        None: I/O error, timeout, or physical USB disconnect
    """
    req = bytearray(32)
    req[0] = 0x53  # Vial layer stats command
    req[1] = 0x03  # Subcommand 0x03: Get Active Layer
    try:
        os.write(fd, req)
        r, _, _ = select.select([fd], [], [], 0.08)  # 80ms safety timeout
        if not r:
            return None
        resp = os.read(fd, 32)
        if len(resp) >= 3:
            if resp[0] == 0x53 and resp[1] == 0x03:
                return resp[2]
            elif resp[0] == 0xFF:
                # Dongle answered: keyboard BLE link is offline / sleeping
                return -1
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
        "pid": None,
        "last_layer": None,
        "running": True,
        "error_count": 0,
        "timeout_id": 0,
        "last_priority_check": 0.0,
    }

    def cleanup_fd():
        if state["fd"] is not None:
            try:
                os.close(state["fd"])
            except OSError:
                pass
            state["fd"] = None
        state["dev"] = None
        state["pid"] = None
        state["last_layer"] = None
        state["error_count"] = 0

    def poll_cycle():
        if not state["running"]:
            return False

        now = time.monotonic()

        # If currently connected to dongle (4644), check periodically if wired (4643) has appeared
        if state["fd"] is not None and state["pid"] == "4644":
            if now - state["last_priority_check"] >= PRIORITY_CHECK_INTERVAL_SEC:
                state["last_priority_check"] = now
                best_device = find_vial_device()
                if best_device and best_device[1] == "4643":
                    print(f"Higher-priority wired connection found ({best_device[0]}). Switching from dongle...")
                    cleanup_fd()

        # 1. Connect if needed
        if state["fd"] is None:
            best_device = find_vial_device()
            if best_device:
                dev_path, dev_pid = best_device
                try:
                    state["fd"] = os.open(dev_path, os.O_RDWR | os.O_NONBLOCK)
                    state["dev"] = dev_path
                    state["pid"] = dev_pid
                    state["last_priority_check"] = now
                    state["error_count"] = 0
                    kind = "wired" if dev_pid == "4643" else "wireless dongle"
                    print(f"Connected to Charybdis {kind} Vial interface: {dev_path}")
                except (OSError, IOError):
                    cleanup_fd()
            if state["fd"] is None:
                indicator.deactivate()
                state["timeout_id"] = GLib.timeout_add(POLL_INTERVAL_RETRY_MS, poll_cycle)
                return False

        # 2. Query active layer
        layer = query_active_layer(state["fd"])

        if layer is None:
            state["error_count"] += 1
            if state["error_count"] >= 3:
                # Persistent physical disconnect or read error
                print(f"Device disconnected or persistent read error on {state['dev']}. Reconnecting...")
                cleanup_fd()
                indicator.deactivate()
                state["timeout_id"] = GLib.timeout_add(POLL_INTERVAL_RETRY_MS, poll_cycle)
                return False
            # Transient USB timeout/jitter: retry next active poll without tearing down connection
            state["timeout_id"] = GLib.timeout_add(POLL_INTERVAL_ACTIVE_MS, poll_cycle)
            return False

        state["error_count"] = 0

        if layer == -1:
            # Keyboard link is offline/asleep (0xFF response)
            if state["last_layer"] != -1:
                indicator.deactivate()
                state["last_layer"] = -1
            # Back off to 1 second polling to conserve host resources
            state["timeout_id"] = GLib.timeout_add(POLL_INTERVAL_OFFLINE_MS, poll_cycle)
            return False

        # 3. Handle layer transition
        if layer != state["last_layer"]:
            if layer == 1:
                indicator.activate_layer1()
            else:
                indicator.deactivate()
            state["last_layer"] = layer

        # Active fast polling (40-50ms)
        state["timeout_id"] = GLib.timeout_add(POLL_INTERVAL_ACTIVE_MS, poll_cycle)
        return False

    def handle_signal(signum, frame):
        state["running"] = False
        print("\nShutting down Charybdis Indicator Daemon, restoring panel...")
        if state["timeout_id"]:
            GLib.source_remove(state["timeout_id"])
            state["timeout_id"] = 0
        indicator.deactivate()
        cleanup_fd()
        loop.quit()

    loop = GLib.MainLoop()

    signal.signal(signal.SIGINT, handle_signal)
    signal.signal(signal.SIGTERM, handle_signal)

    # Initial poll cycle
    state["timeout_id"] = GLib.timeout_add(0, poll_cycle)

    try:
        loop.run()
    finally:
        indicator.deactivate()
        cleanup_fd()


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
        best_device = find_vial_device()
        if best_device:
            dev_path, dev_pid = best_device
            kind = "wired" if dev_pid == "4643" else "wireless dongle"
            print(f"Vial device: {dev_path} ({kind}, PID 0x{dev_pid})")
            try:
                fd = os.open(dev_path, os.O_RDWR | os.O_NONBLOCK)
                layer = query_active_layer(fd)
                if layer == -1:
                    print("Active layer: Keyboard offline/asleep (0xFF)")
                elif layer is not None:
                    print(f"Active layer: {layer}")
                else:
                    print("Active layer: Query error")
                os.close(fd)
            except Exception as e:
                print(f"Read error: {e}")
        else:
            print("Vial device: Not found")
        os.system("systemctl --user status charybdis-cursor.service --no-pager")
        return

    run_daemon()


if __name__ == "__main__":
    main()
