use std::cell::RefCell;
use std::panic::Location;
use std::rc::{Rc, Weak};
use log::{debug, trace, warn};
use crate::core::audio::Apu;
use crate::core::cpu::{Cpu, Speed};
use crate::core::ppu::Ppu;
use crate::core::interrupts::InterruptController;
use crate::core::joypad::Joypad;
use crate::core::memory::cartridge::Cartridge;
use crate::core::timers::Timer;
use crate::core::memory::dma::{DmaController, HdmaController};
use crate::core::memory::WRAM;

pub(crate) struct BusController(Weak<RefCell<Bus>>);

impl BusController {
    pub fn new(bus: Weak<RefCell<Bus>>) -> Self {
        BusController(bus)
    }

    #[track_caller]
    pub fn write(&mut self, addr: u16, val: u8) {
        trace!("Write of {:02X} to {:04X} (requested by: {})", val, addr, Location::caller());
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().write(addr, val);
        }
    }

    #[track_caller]
    pub fn read(&self, addr: u16) -> u8 {
        trace!("Read from to {:04X} (requested by: {})", addr, Location::caller());
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().read(addr)
        } else { 0xFF }
    }

    pub fn dma_read(&self, addr: u16) -> u8 {
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().dma_read(addr)
        } else { 0xFF }
    }

    pub fn dma_write(&mut self, addr: u16, val: u8) {
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().dma_write(addr, val);
        }
    }
    
    pub fn hdma_halt_cpu(&mut self) {
        debug!("HDMA halt requested");
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().hdma_halt_cpu();
        }
    }
    
    pub fn hdma_restart_cpu(&mut self) {
        debug!("HDMA restart requested");
        if let Some(b) = self.0.upgrade() {
            (*b).borrow_mut().hdma_continue();
        }
    }
}

pub(crate) struct Bus {
    timer: Rc<RefCell<Timer>>,
    joypad: Rc<RefCell<Joypad>>,
    cpu: Option<Rc<RefCell<Cpu>>>,
    apu: Rc<RefCell<Apu>>,
    dma: Option<Rc<RefCell<DmaController>>>,
    hdma: Option<Rc<RefCell<HdmaController>>>,
    cartridge: Rc<RefCell<Cartridge>>,
    interrupts: Rc<RefCell<InterruptController>>,
    ppu: Rc<RefCell<Ppu>>,
    wram: WRAM,
    hram: Vec<u8>,
    iospace: Vec<u8>
}

