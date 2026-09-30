use rmk_types::action::KeyAction;
use rmk_types::constants::COMBO_MAX_LENGTH;

/// Combo config instantiated with firmware's combo Vec capacity.
pub type ComboConfig = rmk_types::combo::Combo;

use crate::event::KeyboardEvent;

// Combo.state is a u16 bitmask, so combos are limited to 16 keys.
// Use core::assert! explicitly — the crate-level `assert!` macro dispatches to
// defmt::assert! which is not const-compatible.
const _: () = core::assert!(
    COMBO_MAX_LENGTH <= 16,
    "COMBO_MAX_LENGTH exceeds 16 — Combo.state is u16 and cannot track more than 16 keys"
);

/// Runtime combo instance (config + runtime state)
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Combo {
    pub(crate) config: ComboConfig,
    /// The state records the pressed keys of the combo
    state: u16,
    /// The flag indicates whether the combo is triggered
    is_triggered: bool,
}

impl Default for Combo {
    fn default() -> Self {
        Self::empty()
    }
}

impl Combo {
    pub fn new(config: ComboConfig) -> Self {
        Self {
            config,
            state: 0,
            is_triggered: false,
        }
    }

    pub fn empty() -> Self {
        Self::new(ComboConfig::empty())
    }

    /// Update the combo's state when a key is pressed.
    /// Returns true if the combo is updated.
    pub(crate) fn update(
        &mut self,
        key_action: &KeyAction,
        base_action: &KeyAction,
        key_event: KeyboardEvent,
        active_layer: u8,
    ) -> bool {
        if !key_event.pressed || self.config.size() == 0 || self.is_triggered {
            // Ignore combo that without actions
            return false;
        }

        if let Some(layer) = self.config.layer {
            if layer != active_layer {
                return false;
            }
            let action_idx = self.config.find_key_action_index(key_action);
            if let Some(i) = action_idx {
                self.state |= 1 << i;
            } else if !self.is_all_pressed() {
                self.reset();
            }
            action_idx.is_some()
        } else {
            // Global combo: match against base_action (Layer 0) first,
            // or active key_action if base_action didn't match.
            let action_idx = self
                .config
                .find_key_action_index(base_action)
                .or_else(|| self.config.find_key_action_index(key_action));
            if let Some(i) = action_idx {
                self.state |= 1 << i;
            } else if !self.is_all_pressed() {
                self.reset();
            }
            action_idx.is_some()
        }
    }

    /// Re-assert a combo key's bit in the state of an already-triggered combo.
    ///
    /// Covers the case where the user releases one key of a held chord and presses
    /// it again while the other combo key is still down. The re-press must not
    /// leak to HID (it would overwrite the combo output's slot), and the eventual
    /// release must still complete the combo — so we re-set the bit here.
    ///
    /// Returns true iff this combo is triggered and `key_action` is one of its
    /// actions, i.e. the caller should swallow the press.
    pub(crate) fn reassert_if_triggered(&mut self, key_action: &KeyAction, base_action: &KeyAction) -> bool {
        if !self.is_triggered {
            return false;
        }
        let action_idx = if self.config.layer.is_some() {
            self.config.find_key_action_index(key_action)
        } else {
            self.config
                .find_key_action_index(base_action)
                .or_else(|| self.config.find_key_action_index(key_action))
        };
        if let Some(i) = action_idx {
            self.state |= 1 << i;
            return true;
        }
        false
    }

    /// Update the combo's state when a key is released
    /// When the combo is fully released from triggered state, this function returns true
    pub(crate) fn update_released(&mut self, key_action: &KeyAction, base_action: &KeyAction) -> bool {
        let action_idx = if self.config.layer.is_some() {
            self.config.find_key_action_index(key_action)
        } else {
            self.config
                .find_key_action_index(base_action)
                .or_else(|| self.config.find_key_action_index(key_action))
        };
        if let Some(i) = action_idx {
            self.state &= !(1 << i);
        }

        // Reset the combo if all keys are released
        if self.state == 0 {
            if self.is_triggered {
                self.reset();
                return true;
            }
            self.reset();
        }
        false
    }

