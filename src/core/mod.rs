use std::cell::RefCell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use crate::core::bus::Bus;
use crate::core::cpu::Cpu;
use crate::core::interrupts::InterruptController;
use crate::core::joypad::Joypad;
use crate::core::memory::cartridge::Cartridge;
use crate::core::memory::dma::DmaController;
use crate::core::timers::Timer;

mod traits;
pub(in crate::core) mod interrupts;
pub(in crate::core) mod joypad;
mod timers;
mod bus;
pub mod cpu;
mod memory;

pub struct GameBoy {
    bus: Rc<RefCell<Bus>>,
    cpu: Rc<RefCell<Cpu>>,
    dma: Rc<RefCell<DmaController>>,
    timer: Rc<RefCell<Timer>>,
    cartridge: Rc<RefCell<Cartridge>>
}

impl GameBoy {
    pub fn new(rom_path: PathBuf) -> io::Result<Self> {
        let cartridge = Rc::new(RefCell::new(Cartridge::open(rom_path)?));
        let interrupts = Rc::new(RefCell::new(InterruptController::new()));

        let is_cgb = (*cartridge).borrow().is_cgb();

        let timer = Rc::new(RefCell::new(Timer::new(Rc::clone(&interrupts))));
        let joypad = Rc::new(RefCell::new(Joypad::new(Rc::clone(&interrupts))));

        let bus = Bus::new(Rc::clone(&timer), Rc::clone(&joypad),
                               Rc::clone(&interrupts), Rc::clone(&cartridge));

        let cpu = Rc::new(RefCell::new(Cpu::new(Rc::clone(&interrupts), Bus::get_controller(&bus), is_cgb)));
        let dma = Rc::new(RefCell::new(DmaController::new(Bus::get_controller(&bus))));
        {
            let mut b = (*bus).borrow_mut();
            b.set_cpu(Rc::clone(&cpu));
            b.set_dma_controller(Rc::clone(&dma));
        }
        Ok(Self { bus, cpu, dma, timer, cartridge: Rc::clone(&cartridge) })
    }

    fn clock(&mut self) {
        (*self.cpu).borrow_mut().clock();
        (*self.dma).borrow_mut().clock();
        (*self.timer).borrow_mut().clock();
    }

    pub fn cycle(&mut self) {
        for i in 0..=0xFFFF {
            self.clock()
        }
    }
}