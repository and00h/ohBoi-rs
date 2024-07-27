use std::cell::RefCell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use crate::core::audio::{Apu};
use crate::core::bus::Bus;
use crate::core::cpu::{Cpu, Speed};
use crate::core::ppu::{Ppu, PpuState};
use crate::core::interrupts::InterruptController;
use crate::core::joypad::{Joypad, Key};
use crate::core::memory::cartridge::Cartridge;
use crate::core::memory::dma::{DmaController, HdmaController, HdmaState};
use crate::core::timers::Timer;
use crate::core::utils::Counter;

macro_rules! rc_cell {
    ($s:ty) => {
        Rc<RefCell<$s>>
    };
}

macro_rules! rc_cell_new {
    ($s:expr) => {
        Rc::new(RefCell::new($s))
    };
}

mod traits;
pub(in crate::core) mod interrupts;
pub mod joypad;
mod timers;
mod bus;
pub mod cpu;
mod memory;
mod audio;
mod utils;
mod ppu;

pub struct GameBoy {
    joypad: rc_cell!(Joypad),
    bus: Rc<RefCell<Bus>>,
    cpu: Rc<RefCell<Cpu>>,
    ppu: Rc<RefCell<Ppu>>,
    apu: Rc<RefCell<Apu>>,
    dma: Rc<RefCell<DmaController>>,
    hdma_controller: Option<Rc<RefCell<HdmaController>>>,
    timer: Rc<RefCell<Timer>>,
    cartridge: Rc<RefCell<Cartridge>>,
    cycle_counter: u64,
    enable_audio_channels: (bool, bool, bool, bool)
}

impl GameBoy {
    pub fn new(rom_path: PathBuf) -> io::Result<Self> {
        let cartridge = rc_cell_new!(Cartridge::open(rom_path)?);
        let interrupts = rc_cell_new!(InterruptController::new());

        let is_cgb = (*cartridge).borrow().is_cgb();
        
        let timer = rc_cell_new!(Timer::new(Rc::clone(&interrupts)));
        let joypad = rc_cell_new!(Joypad::new(Rc::clone(&interrupts)));
        
        let ppu = rc_cell_new!(Ppu::new(Rc::clone(&interrupts), is_cgb));
        let apu = rc_cell_new!(Apu::new(Rc::clone(&timer)));

        let bus = Bus::new(Rc::clone(&ppu), Rc::clone(&apu), Rc::clone(&timer), Rc::clone(&joypad),
                               Rc::clone(&interrupts), Rc::clone(&cartridge));
        
        let cpu = rc_cell_new!(Cpu::new(Rc::clone(&interrupts), Bus::get_controller(&bus), is_cgb));
        let dma = rc_cell_new!(DmaController::new(Bus::get_controller(&bus)));
        
        let hdma_controller = if is_cgb {
            Some(rc_cell_new!(HdmaController::new(Bus::get_controller(&bus))))
        } else {
            None
        };
        {
            let mut b = (*bus).borrow_mut();
            b.set_cpu(Rc::clone(&cpu));
            b.set_dma_controller(Rc::clone(&dma));
            if is_cgb {
                b.set_hdma_controller(Rc::clone(hdma_controller.as_ref().unwrap()));
            }
        }
        Ok(Self { joypad, bus, cpu, ppu, apu, dma, hdma_controller, timer, cartridge: Rc::clone(&cartridge), cycle_counter: 0, enable_audio_channels: (true, true, true, true) })
    }

    pub fn clock(&mut self) {
        use cpu::CpuState;
        let cpu_speed = (*self.cpu).borrow().speed();
        let cpu_state = (*self.cpu).borrow().state().to_owned();
        let clocks = if matches!(cpu_speed, Speed::Double) { 2 } else { 1 };
        
        if matches!(cpu_speed, Speed::Switching(_)) && matches!(cpu_state, CpuState::Stopped(2050)) {
            let mut timer = (*self.timer).borrow_mut();
            timer.reset_counter();
        }

        (*self.ppu).borrow_mut().clock();
        (*self.apu).borrow_mut().clock();
        if let Some(hdma) = &self.hdma_controller {
            (*hdma).borrow_mut().clock();
            if matches!(self.cpu.borrow().state(), CpuState::HdmaHalted) &&
                matches!(hdma.borrow().state(), HdmaState::HBlankTransferFinishedBlock | HdmaState::Idle) {
                self.cpu.borrow_mut().hdma_continue();
            }
        }
        
        for _ in 0..clocks {
            if !matches!(cpu_state, CpuState::Halted) {
                (*self.dma).borrow_mut().clock();
            }
            if !matches!(cpu_state, CpuState::Stopped(_)) { (*self.timer).borrow_mut().clock(); }
            (*self.cpu).borrow_mut().clock();
        }
        self.cycle_counter += 4;
    }

    pub fn reset_cycle_counter(&mut self) {
        self.cycle_counter = 0;
    }

    pub fn is_in_vblank(&self) -> bool {
        matches!((*self.ppu).borrow_mut().state, PpuState::VBlank)
    }

    pub fn cycle_counter(&self) -> u64 {
        self.cycle_counter
    }

    pub fn screen(&self) -> Vec<u8> {
        (*self.ppu).borrow_mut().screen().to_owned()
    }

    pub fn press(&self, key: Key) {
        (*self.joypad).borrow_mut().press(key);
    }

    pub fn release(&self, key: Key) {
        (*self.joypad).borrow_mut().release(key);
    }

    pub fn audio_output(&self) -> Option<(f32, f32)> {
        (*self.apu).borrow_mut().get_current_output()
    }

    pub fn get_channels_output(&self) -> (f32, f32, f32, f32) {
        (*self.apu).borrow_mut().get_channels_output()
    }

    pub fn load_new_game(&mut self, rom_path: PathBuf) -> io::Result<()> {
        self.cartridge.replace(Cartridge::open(rom_path)?);

        (*self.bus).borrow_mut().reset();
        (*self.cpu).borrow_mut().reset();
        (*self.ppu).borrow_mut().reset();
        (*self.apu).borrow_mut().reset();
        (*self.timer).borrow_mut().reset();

        Ok(())
    }
    
    pub fn close_game(&self) {
        (*self.cartridge).borrow().save();
    }

    pub fn enable_audio_channel(&mut self, channel: u8, enable: bool) {
        match channel {
            0 => (*self.apu).borrow_mut().square1_enable = enable,
            1 => (*self.apu).borrow_mut().square2_enable = enable,
            2 => (*self.apu).borrow_mut().wave_enable = enable,
            3 => (*self.apu).borrow_mut().noise_enable = enable,
            _ => {}
        }
    }

    #[cfg(feature = "debug_ui")]
    pub fn rom(&self) -> &[u8] {
        unsafe { (*self.cartridge.as_ptr()).rom() }
    }
    
    #[cfg(feature = "debug_ui")]
    pub fn ext_ram(&self) -> Option<&[u8]> {
        unsafe { (*self.cartridge.as_ptr()).ext_ram() }
    }
    
    #[cfg(feature = "debug_ui")]
    pub fn get_tileset0(&self) -> Vec<u8> {
        (*self.ppu).borrow().get_tileset0()
    }
}