    /// Returns true if this combo contains the given key action on its active or base layer.
    pub(crate) fn matches_key(&self, key_action: &KeyAction, base_action: &KeyAction) -> bool {
        if self.config.layer.is_some() {
            self.config.contains(key_action)
        } else {
            self.config.contains(base_action) || self.config.contains(key_action)
        }
    }

    /// Mark the combo as done, if all actions are satisfied
    pub(crate) fn trigger(&mut self) -> KeyAction {
        if self.is_triggered() {
            return self.config.output;
        }

        if self.is_all_pressed() {
            self.is_triggered = true;
        }
        self.config.output
    }

    // Check if the combo is dispatched into key event
    pub(crate) fn is_triggered(&self) -> bool {
        self.is_triggered
    }

    // Check if all keys of this combo are pressed, but it does not mean the combo key event is sent
    pub(crate) fn is_all_pressed(&self) -> bool {
        let cnt = self.config.size();
        cnt > 0 && self.keys_pressed() == cnt as u32
    }

    // The size of the current combo
    pub(crate) fn size(&self) -> usize {
        self.config.size()
    }

    pub(crate) fn keys_pressed(&self) -> u32 {
        self.state.count_ones()
    }

    pub(crate) fn reset(&mut self) {
        self.state = 0;
        self.is_triggered = false;
    }
}

#[cfg(test)]
mod tests {
    use rmk_types::action::Action;
    use rmk_types::keycode::{HidKeyCode, KeyCode};

    use super::*;

    fn hid(k: HidKeyCode) -> KeyAction {
        KeyAction::Single(Action::Key(KeyCode::Hid(k)))
    }

    // A combo whose output is empty (`KC_NO`) must still mark itself triggered
    // once all its keys are pressed. `is_triggered` is what makes the release
    // path consume the combo keys' releases (see `process_combo`). Without it a
    // combo swallows the presses but forwards the releases, leaving a stateful
    // key such as a mouse wheel with an unpaired release that repeats forever.
    #[test]
    fn empty_output_combo_still_triggers_so_releases_are_consumed() {
        let mut combo = Combo::new(ComboConfig::new(
            [hid(HidKeyCode::A), hid(HidKeyCode::B)],
            KeyAction::No,
            None,
        ));
        let a = hid(HidKeyCode::A);
        let b = hid(HidKeyCode::B);

        assert!(combo.update(&a, &a, KeyboardEvent::key(0, 0, true), 0));
        assert!(combo.update(&b, &b, KeyboardEvent::key(0, 1, true), 0));
        assert!(combo.is_all_pressed());

        let output = combo.trigger();
        assert_eq!(output, KeyAction::No, "empty-output combo still emits nothing");
        assert!(
            combo.is_triggered(),
            "a KC_NO combo must trigger so the release path consumes the releases"
        );
    }

    #[test]
    fn global_combo_triggers_with_base_action_on_different_layer() {
        let mut combo = Combo::new(ComboConfig::new(
            [hid(HidKeyCode::F), hid(HidKeyCode::J)],
            hid(HidKeyCode::Space),
            None,
        ));
        let f = hid(HidKeyCode::F);
        let j = hid(HidKeyCode::J);
        let lctrl = hid(HidKeyCode::LCtrl);
        let mouse1 = hid(HidKeyCode::MouseBtn1);

        // On layer 1, physical keys F and J map to LCtrl and MouseBtn1
        assert!(combo.update(&lctrl, &f, KeyboardEvent::key(1, 4, true), 1));
        assert!(combo.update(&mouse1, &j, KeyboardEvent::key(1, 10, true), 1));
        assert!(combo.is_all_pressed());

        let output = combo.trigger();
        assert_eq!(output, hid(HidKeyCode::Space));
        assert!(combo.is_triggered());

        // Releasing F on layer 1
        assert!(!combo.update_released(&lctrl, &f));
        // Releasing J on layer 1 fully releases the combo
        assert!(combo.update_released(&mouse1, &j));
        assert!(!combo.is_triggered());
    }
}
