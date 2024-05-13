use std::cell::RefCell;
use std::rc::Rc;
use crate::core::interrupts::{Interrupt, InterruptController};

mod timer_control_flags {
    pub const ENABLE: u8 = 0b00000100;
    pub const SPEED: u8 = 0b00000011;
}

const TAC_MASK: u8 = 0b11111000;
const FREQ_0: u16 = 0b0000001000000000;
const FREQ_1: u16 = 0b1000;
const FREQ_2: u16 = 0b100000;
const FREQ_3: u16 = 0b10000000;

//const TIMER_INCREMENTS: [u32; 4] = [1024, 16, 64, 256];
const TAC_FREQS: [u16; 4] = [
    FREQ_0, FREQ_1, FREQ_2, FREQ_3
];

pub struct Timer {
    pub tima: u8,
    pub tma: u8,
    tac: u8,
    timer_counter: u16,
    old_output: bool,
    timer_overflow: bool,
    written_tma: bool,
    interrupt_controller: Rc<RefCell<InterruptController>>,
}

impl Timer {
    pub fn new(interrupt_controller: Rc<RefCell<InterruptController>>) -> Self {
        Timer {
            tima: 0, tma: 0, tac: 0, timer_counter: 0xABCC, old_output: false,
            timer_overflow: false, written_tma: false, interrupt_controller
        }
    }

    pub fn divider(&self) -> u8 {
        (self.timer_counter >> 8) as u8
    }

    pub fn divider_lo(&self) -> u8 {
        self.timer_counter as u8
    }

    pub fn tac(&self) -> u8 {
        self.tac | TAC_MASK
    }

    pub fn set_tac(&mut self, val: u8) {
        let old_bit = self.timer_counter & self.timer_freq_mask() != 0;
        self.tac = val;
        let new = (self.timer_counter & self.timer_freq_mask() != 0) && self.timer_enabled();
        if self.old_output && !new {
            let (tima, overflow) = self.tima.overflowing_add(1);
            self.tima = tima;
            self.timer_overflow = overflow;
        } else if !old_bit && new {
            let (tima, overflow) = self.tima.overflowing_add(1);
            self.tima = tima;
            self.timer_overflow = overflow;
        }
        self.old_output = new;
    }

    pub fn set_tima(&mut self, val: u8) {
        if !self.written_tma {
            self.tima = val;
        }
        self.timer_overflow = false;
    }

    pub fn set_tma(&mut self, val: u8) {
        self.tma = val;
        if self.written_tma {
            self.tima = val;
        }
    }

    #[inline]
    fn timer_enabled(&self) -> bool {
        (self.tac & timer_control_flags::ENABLE) != 0
    }

    #[inline]
    fn timer_freq_mask(&self) -> u16 {
        let idx = (self.tac & timer_control_flags::SPEED) as usize;
        TAC_FREQS[idx]
    }

    pub fn clock(&mut self) {
        self.written_tma = false;
        if self.timer_overflow {
            self.written_tma = true;
            self.timer_overflow = false;
            self.tima = self.tma;
            (*self.interrupt_controller).borrow_mut().raise(Interrupt::TIMER);
        }
        self.timer_counter = self.timer_counter.wrapping_add(4);
        let freq_mask = self.timer_freq_mask();
        let new_output = (self.timer_counter & freq_mask != 0) && self.timer_enabled();
        if !new_output && self.old_output {
            let (tima, overflow) = self.tima.overflowing_add(1);
            self.tima = tima;
            self.timer_overflow = overflow;
        }
        self.old_output = new_output;
    }

    pub fn reset_counter(&mut self) {
        self.timer_counter = 0;
        if self.old_output {
            let (tima, overflow) = self.tima.overflowing_add(1);
            self.tima = tima;
            self.old_output = false;
            self.timer_overflow |= overflow;
        }
    }
    pub fn reset(&mut self) {
        self.tima = 0;
        self.tma = 0;
        self.set_tac(0);
        self.timer_counter = 0xABCC;
    }
}