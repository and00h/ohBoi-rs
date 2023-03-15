use std::cell::RefCell;
use std::rc::Rc;
use crate::core::interrupts::{Interrupt, InterruptController};

mod timer_control_flags {
    pub const ENABLE: u8 = 0b00000100;
    pub const SPEED: u8 = 0b00000011;
}

mod timer_masks {
    pub const TAC_MASK: u8 = 0b11111000;
}

const TIMER_INCREMENTS: [i32; 4] = [1024, 16, 64, 256];

pub struct Timer {
    tima: u8,
    tma: u8,
    tac: u8,
    timer_counter: i32,
    divider: u8,
    divider_counter: u32,
    timer_overflow: bool,
    interrupt_controller: Rc<RefCell<InterruptController>>
}

impl Timer {
    pub fn new(interrupt_controller: Rc<RefCell<InterruptController>>) -> Self {
        Timer {
            tima: 0, tma: 0, tac: 0, timer_counter: 1024, timer_overflow: false,
            divider: 0, divider_counter: 0,
            interrupt_controller
        }
    }

    pub fn tac(&self) -> u8 {
        self.tac | timer_masks::TAC_MASK
    }

    pub fn set_tac(&mut self, val: u8) {
        self.tac = val;
    }

    #[inline]
    fn timer_enabled(&self) -> bool {
        (self.tac & timer_control_flags::ENABLE) != 0
    }

    #[inline]
    fn get_timer_increment(&self) -> i32 {
        let idx = (self.tac & timer_control_flags::SPEED) as usize;
        TIMER_INCREMENTS[idx]
    }

    pub fn clock(&mut self, cycles: u32) {
        // Divider
        self.divider_counter += cycles;
        if self.divider_counter >= 0xFF {
            self.divider_counter -= 0xFF;
            self.divider = self.divider.wrapping_add(1);
        }

        if self.timer_enabled() {
            self.timer_counter -= cycles as i32;
            let increment = self.get_timer_increment();
            while self.timer_counter <= 0 {
                self.timer_counter += increment;
                if self.tima == 0xFF {
                    (*self.interrupt_controller).borrow_mut().raise(Interrupt::TIMER);
                    self.tima = self.tma;
                } else {
                    self.tima += 1;
                }
            }
        }
    }

    pub fn update_timer_counter(&mut self) {
        self.timer_counter = self.get_timer_increment();
    }

    pub fn reset(&mut self) {
        self.tima = 0;
        self.tma = 0;
        self.set_tac(0);
        self.divider = 0;
        self.divider_counter = 0;
        self.timer_counter = TIMER_INCREMENTS[0];
    }
}