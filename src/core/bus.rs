use std::cell::RefCell;
use std::rc::{Rc, Weak};
use log::warn;
use crate::core::cpu::Cpu;
use crate::core::interrupts::{Interrupt, InterruptController};
use crate::core::joypad::Joypad;
use crate::core::memory::cartridge::Cartridge;
use crate::core::timers::Timer;
use crate::core::memory::dma::DmaController;
use crate::core::memory::WRAM;

pub(crate) struct BusController(Weak<RefCell<Bus>>);

impl BusController {
    pub fn new(bus: Weak<RefCell<Bus>>) -> Self {
        BusController(bus)
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().write(addr, val);
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().read(addr)
        } else { 0xFF }
    }
}

pub(crate) struct Bus {
    timer: Rc<RefCell<Timer>>,
    joypad: Rc<RefCell<Joypad>>,
    cpu: Option<Rc<RefCell<Cpu>>>,
    dma: Option<Rc<RefCell<DmaController>>>,
    cartridge: Rc<RefCell<Cartridge>>,
    interrupts: Rc<RefCell<InterruptController>>,
    wram: WRAM,
    hram: Vec<u8>
    // TODO GPU
}

impl Bus {
    pub fn new(timer: Rc<RefCell<Timer>>, joypad: Rc<RefCell<Joypad>>,
               interrupts: Rc<RefCell<InterruptController>>, cartridge: Rc<RefCell<Cartridge>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Bus {
            timer,
            joypad,
            cpu: None,
            dma: None,
            cartridge,
            interrupts,
            wram: WRAM::new(),
            hram: vec![0; 0x7F]
        }))
    }

    pub fn get_controller(this: &Rc<RefCell<Self>>) -> BusController {
        BusController::new(Rc::downgrade(this))
    }

    pub fn set_cpu(&mut self, cpu: Rc<RefCell<Cpu>>) {
        self.cpu = Some(cpu);
    }

    pub fn set_dma_controller(&mut self, dma: Rc<RefCell<DmaController>>) {
        self.dma = Some(dma);
    }

    pub fn write_io(&self, addr: u16, val: u8) {
        match addr {
            0xFF01 => print!("{}", val as char),
            0xFF04 => (*self.timer).borrow_mut().divider = 0,
            0xFF05 => (*self.timer).borrow_mut().tima = val,
            0xFF06 => (*self.timer).borrow_mut().tma = val,
            0xFF07 => (*self.timer).borrow_mut().set_tac(val),
            0xFF0F => (*self.interrupts).borrow_mut().set_interrupt_request(val),
            _ => warn!("Write of value 0x{:X} to I/O port 0x{:X} unhandled", val, addr)
        }
    }

    pub fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (*self.timer).borrow().divider,
            0xFF05 => (*self.timer).borrow().tima,
            0xFF06 => (*self.timer).borrow_mut().tma,
            0xFF07 => (*self.timer).borrow_mut().tac(),
            0xFF0F => (*self.interrupts).borrow_mut().get_interrupt_request(),
            _ => {
                warn!("Read from I/O port 0x{:X} unhandled", addr);
                0xFF
            }
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => (*self.cartridge).borrow().read(addr),
            0x8000..=0x9FFF => 0x00,
            0xC000..=0xDFFF => self.wram.read(addr),
            0xE000..=0xFDFF => self.wram.read(addr - 0x2000),
            0xFE00..=0xFE9F => 0x00,
            0xFEA0..=0xFEFF => 0x00,
            0xFF00..=0xFF7F => self.read_io(addr),
            0xFF80..=0xFFFE => self.hram[addr as usize - 0xFF80],
            0xFFFF => (*self.interrupts).borrow().get_interrupt_enable(),
            _ => unimplemented!()
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => (*self.cartridge).borrow_mut().write(addr, val),
            0x8000..=0x9FFF => {  },
            0xC000..=0xDFFF => self.wram.write(addr, val),
            0xE000..=0xFDFF => self.wram.write(addr - 0x2000, val),
            0xFE00..=0xFE9F => {},
            0xFEA0..=0xFEFF => {},
            0xFF00..=0xFF7F => self.write_io(addr, val),
            0xFF80..=0xFFFE => self.hram[addr as usize - 0xFF80] = val,
            0xFFFF => (*self.interrupts).borrow_mut().set_interrupt_enable(val),
            _ => unimplemented!()
        }
    }


}