import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const INDICATOR_SIZE = 28; // Diameter of the halo
const HALF_SIZE = INDICATOR_SIZE / 2;

export default class CharybdisCursorExtension extends Extension {
    enable() {
        this._isLayer1Active = false;
        this._tracker = null;
        this._trackerId = 0;
        this._pollTimeoutId = 0;

        // Create the glowing emerald indicator actor
        this._indicator = new St.Widget({
            name: 'charybdis-layer1-halo',
            reactive: false,
            can_focus: false,
            track_hover: false,
            visible: false,
            style: `
                width: ${INDICATOR_SIZE}px;
                height: ${INDICATOR_SIZE}px;
                border-radius: ${HALF_SIZE}px;
                background-color: rgba(46, 213, 115, 0.22);
                border: 2.5px solid #2ed573;
                box-shadow: 0 0 10px rgba(46, 213, 115, 0.7);
            `,
        });

        // Add to uiGroup so it renders above windows but below system overlays
        Main.layoutManager.uiGroup.add_child(this._indicator);

        // Position tracking via MetaCursorTracker
        if (global.backend?.get_cursor_tracker) {
            this._tracker = global.backend.get_cursor_tracker();
            this._trackerId = this._tracker.connect('position-invalidated', () => {
                if (this._isLayer1Active) {
                    this._updatePos();
                }
            });
        }

        // Subscribe to D-Bus signal from charybdis_cursor daemon
        this._dbusSignalId = Gio.DBus.session.signal_subscribe(
            null,
            'local.CharybdisCursor',
            'LayerChanged',
            '/local/CharybdisCursor',
            null,
            Gio.DBusSignalFlags.NONE,
            (conn, sender, path, iface, signal, params) => {
                try {
                    const [layer, isLayer1] = params.deep_unpack();
                    this._setLayer1(isLayer1);
                } catch (e) {
                    console.error(`[charybdis-cursor] D-Bus signal error: ${e}`);
                }
            }
        );

        // Initial check from runtime state file
        this._checkInitialState();
    }

    disable() {
        if (this._dbusSignalId) {
            Gio.DBus.session.signal_unsubscribe(this._dbusSignalId);
            this._dbusSignalId = 0;
        }

        if (this._tracker && this._trackerId) {
            this._tracker.disconnect(this._trackerId);
            this._trackerId = 0;
            this._tracker = null;
        }

        if (this._pollTimeoutId) {
            GLib.source_remove(this._pollTimeoutId);
            this._pollTimeoutId = 0;
        }

        if (this._indicator) {
            this._indicator.destroy();
            this._indicator = null;
        }
    }

    _checkInitialState() {
        const runtimeDir = GLib.get_user_runtime_dir();
        const stateFile = GLib.build_filenamev([runtimeDir, 'charybdis_layer']);
        if (GLib.file_test(stateFile, GLib.FileTest.EXISTS)) {
            try {
                const [ok, content] = GLib.file_get_contents(stateFile);
                if (ok) {
                    const text = new TextDecoder().decode(content).trim();
                    this._setLayer1(text === '1');
                }
            } catch (e) {
                // Ignore read error
            }
        }
    }

    _setLayer1(active) {
        if (this._isLayer1Active === active) return;
        this._isLayer1Active = active;

        if (active) {
            this._updatePos();
            this._indicator.show();

            // If tracker is not supported, start high-rate poll only while Layer 1 is active
            if (!this._tracker && !this._pollTimeoutId) {
                this._pollTimeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 12, () => {
                    this._updatePos();
                    return GLib.SOURCE_CONTINUE;
                });
            }
        } else {
            this._indicator.hide();
            if (this._pollTimeoutId) {
                GLib.source_remove(this._pollTimeoutId);
                this._pollTimeoutId = 0;
            }
        }
    }

    _updatePos() {
        if (!this._indicator) return;
        const [x, y] = global.get_pointer();
        this._indicator.set_position(x - HALF_SIZE, y - HALF_SIZE);
    }
}
