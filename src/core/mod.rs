use std::cell::RefCell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use crate::core::audio::{Apu};
use crate::core::bus::Bus;
use crate::core::cpu::Cpu;
use crate::core::gpu::{Ppu, PpuState};
use crate::core::interrupts::InterruptController;
use crate::core::joypad::{Joypad, Key};
use crate::core::memory::cartridge::Cartridge;
use crate::core::memory::dma::DmaController;
use crate::core::timers::Timer;
use crate::core::utils::Counter;

mod traits;
pub(in crate::core) mod interrupts;
pub mod joypad;
mod timers;
mod bus;
pub mod cpu;
mod memory;
mod gpu;
mod audio;
mod utils;

pub struct GameBoy {
    joypad: Rc<RefCell<Joypad>>,
    bus: Rc<RefCell<Bus>>,
    cpu: Rc<RefCell<Cpu>>,
    ppu: Rc<RefCell<Ppu>>,
    apu: Rc<RefCell<Apu>>,
    dma: Rc<RefCell<DmaController>>,
    timer: Rc<RefCell<Timer>>,
    cartridge: Rc<RefCell<Cartridge>>,
    cycle_counter: u64
}

impl GameBoy {
    pub fn new(rom_path: PathBuf) -> io::Result<Self> {
        let cartridge = Rc::new(RefCell::new(Cartridge::open(rom_path)?));
        let interrupts = Rc::new(RefCell::new(InterruptController::new()));

        let is_cgb = (*cartridge).borrow().is_cgb();

        let timer = Rc::new(RefCell::new(Timer::new(Rc::clone(&interrupts))));
        let joypad = Rc::new(RefCell::new(Joypad::new(Rc::clone(&interrupts))));

        let ppu = Rc::new(RefCell::new(Ppu::new(Rc::clone(&interrupts))));
        let apu = Rc::new(RefCell::new(Apu::new(Rc::clone(&timer))));
        let bus = Bus::new(Rc::clone(&ppu), Rc::clone(&apu), Rc::clone(&timer), Rc::clone(&joypad),
                               Rc::clone(&interrupts), Rc::clone(&cartridge));

        let cpu = Rc::new(RefCell::new(Cpu::new(Rc::clone(&interrupts), Bus::get_controller(&bus), is_cgb)));
        let dma = Rc::new(RefCell::new(DmaController::new(Bus::get_controller(&bus))));
        {
            let mut b = (*bus).borrow_mut();
            b.set_cpu(Rc::clone(&cpu));
            b.set_dma_controller(Rc::clone(&dma));
        }
        Ok(Self { joypad, bus, cpu, ppu, apu, dma, timer, cartridge: Rc::clone(&cartridge), cycle_counter: 0 })
    }

    pub fn clock(&mut self) {
        use cpu::CpuState;
        if !matches!((*self.cpu).borrow().state(), &CpuState::Halted) {
            (*self.dma).borrow_mut().clock();
        }
        (*self.timer).borrow_mut().clock();
        (*self.ppu).borrow_mut().clock();
        (*self.apu).borrow_mut().clock();
        (*self.cpu).borrow_mut().clock();
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

    pub fn load_new_game(&mut self, rom_path: PathBuf) -> io::Result<()> {
        self.cartridge.replace(Cartridge::open(rom_path)?);

        (*self.bus).borrow_mut().reset();
        (*self.cpu).borrow_mut().reset();
        (*self.ppu).borrow_mut().reset();
        (*self.apu).borrow_mut().reset();
        (*self.timer).borrow_mut().reset();

        Ok(())
    }
}