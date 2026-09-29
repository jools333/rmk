use rmk::config::{AutoMouseLayerConfig, BehaviorConfig, CombosConfig, KeyboardMacrosConfig};
use rmk::keyboard::combo::{Combo, ComboConfig};
use rmk::types::constants::COMBO_MAX_NUM;
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rmk::types::morse::{MorseMode, MorseProfile};
use rmk::{k, macros, wm};

pub fn get_default_behavior_config() -> BehaviorConfig {
    let mut behavior_config = BehaviorConfig::default();

    // 1. Morse / Hold-Tap тайминги и поведение (из layout.vil / Vial settings):
    // Setting 2: Combo timeout = 60ms
    // Setting 6: OneShot timeout = 1000ms
    // Setting 7: Hold timeout (Morse timeout / tapping-term) = 200ms
    // Setting 18: Tap interval = 20ms
    // Setting 19: Tap capslock interval = 20ms
    // Setting 22: PermissiveHold = true
    // Setting 23: HoldOnOtherKeyPress = false
    // Setting 25: Quick-tap / gap timeout = 180ms
    // Setting 26: UnilateralTap = true
    // Setting 27: Prior idle time = 125ms (через enable_flow_tap + prior_idle_time)
    behavior_config.morse.enable_flow_tap = true;
    behavior_config.morse.prior_idle_time = embassy_time::Duration::from_millis(125);
    behavior_config.morse.default_profile = MorseProfile::new(
        Some(true), // unilateral_tap: true (setting 26)
        Some(MorseMode::PermissiveHold), // permissive_hold: true (setting 22)
        Some(200u16), // hold timeout: 200ms (setting 7)
        Some(180u16), // gap timeout: 180ms (setting 25)
    )
    .with_quick_tap_timeout_ms(Some(180))
    .with_enable_flow_tap(Some(true));

    behavior_config.one_shot.timeout = embassy_time::Duration::from_millis(1000);
    behavior_config.tap.tap_interval = 20;
    behavior_config.tap.tap_capslock_interval = 20;

    // 2. Все 14 комбо из раскладки Charybdis Mini / keyboard.toml
    let combos: [Option<Combo>; COMBO_MAX_NUM] = [
        // 1. Q + P -> Win+Shift+P (VPN Toggle)
        Some(Combo::new(ComboConfig::new(
            [k!(Q), k!(P)],
            wm!(P, ModifierCombination::LSHIFT | ModifierCombination::LGUI),
            None,
        ))),
        // 2. LGui + Quote -> Win+S (Web Search)
        Some(Combo::new(ComboConfig::new(
            [k!(LGui), k!(Quote)],
            wm!(S, ModifierCombination::RGUI),
            None,
        ))),
        // 3. F + J -> Win+Space (Language Switch)
        Some(Combo::new(ComboConfig::new(
            [k!(F), k!(J)],
            wm!(Space, ModifierCombination::RGUI),
            None,
        ))),
        // 4. Z + Slash -> Win+Shift+Q (Close Window)
        Some(Combo::new(ComboConfig::new(
            [k!(Z), k!(Slash)],
            wm!(Q, ModifierCombination::LSHIFT | ModifierCombination::LGUI),
            None,
        ))),
        // 5. V + M -> Win+Z (Editor Action)
        Some(Combo::new(ComboConfig::new(
            [k!(V), k!(M)],
            wm!(Z, ModifierCombination::RGUI),
            None,
        ))),
        // 6. F + Semicolon -> Ctrl+S (Save)
        Some(Combo::new(ComboConfig::new(
            [k!(F), k!(Semicolon)],
            wm!(S, ModifierCombination::LCTRL),
            None,
        ))),
        // 7. R + U -> Win+A (Messenger)
        Some(Combo::new(ComboConfig::new(
            [k!(R), k!(U)],
            wm!(A, ModifierCombination::RGUI),
            None,
        ))),
        // 8. E + I -> Win+X (Command Palette)
        Some(Combo::new(ComboConfig::new(
            [k!(E), k!(I)],
            wm!(X, ModifierCombination::RGUI),
            None,
        ))),
        // 9. A + Semicolon -> Ctrl+P (Quick Open)
        Some(Combo::new(ComboConfig::new(
            [k!(A), k!(Semicolon)],
            wm!(P, ModifierCombination::RCTRL),
            None,
        ))),
        // 10. W + O -> Macro 0 (->)
        Some(Combo::new(ComboConfig::new([k!(W), k!(O)], macros!(0), None))),
        // 11. C + Comma -> Alt+Tab (Window Switch)
        Some(Combo::new(ComboConfig::new(
            [k!(C), k!(Comma)],
            wm!(Tab, ModifierCombination::LALT),
            None,
        ))),
        // 12. X + Dot -> Macro 1 (=>)
        Some(Combo::new(ComboConfig::new([k!(X), k!(Dot)], macros!(1), None))),
        // 13. X + Slash -> Win+Shift+Y (VPN 2 Toggle)
        Some(Combo::new(ComboConfig::new(
            [k!(X), k!(Slash)],
            wm!(Y, ModifierCombination::LSHIFT | ModifierCombination::LGUI),
            None,
        ))),
        // 14. Delete + Backspace -> Ctrl+F12 (Search, клавиши 37 и 40 на больших пальцах)
        Some(Combo::new(ComboConfig::new(
            [k!(Delete), k!(Backspace)],
            wm!(F12, ModifierCombination::LCTRL),
            None,
        ))),
        None,
        None,
        None,
        None,
        None,
        None,
    ];

    behavior_config.combo = CombosConfig {
        combos,
        timeout: embassy_time::Duration::from_millis(60),
        prior_idle_time: None,
    };

    // 3. Макросы:
    // Macro 0: "->" (тонкая стрелка)
    // Macro 1: "=>" (толстая стрелка)
    let macro0 = rmk::keyboard_macros::to_macro_sequence("->");
    let macro1 = rmk::keyboard_macros::to_macro_sequence("=>");
    behavior_config.keyboard_macros = KeyboardMacrosConfig::new(
        rmk::keyboard_macros::define_macro_sequences(&[macro0, macro1]),
    );

    // 4. Auto Mouse Layer:
    // Порог 4 counts (~0.12 мм движения шарика)
    // Таймаут бездействия выключен (0 мс): возврат только вручную (to!(0)) или по нажатию обычных клавиш
    // deactivate_on_key = true (любая обычная клавиша мгновенно закрывает мышиный слой)
    // reset_timeout_on_key = true
    // Исключения: модификаторы LCtrl, LShift, LAlt, LGui (чтобы работал Ctrl+Click и др.)
    // Заблокированные слои: Layer 5 (Game) — чтобы в играх при движении трекбола не переключало на мышиный слой
    let auto_mouse_config = AutoMouseLayerConfig::new(
        Some(0),
        1,
        embassy_time::Duration::from_millis(0),
        4,
    )
    .with_deactivate_on_key(&[
        KeyCode::Hid(HidKeyCode::LCtrl),
        KeyCode::Hid(HidKeyCode::LShift),
        KeyCode::Hid(HidKeyCode::LAlt),
        KeyCode::Hid(HidKeyCode::LGui),
    ])
    .with_reset_timeout_on_key()
    .with_blocked_layers(&[5]);
    let _ = behavior_config.auto_mouse_layer.push(auto_mouse_config);

    behavior_config
}
