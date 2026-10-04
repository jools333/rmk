#!/usr/bin/env python3
"""
Charybdis Mini — Linux Host Cursor Indicator Daemon
Provides INSTANT (0ms) visual cursor indication when Layer 1 (Mouse Layer) is active
on the Charybdis Mini (wired USB or wireless dongle) by combining:
1. A hardware-accelerated, transparent, click-through glowing green halo tracking the pointer.
2. Synchronized GNOME cursor theme (Yaru-Green) and +15% size via GSettings.
Reverts to default theme and size on all other layers or upon disconnect/exit.
"""

import argparse
import glob
import math
import os
import signal
import struct
import sys
import time

# Ensure X11 backend for GDK to allow global overlay positioning on Ubuntu Wayland/Xwayland
os.environ.setdefault("GDK_BACKEND", "x11")

# Ensure line-buffered stdout for systemd journal
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(line_buffering=True)
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(line_buffering=True)

try:
    import gi
    gi.require_version("Gtk", "3.0")
    gi.require_version("Gdk", "3.0")
    from gi.repository import Gtk, Gdk, Gio, GLib
    import cairo
except ImportError as e:
    print(f"Error: Missing required GUI dependencies ({e}). Install with: sudo apt install python3-gi python3-cairo", file=sys.stderr)
    sys.exit(1)

# Hardware IDs
CHARYBDIS_VID = "4C4B"
CHARYBDIS_PIDS = ("4643", "4644")  # 0x4643: Wired Central, 0x4644: USB Dongle
VIAL_USAGE_SIGNATURE = b"\x06\x60\xff\x09\x61"  # Usage Page 0xFF60, Usage 0x61

# GNOME Settings Schema
SCHEMA_INTERFACE = "org.gnome.desktop.interface"
KEY_CURSOR_THEME = "cursor-theme"
KEY_CURSOR_SIZE = "cursor-size"

GREEN_THEME_NAME = "Yaru-Green"
BASE_THEME_NAME = "Yaru"
DEFAULT_BASE_SIZE = 24
SIZE_MULTIPLIER = 1.15  # +15%


def get_user_icons_dir() -> str:
    """Returns ~/.local/share/icons path."""
    return os.path.expanduser("~/.local/share/icons")


def tint_pixel(r: int, g: int, b: int, a: int) -> tuple[int, int, int, int]:
    """Applies high-visibility green tint to a cursor pixel."""
    if a == 0:
        return 0, 0, 0, 0
    lum = (r + g + b) / 3.0 / 255.0
    if lum > 0.4:
        # Bright body / highlight: vibrant lime-emerald green
        factor = (lum - 0.4) / 0.6
        new_r = int(45 * factor)
        new_g = int(220 + 35 * factor)
        new_b = int(75 * factor)
    else:
        # Dark border / shadow: charcoal with subtle dark green tint for crisp contrast
        new_r = int(r * 0.3)
        new_g = int(g * 0.9 + 20)
        new_b = int(b * 0.3)
    return min(255, new_r), min(255, new_g), min(255, new_b), a


