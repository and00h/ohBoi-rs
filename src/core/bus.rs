use std::cell::RefCell;
use std::rc::{Rc, Weak};
use crate::core::cpu::Cpu;
use crate::core::interrupts::InterruptController;
use crate::core::joypad::Joypad;
use crate::core::timers::Timer;
use crate::core::memory::DmaController;

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
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().advance(cycles);
        }
    }
}

pub(crate) struct Bus {
    timer: Timer,
    joypad: Joypad,
    cpu: Rc<RefCell<Cpu>>,
    dma: Rc<RefCell<DmaController>>
}

impl Bus {
    pub fn new(cpu: Rc<RefCell<Cpu>>, dma: Rc<RefCell<DmaController>>) -> Self {
        let interrupts = Rc::new(RefCell::new(InterruptController::new()));
        Bus {
            timer: Timer::new(Rc::clone(&interrupts)),
            joypad: Joypad::new(Rc::clone(&interrupts)),
            cpu,
            dma
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        unimplemented!()
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        unimplemented!()
    }

    pub fn advance(&mut self, cycles: u64) {
        self.timer.clock(cycles);
        
    }
}