impl Bus {
    pub fn new(ppu: Rc<RefCell<Ppu>>,
               apu: Rc<RefCell<Apu>>,
               timer: Rc<RefCell<Timer>>,
               joypad: Rc<RefCell<Joypad>>,
               interrupts: Rc<RefCell<InterruptController>>,
               cartridge: Rc<RefCell<Cartridge>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Bus {
            ppu,
            timer,
            joypad,
            cpu: None,
            apu,
            dma: None,
            hdma: None,
            cartridge,
            interrupts,
            wram: WRAM::new(),
            hram: vec![0; 0x7F],
            iospace: vec![0; 0x80]
        }))
    }

    pub fn reset(&mut self) {
        self.hram = vec![0; 0x7F];
        self.iospace = vec![0; 0x80];
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
    
    pub fn set_hdma_controller(&mut self, hdma: Rc<RefCell<HdmaController>>) {
        self.hdma = Some(hdma);
    }
    
    fn write_io(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF00 => (*self.joypad).borrow_mut().select_key_group(val),
            0xFF01 => print!("{}", val as char),
            0xFF02 => print!("{}", val as char),
            0xFF04 => (*self.timer).borrow_mut().reset_counter(),
            0xFF05 => (*self.timer).borrow_mut().set_tima(val),
            0xFF06 => (*self.timer).borrow_mut().set_tma(val),
            0xFF07 => (*self.timer).borrow_mut().set_tac(val),
            0xFF0F => (*self.interrupts).borrow_mut().set_interrupt_request(val),
            0xFF10..=0xFF3F => (*self.apu).borrow_mut().write(addr, val),
            0xFF46 => (*self.dma.as_ref().unwrap()).borrow_mut().trigger(val),
            0xFF51..=0xFF55 => {
                if let Some(hdma) = self.hdma.as_ref() {
                    hdma.borrow_mut().write(addr, val);
                }
                if addr == 0xFF55 {
                    unsafe {
                        (*self.cpu.as_ref().unwrap().as_ptr()).hdma_halt();
                    }
                }
            },
            0xFF4D => {
                warn!("Speed switch requested");
                unsafe {
                    if let Some(cpu) = self.cpu.as_ref() {
                        (*cpu.as_ptr()).arm_speed_switch();
                    }
                }
            },
            0xFF40..=0xFF4F | 0xFF68..=0xFF6B => (*self.ppu).borrow_mut().write(addr, val, false),
            0xFF70 => self.wram.switch_bank(val as usize & 0b111),
            _ => {
                warn!("Write of value 0x{:X} to I/O port 0x{:X} unhandled", val, addr);
                self.iospace[addr as usize - 0xFF00] = val;
            }
        }
    }

    fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xFF00 => (*self.joypad).borrow().get_key_register(),
            0xFF02 => 0xFF,
            0xFF03 => (*self.timer).borrow().divider_lo(),
            0xFF04 => (*self.timer).borrow().divider(),
            0xFF05 => (*self.timer).borrow().tima,
            0xFF06 => (*self.timer).borrow_mut().tma,
            0xFF07 => (*self.timer).borrow_mut().tac(),
            0xFF0F => (*self.interrupts).borrow_mut().get_interrupt_request(),
            0xFF10..=0xFF3F => (*self.apu).borrow().read(addr),
            0xFF46 => (*self.dma.as_ref().unwrap()).borrow().mem_index(),
            0xFF4D => {
                unsafe {
                    let cpu = self.cpu.as_ref().unwrap().as_ptr();
                    let mut res = if matches!((*cpu).speed(), Speed::Double) {
                        0x80
                    } else {
                        0x00
                    };
                    if (*cpu).is_speed_switching() { 
                        res |= 1; 
                    } 
                    res
                }
            }, 
            0xFF40..=0xFF4F | 0xFF68..=0xFF6B => (*self.ppu).borrow_mut().read(addr, false),
            0xFF50 => 1,
            0xFF51..=0xFF55 => if let Some(hdma) = self.hdma.as_ref() { 
                hdma.borrow().read(addr) 
            } else { 0xFF },
            0xFF70 => self.wram.bank1_index as u8,
            _ => {
                warn!("Read from I/O port 0x{:X} unhandled", addr);
                self.iospace[addr as usize - 0xFF00]
            }
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        if (*self.dma.as_ref().unwrap()).borrow().is_addr_accessible(addr) {
            match addr {
                0x0000..=0x7FFF | 0xA000..=0xBFFF => (*self.cartridge).borrow().read(addr),
                0x8000..=0x9FFF | 0xFE00..=0xFE9F => (*self.ppu).borrow().read(addr, false),
                0xC000..=0xDFFF => self.wram.read(addr),
                0xE000..=0xFDFF => self.wram.read(addr - 0x2000),
                0xFEA0..=0xFEFF => 0x00,
                0xFF00..=0xFF7F => self.read_io(addr),
                0xFF80..=0xFFFE => self.hram[addr as usize - 0xFF80],
                0xFFFF => (*self.interrupts).borrow().get_interrupt_enable(),
            }
        } else {
            0xFF
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if (*self.dma.as_ref().unwrap()).borrow().is_addr_accessible(addr) {
            match addr {
                0x0000..=0x7FFF | 0xA000..=0xBFFF => (*self.cartridge).borrow_mut().write(addr, val),
                0x8000..=0x9FFF | 0xFE00..=0xFE9F => (*self.ppu).borrow_mut().write(addr, val, false),
                0xC000..=0xDFFF => self.wram.write(addr, val),
                0xE000..=0xFDFF => self.wram.write(addr - 0x2000, val),
                0xFEA0..=0xFEFF => {},
                0xFF00..=0xFF7F => self.write_io(addr, val),
                0xFF80..=0xFFFE => self.hram[addr as usize - 0xFF80] = val,
                0xFFFF => (*self.interrupts).borrow_mut().set_interrupt_enable(val),
            }
        }
    }

    pub(in crate::core) fn dma_read(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9FFF | 0xFE00..=0xFE9F => (*self.ppu).borrow_mut().read(addr, true),
            0x0000..=0x7FFF | 0xA000..=0xBFFF => (*self.cartridge).borrow().read(addr),
            0xC000..=0xDFFF => self.wram.read(addr),
            0xE000..=0xFDFF => self.wram.read(addr - 0x2000),
            0xFEA0..=0xFEFF => 0x00,
            0xFF00..=0xFF7F => self.read_io(addr),
            0xFF80..=0xFFFE => self.hram[addr as usize - 0xFF80],
            0xFFFF => (*self.interrupts).borrow().get_interrupt_enable(),
        }
    }

    pub(in crate::core) fn dma_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x8000..=0x9FFF | 0xFE00..=0xFE9F => (*self.ppu).borrow_mut().write(addr, val, true),
            0x0000..=0x7FFF | 0xA000..=0xBFFF => (*self.cartridge).borrow_mut().write(addr, val),
            0xC000..=0xDFFF => self.wram.write(addr, val),
            0xE000..=0xFDFF => self.wram.write(addr - 0x2000, val),
            0xFEA0..=0xFEFF => {},
            0xFF00..=0xFF7F => self.write_io(addr, val),
            0xFF80..=0xFFFE => self.hram[addr as usize - 0xFF80] = val,
            0xFFFF => (*self.interrupts).borrow_mut().set_interrupt_enable(val),
        }
    }
    
    pub fn hdma_halt_cpu(&mut self) {
        self.cpu.as_ref().unwrap().borrow_mut().hdma_halt();
    }
    
    pub fn hdma_continue(&mut self) {
        self.cpu.as_ref().unwrap().borrow_mut().hdma_continue();
    }
}
