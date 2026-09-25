use rmk::a;
use rmk::k;
use rmk::kbctrl;
use rmk::lt;
use rmk::mo;
use rmk::mt;
use rmk::tg;
use rmk::to;
use rmk::types::action::KeyAction;
use rmk::types::modifier::ModifierCombination;
use rmk::user;
use rmk::wm;

pub const ROW: usize = 4;
pub const COL: usize = 12;
pub const NUM_LAYER: usize = 9;

#[rustfmt::skip]
pub fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        // =========================================================================
        // Layer 0: Base
        // =========================================================================
        [
            [k!(Tab),   k!(Q), lt!(7, W), lt!(1, E), lt!(6, R),                       k!(T),   k!(LeftBracket),                 k!(P),         k!(O),     k!(I),     k!(U), k!(Y)],
            [k!(LGui),  k!(A), k!(S),     k!(D),     mt!(F, ModifierCombination::LCTRL), mt!(G, ModifierCombination::LALT), mt!(Quote, ModifierCombination::LCTRL), k!(Semicolon), k!(L),     k!(K),     k!(J), k!(H)],
            [k!(RGui),  k!(Z), k!(X),     k!(C),     k!(V),                       k!(B),   mt!(Escape, ModifierCombination::LALT), k!(Slash), k!(Dot), k!(Comma), k!(M), k!(N)],
            [a!(No), mt!(Space, ModifierCombination::LSHIFT), a!(No), mt!(Delete, ModifierCombination::LGUI), lt!(2, Delete), a!(No), a!(No), mt!(Enter, ModifierCombination::LSHIFT), a!(No), lt!(3, Backspace), a!(No), a!(No)],
        ],

        // =========================================================================
        // Layer 1: Mouse
        // =========================================================================
        [
            [a!(Transparent), a!(Transparent), mo!(7), mo!(6), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), k!(LGui), k!(LAlt), k!(LShift), k!(LCtrl), a!(Transparent), a!(Transparent), a!(Transparent), k!(MouseBtn3), k!(MouseBtn2), k!(MouseBtn1), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), mo!(3), mo!(2), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), k!(MouseBtn1), a!(No), k!(MouseBtn2), k!(MouseBtn3), a!(No), a!(No), a!(Transparent), a!(No), to!(0), a!(No), a!(No)],
        ],

        // =========================================================================
        // Layer 2: Symbol
        // =========================================================================
        [
            [wm!(Insert, ModifierCombination::RSHIFT), wm!(Kc1, ModifierCombination::LSHIFT), wm!(Kc2, ModifierCombination::LSHIFT), wm!(Kc3, ModifierCombination::LSHIFT), wm!(Kc4, ModifierCombination::LSHIFT), wm!(Kc5, ModifierCombination::LSHIFT), k!(Backspace), wm!(Kc0, ModifierCombination::LSHIFT), wm!(Kc9, ModifierCombination::LSHIFT), wm!(Kc8, ModifierCombination::LSHIFT), wm!(Kc7, ModifierCombination::LSHIFT), wm!(Kc6, ModifierCombination::LSHIFT)],
            [k!(LCtrl), tg!(4), a!(No), k!(RGui), k!(Backspace), tg!(3), k!(Grave), k!(Backslash), k!(RightBracket), k!(LeftBracket), k!(Equal), k!(Minus)],
            [k!(LAlt), tg!(5), wm!(X, ModifierCombination::LCTRL), a!(No), wm!(P, ModifierCombination::LCTRL | ModifierCombination::LALT), a!(No), wm!(Grave, ModifierCombination::LSHIFT), wm!(Backslash, ModifierCombination::LSHIFT), wm!(RightBracket, ModifierCombination::LSHIFT), wm!(LeftBracket, ModifierCombination::LSHIFT), wm!(Equal, ModifierCombination::LSHIFT), wm!(Minus, ModifierCombination::LSHIFT)],
            [a!(No), k!(Space), a!(No), k!(RGui), a!(Transparent), a!(No), a!(No), mt!(Enter, ModifierCombination::LSHIFT), a!(No), a!(Transparent), a!(No), a!(No)],
        ],

        // =========================================================================
        // Layer 3: Number
        // =========================================================================
        [
            [wm!(C, ModifierCombination::RCTRL), k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Semicolon), k!(Kc0), k!(Kc9), k!(Kc8), k!(Kc7), k!(Kc6)],
            [wm!(V, ModifierCombination::RCTRL), wm!(I, ModifierCombination::RCTRL | ModifierCombination::LSHIFT), k!(Home), k!(End), k!(Backspace), wm!(F12, ModifierCombination::LCTRL), mt!(F12, ModifierCombination::LCTRL), k!(Minus), k!(Right), k!(Up), k!(Down), k!(Left)],
            [wm!(X, ModifierCombination::RCTRL), k!(F1), k!(F2), k!(F3), k!(F4), k!(F5), k!(F11), k!(F10), k!(F9), k!(F8), k!(F7), k!(F6)],
            [a!(No), mt!(Tab, ModifierCombination::LCTRL), a!(No), wm!(F3, ModifierCombination::LCTRL), k!(LAlt), a!(No), a!(No), mt!(Enter, ModifierCombination::LSHIFT), a!(No), a!(Transparent), a!(No), a!(No)],
        ],

        // =========================================================================
        // Layer 4: Function
        // =========================================================================
        [
            [a!(Transparent), wm!(Delete, ModifierCombination::LCTRL | ModifierCombination::LALT), k!(F7), k!(F8), k!(F9), k!(F10), a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(Transparent), a!(No), k!(F4), k!(F5), k!(F6), k!(F11), a!(Transparent), k!(LGui), k!(LAlt), k!(LShift), k!(LCtrl), a!(No)],
            [a!(Transparent), a!(No), k!(F1), k!(F2), k!(F3), k!(F12), a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
        ],

        // =========================================================================
        // Layer 5: Button (Game)
        // =========================================================================
        [
            [k!(Escape), k!(Escape), k!(Q), k!(W), k!(E), k!(R), a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(Transparent), k!(LCtrl), k!(A), k!(S), k!(D), k!(F), a!(Transparent), a!(No), k!(MouseBtn3), k!(MouseBtn2), k!(MouseBtn1), a!(No)],
            [a!(Transparent), k!(LShift), k!(Z), k!(X), k!(C), k!(V), a!(Transparent), k!(Kc5), k!(Kc4), k!(Kc3), k!(Kc2), k!(Kc1)],
            [a!(No), k!(M), a!(No), k!(LAlt), k!(Space), a!(No), a!(No), k!(T), a!(No), k!(B), a!(No), a!(No)],
        ],

        // =========================================================================
        // Layer 6: MouseSnip (Sniper Mode: 1/6 speed)
        // =========================================================================
        [
            [a!(Transparent), a!(Transparent), mo!(7), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), k!(LGui), k!(LAlt), k!(LShift), k!(LCtrl), a!(Transparent), a!(Transparent), a!(Transparent), k!(MouseBtn3), k!(MouseBtn2), k!(MouseBtn1), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), mo!(3), mo!(2), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), k!(MouseBtn1), a!(No), k!(MouseBtn2), k!(MouseBtn3), a!(No), a!(No), a!(Transparent), a!(No), to!(0), a!(No), a!(No)],
        ],

        // =========================================================================
        // Layer 7: MouseScroll (Scroll Mode: 1/3 speed, Y inverted)
        // =========================================================================
        [
            [a!(Transparent), a!(Transparent), mo!(7), mo!(6), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), k!(LGui), k!(LAlt), k!(LShift), k!(LCtrl), a!(Transparent), a!(Transparent), a!(Transparent), k!(MouseBtn3), k!(MouseBtn2), k!(MouseBtn1), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), mo!(3), mo!(2), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), k!(MouseBtn1), a!(No), k!(MouseBtn2), k!(MouseBtn3), a!(No), a!(No), a!(Transparent), a!(No), to!(0), a!(No), a!(No)],
        ],

        // =========================================================================
        // Layer 8: Reset (Bootloader, Dual-Dongle & BLE profile switching)
        // =========================================================================
        [
            [user!(8), user!(9), user!(0), user!(1), user!(2), kbctrl!(Bootloader), a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No), a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No), a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), kbctrl!(Bootloader), a!(No), a!(No), a!(No), a!(No), a!(No), kbctrl!(Bootloader), a!(No), a!(No), a!(No), a!(No)],
        ],
    ]
}
