// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

mod channels;

use std::cell::RefCell;
use std::rc::Rc;
use bitfield::bitfield;
use log::error;
use crate::audio::channels::{Noise, Square1, Square2, WaveChannel};
use crate::timers::Timer;
use crate::utils::{Counter, FallingEdgeDetector};

const READ_OR_VALUES: [u8; 23] = [
    0x80, 0x3F, 0x00, 0xFF, 0xBF,
    0xFF, 0x3F, 0x00, 0xFF, 0xBF,
    0x7F, 0xFF, 0x9F, 0xFF, 0xBF,
    0xFF, 0xFF, 0x00, 0x00, 0xBF,
    0x00, 0x00, 0x70
];

#[repr(u8)]
pub(crate) enum APUChannelReg {
    NRx0,
    NRx1,
    NRx2,
    NRx3,
    NRx4
}

impl From<u16> for APUChannelReg {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::NRx0,
            1 => Self::NRx1,
            2 => Self::NRx2,
            3 => Self::NRx3,
            4 => Self::NRx4,
            _ => panic!("Invalid value: {}", value)
        }
    }
}

bitfield! {
    pub struct NR52(u8);
    impl Debug;

    pub sound_on, _: 7;
    #[inline]
    square1_enable, _: 0;
    #[inline]
    square2_enable, _: 1;
    #[inline]
    wave_enable, _: 2;
    #[inline]
    noise_enable, _: 3;
}

bitfield! {
    pub struct NR51(u8);
    impl Debug;

    pub ch1_right, _: 0;
    pub ch2_right, _: 1;
    pub ch3_right, _: 2;
    pub ch4_right, _: 3;
    pub ch1_left, _: 4;
    pub ch2_left, _: 5;
    pub ch3_left, _: 6;
    pub ch4_left, _: 7;
}

bitfield! {
    pub struct NR50(u8);
    impl Debug;

    pub vin_left, _: 7;
    pub left_volume, _: 6, 4;
    pub vin_right, _: 3;
    pub right_volume, _: 2, 0;
}

pub struct Apu {
    timer: Rc<RefCell<Timer>>,
    falling_edge_detector: FallingEdgeDetector,
    nr50: NR50,
    nr51: NR51,
    nr52: NR52,
    downsample_counter: Counter,
    current_output: Option<(f32, f32)>,
    square1: Square1,
    square2: Square2,
    wave_ch: WaveChannel,
    noise: Noise,
    pub(crate) square1_enable: bool,
    pub(crate) square2_enable: bool,
    pub(crate) wave_enable: bool,
    pub(crate) noise_enable: bool
}

impl Apu {
    pub fn new(timer: Rc<RefCell<Timer>>) -> Self {
        let old = ((*timer).borrow().divider() & 0x10) == 0x10;

        Self {
            timer,
            falling_edge_detector: FallingEdgeDetector::with_initial_value(old),
            nr50: NR50(0x77),
            nr51: NR51(0xF3),
            nr52: NR52(0x81),
            downsample_counter: Counter::new(4194304 / 44100, 1),
            current_output: None,
            square1: Square1::new(),
            square2: Square2::new(),
            wave_ch: WaveChannel::new(),
            noise: Noise::new(),
            square1_enable: true,
            square2_enable: true,
            wave_enable: true,
            noise_enable: true
        }
    }

