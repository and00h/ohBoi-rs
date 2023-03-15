use std::cell::RefCell;
use std::rc::{Rc, Weak};
use crate::core::cpu::Cpu;
use crate::core::interrupts::InterruptController;
use crate::core::joypad::Joypad;
use crate::core::timers::Timer;

pub(crate) struct BusController(Weak<RefCell<Bus>>);

impl BusController {
    pub fn new(bus: Weak<RefCell<Bus>>) -> Self {
        BusController(bus)
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        unimplemented!()
    }

    pub fn read(&self, addr: u16) -> u8 {
        unimplemented!()
    }

    pub fn advance(&self, cycles: u64) {
        unimplemented!()
    }
}

pub(crate) struct Bus {
    timer: Timer,
    joypad: Joypad,
    cpu: Rc<RefCell<Cpu>>
}

impl Bus {
    pub fn new(cpu: Rc<RefCell<Cpu>>) -> Self {
        let interrupts = Rc::new(RefCell::new(InterruptController::new()));
        Bus {
            timer: Timer::new(Rc::clone(&interrupts)),
            joypad: Joypad::new(Rc::clone(&interrupts)),
            cpu
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        unimplemented!()
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        unimplemented!()
    }

    pub fn advance(&mut self, cycles: u64) {
        unimplemented!()
    }
}