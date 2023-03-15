use log::{debug, trace};
use serde::{Serialize, Deserialize};

const INITIAL_INTERRUPT_REQUEST: u8 = 0b11100001;
const INTERRUPT_REQUEST_MASK: u8 = 0b00011111;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Interrupt {
    VBLANK  = 0b00000001,
    LCD     = 0b00000010,
    TIMER   = 0b00000100,
    SERIAL  = 0b00001000,
    JPAD    = 0b00010000
}

#[derive(Serialize, Deserialize)]
pub struct InterruptController {
    pub ime: bool,
    int_request: u8,
    int_enable: u8
}

impl InterruptController {
    pub fn new() -> Self {
        trace!("Building InterruptController");
        InterruptController { ime: false, int_request: INITIAL_INTERRUPT_REQUEST, int_enable: 0 }
    }

    pub fn reset(&mut self) {
        trace!("Resetting InterruptController");
        self.ime = false;
        self.int_request = INITIAL_INTERRUPT_REQUEST;
        self.int_enable = 0;
    }

    pub fn get_interrupt_request(&self) -> u8 {
        self.int_request
    }

    pub fn get_interrupt_enable(&self) -> u8 {
        self.int_enable
    }

    pub fn set_interrupt_request(&mut self, val: u8) {
        self.int_request = val | 0xE0;
        trace!("Interrupt request set to {:#04x}", self.int_request);
    }

    pub fn set_interrupt_enable(&mut self, val: u8) {
        self.int_enable = val;
        trace!("Interrupt enable set to {:#04x}", self.int_enable);
    }

    pub fn raise(&mut self, interrupt: Interrupt) {
        debug!("Interrupt {:?} raised", interrupt);
        self.int_request |= interrupt as u8;
    }

    pub fn serve(&mut self, interrupt: Interrupt) {
        debug!("Interrupt {:?} served", interrupt);
        self.int_request &= !(interrupt as u8);
    }

    pub fn interrupts_pending(&self) -> bool {
        (self.int_enable & self.int_request & INTERRUPT_REQUEST_MASK) != 0
    }

    pub fn interrupts_requested(&self) -> bool {
        (self.int_request & INTERRUPT_REQUEST_MASK) != 0
    }

    pub fn is_enabled(&self, interrupt: Interrupt) -> bool {
        (self.int_enable & interrupt as u8) != 0
    }

    pub fn is_raised(&self, interrupt: Interrupt) -> bool {
        (self.int_request & interrupt as u8) != 0
    }
}