def build_green_theme(base_theme: str = BASE_THEME_NAME, force: bool = False) -> str:
    """Generates the Yaru-Green cursor theme from system base theme."""
    target_theme_dir = os.path.join(get_user_icons_dir(), GREEN_THEME_NAME)
    target_cursors_dir = os.path.join(target_theme_dir, "cursors")
    src_cursors_dir = f"/usr/share/icons/{base_theme}/cursors"

    if not os.path.isdir(src_cursors_dir):
        if os.path.isdir("/usr/share/icons/Adwaita/cursors"):
            src_cursors_dir = "/usr/share/icons/Adwaita/cursors"
            base_theme = "Adwaita"
        else:
            raise RuntimeError(f"Base cursor directory not found: {src_cursors_dir}")

    default_cursor = os.path.join(target_cursors_dir, "default")
    if not force and os.path.exists(default_cursor):
        return target_theme_dir

    print(f"Generating '{GREEN_THEME_NAME}' from '{base_theme}'...")
    os.makedirs(target_cursors_dir, exist_ok=True)

    # Write cursor.theme & index.theme
    with open(os.path.join(target_theme_dir, "cursor.theme"), "w") as f:
        f.write(f"[Icon Theme]\nName={GREEN_THEME_NAME}\nInherits={base_theme}\n")

    with open(os.path.join(target_theme_dir, "index.theme"), "w") as f:
        f.write(f"[Icon Theme]\nName={GREEN_THEME_NAME}\nInherits={base_theme}\nDirectories=cursors\n\n[cursors]\nSize=24\nType=Scalable\n")

    files_processed = 0
    links_processed = 0

    for name in os.listdir(src_cursors_dir):
        src_path = os.path.join(src_cursors_dir, name)
        dst_path = os.path.join(target_cursors_dir, name)

        if os.path.islink(src_path):
            target = os.readlink(src_path)
            if os.path.lexists(dst_path):
                os.unlink(dst_path)
            os.symlink(target, dst_path)
            links_processed += 1
        elif os.path.isfile(src_path):
            with open(src_path, "rb") as f:
                raw = f.read()

            if raw.startswith(b"Xcur"):
                data = bytearray(raw)
                try:
                    magic, header_len, version, ntoc = struct.unpack_from("<4sIII", data, 0)
                    for i in range(ntoc):
                        ctype, subtype, pos = struct.unpack_from("<III", data, 16 + i * 12)
                        if ctype == 0xFFFD0002 and pos + 36 <= len(data):
                            chdr, t, st, v, w, h, xhot, yhot, delay = struct.unpack_from("<IIIIIIIII", data, pos)
                            pix_offset = pos + 36
                            num_pixels = w * h
                            if pix_offset + num_pixels * 4 <= len(data):
                                for p in range(num_pixels):
                                    off = pix_offset + p * 4
                                    b, g, r, a = data[off : off + 4]
                                    if a > 0:
                                        nb, ng, nr, na = tint_pixel(r, g, b, a)
                                        data[off] = nb
                                        data[off + 1] = ng
                                        data[off + 2] = nr
                except Exception:
                    pass

                if os.path.lexists(dst_path):
                    os.unlink(dst_path)
                with open(dst_path, "wb") as f:
                    f.write(data)
            else:
                if os.path.lexists(dst_path):
                    os.unlink(dst_path)
                with open(dst_path, "wb") as f:
                    f.write(raw)
            files_processed += 1

    # Update cache if available
    os.system(f"gtk-update-icon-cache -q {target_theme_dir} 2>/dev/null")
    print(f"Theme '{GREEN_THEME_NAME}' generated successfully ({files_processed} files, {links_processed} links).")
    return target_theme_dir


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


class CursorHaloOverlay(Gtk.Window):
    """
    Hardware-accelerated, 100% click-through green halo overlay.
    Follows cursor position smoothly to provide instant (0ms) feedback without
    relying on Wayland client application re-renders.
    """

    def __init__(self):
        super().__init__(type=Gtk.WindowType.POPUP)
        self.set_app_paintable(True)
        self.set_keep_above(True)

        screen = self.get_screen()
        visual = screen.get_rgba_visual() if screen else None
        if visual:
            self.set_visual(visual)

        self.size = 32
        self.half = self.size // 2
        self.set_size_request(self.size, self.size)
        self.connect("draw", self.on_draw)
        self.realize()

        # 100% click-through by setting an empty input shape
        self.input_shape_combine_region(cairo.Region())

        self.display = Gdk.Display.get_default()
        self.seat = self.display.get_default_seat() if self.display else None
        self.pointer = self.seat.get_pointer() if self.seat else None
        self._timer_id = 0

    def on_draw(self, widget, cr):
        cr.set_operator(cairo.OPERATOR_CLEAR)
        cr.paint()
        cr.set_operator(cairo.OPERATOR_OVER)

        cx, cy = self.half, self.half
        r = self.half - 3

        # Translucent green interior
        cr.arc(cx, cy, r, 0, 2 * math.pi)
        cr.set_source_rgba(0.18, 0.88, 0.45, 0.28)
        cr.fill_preserve()

        # Crisp glowing emerald green ring
        cr.set_source_rgba(0.16, 0.92, 0.40, 0.95)
        cr.set_line_width(2.5)
        cr.stroke()

    def update_pos(self):
        if not self.pointer:
            return
        _, x, y = self.pointer.get_position()
        self.move(x - self.half, y - self.half)

    def activate(self):
        self.update_pos()
        self.show_all()
        if not self._timer_id:
            # 60 FPS pointer tracking while Layer 1 is active
            self._timer_id = GLib.timeout_add(16, self._track_loop)

    def deactivate(self):
        if self._timer_id:
            GLib.source_remove(self._timer_id)
            self._timer_id = 0
        self.hide()

    def _track_loop(self):
        self.update_pos()
        return True


