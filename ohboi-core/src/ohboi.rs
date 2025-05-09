// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use std::cell::RefCell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use crate::audio::{Apu};
use crate::bus::Bus;
use crate::cpu::{Cpu, Speed};
use crate::ppu::{Ppu, PpuState};
use crate::cpu::interrupts::InterruptController;
use crate::joypad::{Joypad, Key};
use crate::memory::cartridge::Cartridge;
use crate::memory::dma::{DmaController, HdmaController, HdmaState};
use crate::timers::Timer;

#[cfg(feature = "debugging")]
use crate::cpu::Registers;

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
    stopped: bool,
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
        Ok(Self { joypad, bus, cpu, ppu, apu, dma, hdma_controller, timer, cartridge: Rc::clone(&cartridge), cycle_counter: 0, stopped: false })
    }

    pub fn clock(&mut self) {
        if self.stopped {
            return;
        }
        use crate::cpu::CpuState;
        let speed_switch_armed = (*self.cpu).borrow().is_speed_switching();
        let cpu_speed = (*self.cpu).borrow().speed();
        let cpu_state = (*self.cpu).borrow().state().to_owned();
        let clocks = if matches!(cpu_speed, Speed::Double) { 2 } else { 1 };
        
        if speed_switch_armed && matches!(cpu_state, CpuState::Stopped(2051)) {
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
            } else if matches!(self.cpu.borrow().state(), CpuState::Halting | CpuState::Halted) {
                self.cpu.borrow_mut().hdma_halt();
            }
        }
        
        for _ in 0..clocks {
            if !matches!(cpu_state, CpuState::Halted) {
                (*self.dma).borrow_mut().clock();
            }
            if !matches!(cpu_state, CpuState::Stopped(_) | CpuState::HdmaHalted) { (*self.timer).borrow_mut().clock(); }
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
}

#[cfg(feature = "debugging")] 
impl GameBoy {
    pub fn rom(&self) -> &[u8] {
        unsafe { (*self.cartridge.as_ptr()).rom() }
    }

    pub fn ext_ram(&self) -> Option<&[u8]> {
        unsafe { (*self.cartridge.as_ptr()).ext_ram() }
    }

    pub fn get_tileset0(&self) -> Vec<u8> {
        (*self.ppu).borrow().get_tileset0()
    }

    pub fn debug_stop(&mut self) {
        self.stopped = true;
    }

    pub fn debug_continue(&mut self) {
        self.stopped = false;
    }

    pub fn debug_step(&mut self) {
        self.stopped = false;
        let cur_pc = (*self.cpu).borrow().get_current_inst_pc();
        while cur_pc == (*self.cpu).borrow().get_current_inst_pc() {
            self.clock();
        }
        self.stopped = true;
    }

    pub fn get_cpu_registers(&self) -> Registers {
        (*self.cpu).borrow().get_registers()
    }

    pub fn get_current_instruction_window(&self) -> Vec<(usize, String)> {
        (*self.cpu).borrow().get_current_instructions(None)
    }

    pub fn get_current_instr_pc(&self) -> usize {
        (*self.cpu).borrow().get_current_inst_pc() as usize
    }

    pub fn is_running(&self) -> bool {
        !self.stopped
    }

    pub fn tiles(&self) -> Vec<u8> {
        let mut tiles = (*self.ppu).borrow().get_tileset0();
        if let Some(t) = (*self.ppu).borrow().get_tileset1() {
            tiles.append(&mut t.to_owned());
        } else {
            let mut empty = vec![0u8; tiles.len()];
            tiles.append(&mut empty)
        }

        tiles
    }
}