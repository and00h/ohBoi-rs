use std::cell::RefCell;
use std::rc::Rc;
use crate::core::timers::Timer;
pub struct Apu;

impl Apu {
    pub fn new(timer: Rc<RefCell<Timer>>) -> Self {
        Apu {}
    }

    pub fn read(&self, addr: u16) -> u8 {
        0
    }

    pub fn write(&mut self, addr: u16, data: u8) {}

    pub fn clock(&mut self) {}

    pub fn get_current_output(&self) -> Option<(f32, f32)> {
        Some((0.0, 0.0))
    }
}