class CursorManager:
    """Manages instant green halo overlay and GNOME cursor theme/size via Gio.Settings."""

    def __init__(self):
        self.settings = Gio.Settings.new(SCHEMA_INTERFACE)
        self.default_theme = self.settings.get_string(KEY_CURSOR_THEME)
        if self.default_theme == GREEN_THEME_NAME:
            self.default_theme = BASE_THEME_NAME
        self.default_size = self.settings.get_int(KEY_CURSOR_SIZE)
        if self.default_size <= 0:
            self.default_size = DEFAULT_BASE_SIZE

        self.layer1_size = int(round(self.default_size * SIZE_MULTIPLIER))
        self.is_layer1_active = False
        self.overlay = CursorHaloOverlay()

    def update_base_defaults_if_normal(self):
        """Track user preference changes when not on Layer 1."""
        if not self.is_layer1_active:
            cur_theme = self.settings.get_string(KEY_CURSOR_THEME)
            cur_size = self.settings.get_int(KEY_CURSOR_SIZE)
            if cur_theme != GREEN_THEME_NAME:
                self.default_theme = cur_theme
            if cur_size != self.layer1_size and cur_size > 0:
                self.default_size = cur_size
                self.layer1_size = int(round(self.default_size * SIZE_MULTIPLIER))

    def set_layer1_cursor(self):
        """Activates instant overlay and sets green theme."""
        if not self.is_layer1_active:
            self.overlay.activate()
            self.settings.set_string(KEY_CURSOR_THEME, GREEN_THEME_NAME)
            self.settings.set_int(KEY_CURSOR_SIZE, self.layer1_size)
            self.is_layer1_active = True

    def restore_default_cursor(self):
        """Deactivates overlay and restores default cursor."""
        if self.is_layer1_active or self.settings.get_string(KEY_CURSOR_THEME) == GREEN_THEME_NAME:
            self.overlay.deactivate()
            self.settings.set_string(KEY_CURSOR_THEME, self.default_theme)
            self.settings.set_int(KEY_CURSOR_SIZE, self.default_size)
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
    """Main daemon loop running inside GTK/GLib event loop."""
    build_green_theme()
    cursor = CursorManager()

    print("Charybdis Cursor Daemon started.")
    print(f"Normal cursor: '{cursor.default_theme}' (size {cursor.default_size})")
    print(f"Layer 1 cursor: '{GREEN_THEME_NAME}' (size {cursor.layer1_size}) + Instant Green Halo Overlay")

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
                cursor.restore_default_cursor()
                return True

        # 2. Query layer
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
            cursor.restore_default_cursor()
            return True

        # 3. Handle transition
        if layer != state["last_layer"]:
            if layer == 1:
                cursor.set_layer1_cursor()
            else:
                cursor.restore_default_cursor()
                cursor.update_base_defaults_if_normal()
            state["last_layer"] = layer

        return True

    def handle_signal(signum, frame):
        state["running"] = False
        print("\nShutting down Charybdis Cursor Daemon, restoring default cursor...")
        cursor.restore_default_cursor()
        if state["fd"] is not None:
            try:
                os.close(state["fd"])
            except OSError:
                pass
        Gtk.main_quit()

    signal.signal(signal.SIGINT, handle_signal)
    signal.signal(signal.SIGTERM, handle_signal)

    # 40 Hz polling rate (every 25ms)
    GLib.timeout_add(25, poll_keyboard)

    try:
        Gtk.main()
    finally:
        cursor.restore_default_cursor()


def install_service():
    """Installs and enables systemd user service."""
    script_path = os.path.abspath(__file__)
    systemd_user_dir = os.path.expanduser("~/.config/systemd/user")
    os.makedirs(systemd_user_dir, exist_ok=True)
    service_file = os.path.join(systemd_user_dir, "charybdis-cursor.service")

    service_content = f"""[Unit]
Description=Charybdis Mini Host Layer Cursor Indicator
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
    CursorManager().restore_default_cursor()
    print("Charybdis Cursor service uninstalled.")


def test_toggle():
    """Toggles Layer 1 cursor & overlay for 3 seconds to visually test in real-time."""
    build_green_theme()
    cm = CursorManager()
    print(f"Testing Layer 1 cursor + overlay for 3 seconds: '{GREEN_THEME_NAME}' (size {cm.layer1_size})...")
    cm.set_layer1_cursor()
    GLib.timeout_add(3000, lambda: (cm.restore_default_cursor(), Gtk.main_quit()))
    Gtk.main()
    print("Restored default cursor.")


def main():
    parser = argparse.ArgumentParser(description="Charybdis Mini Host Cursor Indicator")
    parser.add_argument("--daemon", action="store_true", help="Run background polling daemon")
    parser.add_argument("--rebuild-theme", action="store_true", help="Force rebuild Yaru-Green theme")
    parser.add_argument("--install", action="store_true", help="Install & start systemd user service")
    parser.add_argument("--uninstall", action="store_true", help="Uninstall systemd user service")
    parser.add_argument("--test-toggle", action="store_true", help="Visually test green cursor & halo for 3 seconds")
    parser.add_argument("--status", action="store_true", help="Check device and service status")

    args = parser.parse_args()

    if args.rebuild_theme:
        build_green_theme(force=True)
        return

    if args.install:
        build_green_theme()
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