    pub fn clock(&mut self) {
        if !self.nr52.sound_on() {
            return;
        }
        let t = ((*self.timer).borrow().divider() & 0x10) == 0;
        if self.falling_edge_detector.detect(t) {
            self.square1.step_functions();
            self.square2.step_functions();
            self.wave_ch.step_functions();
            self.noise.step_functions();
        }
        for _ in 0..4 {
            self.square1.step();
            self.square2.step();
            self.wave_ch.step();
            self.noise.step();

            if self.downsample_counter.step() {
                self.downsample_counter.reset();

                let ch1_out = if self.square1_enable { self.square1.output() as f32 / 15.0 } else { 0.0 };
                let ch2_out = if self.square2_enable { self.square2.output() as f32 / 15.0 } else { 0.0 };
                let ch3_out = if self.wave_enable { self.wave_ch.output() as f32 / 15.0 } else { 0.0 };
                let ch4_out = if self.noise_enable { self.noise.output() as f32 / 15.0 } else { 0.0 };

                let volume_l = (self.nr50.left_volume() as f32) / 7.0;
                let volume_r = (self.nr50.right_volume() as f32) / 7.0;

                self.current_output.replace(((ch1_out + ch2_out + ch3_out + ch4_out) / 4.0 * volume_l, (ch1_out + ch2_out + ch3_out + ch4_out) / 4.0 * volume_r));
            }
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if !self.nr52.sound_on() && addr != 0xFF26 {
            return;
        }
        let reg: APUChannelReg = ((addr & 0xF) % 5).into();
        match addr {
            0xFF10..=0xFF14 => self.square1.write(reg, val),
            0xFF15 => {},
            0xFF16..=0xFF19 => self.square2.write(reg, val),
            0xFF1A..=0xFF1E => self.wave_ch.write(reg, val),
            0xFF1F..=0xFF23 => self.noise.write(reg, val),
            0xFF24 => self.nr50.0 = val,
            0xFF25 => self.nr51.0 = val,
            0xFF26 => {
                if self.nr52.sound_on() && (val & 0x80) == 0 {
                    for i in 0xFF10..=0xFF25 {
                        self.write(i, 0);
                    }
                } else if !self.nr52.sound_on() && (val & 0x80) != 0 {
                    self.square1.reset_counters();
                    self.square2.reset_counters();
                    self.wave_ch.reset_counters();
                    self.noise.reset_counters();
                    self.wave_ch.clear_wave_pattern();
                }
                self.nr52.0 = val
            },
            0xFF30..=0xFF3F => self.wave_ch.write_wave_ram(addr as usize & 0xF, val),
            _ => error!("Writing value {:02X} to invalid APU register {:04X}! Ignoring write", val, addr)
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        let reg: APUChannelReg = ((addr & 0xF) % 5).into();
        let or_value = if !(0xFF30..=0xFF3F).contains(&addr) {
            READ_OR_VALUES[(addr as usize & 0xFF) - 0x10]
        } else {
            0
        };

        let val = match addr {
            0xFF10..=0xFF14 => self.square1.read(reg),
            0xFF15 => 0xFF,
            0xFF16..=0xFF19 => self.square2.read(reg),
            0xFF1A..=0xFF1E => self.wave_ch.read(reg),
            0xFF1F..=0xFF23 => self.noise.read(reg),
            0xFF24 => self.nr50.0,
            0xFF25 => self.nr51.0,
            0xFF26 => {
                (if self.nr52.sound_on() { 0x80 } else { 0 })
                    | (if self.square1.is_running() { 0x1 } else { 0 })
                    | (if self.square2.is_running() { 0x2 } else { 0 })
                    | (if self.wave_ch.is_running() { 0x4 } else { 0 })
                    | (if self.noise.is_running() { 0x8 } else { 0 })
            },
            0xFF30..=0xFF3F => self.wave_ch.read_wave_ram(addr as usize & 0xF),
            _ => {
                error!("Reading from invalid APU register {:04X}!", addr);
                0xFF
            }
        };

        val | or_value
    }

    pub fn get_current_output(&mut self) -> Option<(f32, f32)> {
        self.current_output.take()
    }
    pub fn get_channels_output(&self) -> (f32, f32, f32, f32) {
        let ch1_out = if self.square1_enable { self.square1.output() as f32 / 15.0 } else { 0.0 };
        let ch2_out = if self.square2_enable { self.square2.output() as f32 / 15.0 } else { 0.0 };
        let ch3_out = if self.wave_enable { self.wave_ch.output() as f32 / 15.0 } else { 0.0 };
        let ch4_out = if self.noise_enable { self.noise.output() as f32 / 15.0 } else { 0.0 };

        (ch1_out, ch2_out, ch3_out, ch4_out)
    }

    pub fn reset(&mut self) {
        self.nr50.0 = 0x77;
        self.nr51.0 = 0xF3;
        self.nr52.0 = 0x81;
    }
}
