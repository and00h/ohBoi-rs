use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::rc::Rc;
use crate::core::interrupts::{Interrupt, InterruptController};

mod key_masks {
    pub const RIGHT_A: u8      = 0b00000001;
    pub const LEFT_B: u8       = 0b00000010;
    pub const UP_SELECT: u8    = 0b00000100;
    pub const DOWN_START: u8   = 0b00001000;
    pub const DIRECTIONAL: u8  = 0b00010000;
    pub const BUTTON : u8      = 0b00100000;

    pub const KEY_GROUPS: u8   = DIRECTIONAL | BUTTON;
    pub const KEYS: u8         = RIGHT_A | LEFT_B | UP_SELECT | DOWN_START;
}

#[repr(usize)]
#[derive(Debug, Copy, Clone)]
pub enum Key {
    A = 0,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down
}

const KEY_MASK_MAP: [u8; 8] = [
    key_masks::RIGHT_A, key_masks::LEFT_B, key_masks::UP_SELECT, key_masks::DOWN_START,
    key_masks::RIGHT_A, key_masks::LEFT_B, key_masks::UP_SELECT, key_masks::DOWN_START
];

pub struct Joypad {
    key_state_buttons: u8,
    key_state_dir: u8,
    key_select: u8,
    interrupt_controller: Rc<RefCell<InterruptController>>
}

impl Joypad {
    pub fn new(interrupt_controller: Rc<RefCell<InterruptController>>) -> Self {
        Joypad {
            key_state_buttons: 0xF,
            key_state_dir: 0xF,
            key_select: 0xF0,
            interrupt_controller
        }
    }

    pub fn get_key_register(&self) -> u8 {
        let mut res = (self.key_select & 0xF0) | 0xC0;
        if self.buttons_enabled() { res |= self.key_state_buttons; }
        if self.directional_enabled() { res |= self.key_state_dir; }

        res
    }

    fn buttons_enabled(&self) -> bool {
        self.key_select & key_masks::BUTTON == 0
    }

    fn directional_enabled(&self) -> bool {
        self.key_select & key_masks::DIRECTIONAL == 0
    }

    pub fn press(&mut self, key: Key) {
        match key {
            Key::A | Key::B | Key::Select | Key::Start => {
                self.key_state_buttons &= !KEY_MASK_MAP[key as usize];
                if self.buttons_enabled() { self.raise_jpad_interrupt(); }
            },
            Key::Up | Key::Down | Key::Left | Key::Right => {
                self.key_state_dir &= !KEY_MASK_MAP[key as usize];
                if self.directional_enabled() { self.raise_jpad_interrupt(); }
            },
            _ => {}
        }
    }

    pub fn release(&mut self, key: Key) {
        match key {
            Key::A | Key::B | Key::Select | Key::Start => {
                self.key_state_buttons |= KEY_MASK_MAP[key as usize];
            },
            Key::Up | Key::Down | Key::Left | Key::Right => {
                self.key_state_dir |= KEY_MASK_MAP[key as usize];
            },
            _ => {}
        }
    }

    pub(self) fn is_pressed(&self, key: Key) -> bool {
        let (button_group, buttons) = if (key as usize) < 4 { (key_masks::BUTTON, self.key_state_buttons) } else { (key_masks::DIRECTIONAL, self.key_state_dir) };
        (self.key_select & button_group) == 0 && (buttons & KEY_MASK_MAP[key as usize]) == 0
    }

    pub fn select_key_group(&mut self, val: u8) {
        let key_groups = val & key_masks::KEY_GROUPS;
        self.key_select = (val & 0xF0) | 0xC0;
    }

    #[inline]
    fn keys_enabled(&self) -> bool { (self.key_state_buttons & key_masks::KEY_GROUPS) != key_masks::KEY_GROUPS }
    #[inline]
    fn keys_pressed(&self) -> bool { (self.key_state_buttons & key_masks::KEYS) != key_masks::KEYS }

    #[inline]
    fn raise_jpad_interrupt(&self) {
        (*self.interrupt_controller).borrow_mut().raise(Interrupt::JPAD);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::core::interrupts::{Interrupt, InterruptController};
    use crate::core::joypad::{Joypad, Key, key_masks};

    #[inline]
    fn create_joypad() -> Joypad {
        Joypad::new(Rc::new(RefCell::new(InterruptController::new())))
    }

    #[test]
    fn initial_state() {
        let j = create_joypad();
        assert_eq!(j.get_key_register(), 0xFF);
    }

    #[test]
    fn select_directional() {
        let mut j = create_joypad();
        j.select_key_group(!key_masks::DIRECTIONAL);
        assert_eq!(j.get_key_register(), 0xff & !key_masks::DIRECTIONAL);
    }

    #[test]
    fn button_press_and_release() {
        let mut j = create_joypad();
        j.select_key_group(!key_masks::BUTTON);
        vec![Key::A, Key::B, Key::Select, Key::Start].into_iter()
            .for_each(|k| {
                j.press(k);
                assert!(j.is_pressed(k));
                j.release(k);
                assert!(!j.is_pressed(k));
            });
    }

    #[test]
    fn directional_press_and_release() {
        let mut j = create_joypad();
        j.select_key_group(!key_masks::DIRECTIONAL);
        vec![Key::Left, Key::Right, Key::Up, Key::Down].into_iter()
            .for_each(|k| {
                j.press(k);
                assert!(j.is_pressed(k));
                j.release(k);
                assert!(!j.is_pressed(k));
            });
    }

    #[test]
    fn no_key_pressed_when_keys_disabled() {
        let mut j = create_joypad();
        vec![Key::A, Key::B, Key::Select, Key::Start, Key::Left, Key::Right, Key::Up, Key::Down]
            .into_iter()
            .for_each(|k| {
                j.press(k);
                assert!(!j.is_pressed(k));
            });
    }

    #[test]
    fn pressing_key_raises_interrupt() {
        let mut j = create_joypad();
        j.select_key_group(key_masks::BUTTON);
        j.press(Key::A);
        assert!((*j.interrupt_controller).borrow_mut().is_raised(Interrupt::JPAD));
    }

    #[test]
    fn pressing_key_when_disabled_does_not_raise_interrupt() {
        let mut j = create_joypad();
        j.press(Key::A);
        assert!(!(*j.interrupt_controller).borrow_mut().is_raised(Interrupt::JPAD));
    }
}