use bitfield::bitfield;
use crate::core::audio::APUChannelReg;
use crate::core::utils::Counter;

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0]
];

bitfield! {
    pub struct NR10(u8);
    impl Debug;

    pub sweep_pace, _: 6, 4;
    pub sweep_direction, _: 3, 3;
    pub sweep_slope, _: 2, 0;
}

bitfield! {
    pub struct NRx1(u8);
    impl Debug;

    pub wave_duty, _: 7, 6;
    pub initial_length_timer, _: 5, 0;
}

bitfield! {
    pub struct NRx2(u8);
    impl Debug;

    pub initial_volume, _: 7, 4;
    pub direction, _: 3;
    pub sweep_pace, _: 2, 0;
}

#[derive(Debug)]
struct NRx3(pub u8);

bitfield! {
    pub struct NRx4(u8);
    impl Debug;

    pub trigger, _: 7;
    pub length_enable, _: 6;
    pub freq_hi, _: 5, 0;
}

struct Envelope {
    pub running: bool,
    counter: Counter
}

impl Envelope {
    pub fn new() -> Self {
        Self { running: false, counter: Counter::new(0, 8) }
    }

    pub fn step(&mut self, mut volume: u8, nr22: &NRx2) -> u8 {
        if self.counter.step() {
            self.counter.reset();
            self.counter.limit = match nr22.sweep_pace() as u32 {
                0 => 8,
                val => val
            };
            if self.running && nr22.sweep_pace() > 0 {
                if nr22.direction() {
                    if volume < 15 {
                        volume += 1;
                    }
                } else {
                    volume = volume.saturating_sub(1);
                }
            }
            self.running = volume > 0 && volume < 15;
        }

        volume
    }

    pub fn set_period(&mut self, period: u32) {
        self.counter.limit = period;
    }

    pub fn reset(&mut self) {
        self.counter.reset();
    }

}

// Square channel 1
pub struct Square1 {
    nr10: NR10,
    nr11: NRx1,
    nr12: NRx2,
    nr13: NRx3,
    nr14: NRx4,
    length_timer: Counter,
    freq_counter: Counter,
    freq_shadow: u32,
    envelope: Envelope,
    sweep: Counter,
    sweep_enable: bool,
    seq_pointer: usize,
    volume: u8,
    output: u8,
    enabled: bool,
    dac_enabled: bool
}

impl Square1 {
    pub fn new() -> Self {
        Self {
            nr10: NR10(0),
            nr11: NRx1(0x3F),
            nr12: NRx2(0),
            nr13: NRx3(0xFF),
            nr14: NRx4(0xBF),
            length_timer: Counter::new(64, 2),
            freq_counter: Counter::new(0, 1),
            freq_shadow: 0,
            envelope: Envelope::new(),
            seq_pointer: 0,
            sweep: Counter::new(0, 8),
            sweep_enable: false,
            volume: 0,
            output: 0,
            enabled: false,
            dac_enabled: false
        }
    }

    fn step_length_timer(&mut self) {
        if !self.length_timer.expired() && self.nr14.length_enable() {
            self.enabled = !self.length_timer.step();
        }
    }

    fn step_envelope(&mut self) {
        self.volume = self.envelope.step(self.volume, &self.nr12);
    }

    fn sweep_calc(&mut self) -> u32 {
        let new_freq = self.freq_shadow >> self.nr10.sweep_slope();
        self.freq_shadow = if self.nr10.sweep_direction() == 0 {
            self.freq_shadow.wrapping_add(new_freq)
        } else {
            self.freq_shadow.wrapping_sub(new_freq)
        };

        if new_freq > 2047 {
            self.enabled = false;
        }

        new_freq
    }

    fn step_sweep(&mut self) {
        if self.sweep.step() {
            let sweep_pace = self.nr10.sweep_pace() as u32;
            self.sweep.limit = if sweep_pace == 0 { 8 } else { sweep_pace };
            self.sweep.reset();

            if self.sweep_enable && self.nr10.sweep_pace() > 0 {
                let new_freq = self.sweep_calc();
                if new_freq < 2048 && self.nr10.sweep_slope() > 0 {
                    self.freq_shadow = new_freq;
                    self.freq_counter.limit = new_freq;
                    self.sweep_calc();
                }
                self.sweep_calc();
            }
        }
    }

    pub fn step_functions(&mut self) {
        self.step_length_timer();
        self.step_envelope();
        self.step_sweep();
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_timer.expired() {
            self.length_timer.reset();
            self.length_timer.limit = 64;
        }

        self.freq_counter.limit = 2048u32.saturating_sub(self.freq_counter.limit) << 2;
        self.freq_shadow = self.freq_counter.limit;
        self.freq_counter.reset();

        self.envelope.running = true;
        self.envelope.set_period(self.nr12.sweep_pace() as u32);
        self.envelope.reset();

        self.sweep.limit = self.nr10.sweep_pace() as u32;
        if self.sweep.limit == 0 {
            self.sweep.limit = 8;
        }
        self.sweep_enable = self.sweep.limit > 0 || self.nr10.sweep_slope() > 0;
        if self.nr10.sweep_slope() > 0 {
            let _ = self.sweep_calc();
        }

        self.volume = self.nr12.initial_volume();
        self.seq_pointer = 0;
    }

    pub fn is_running(&self) -> bool {
        self.enabled && self.dac_enabled
    }

    pub fn step(&mut self) {
        if self.freq_counter.step() {
            self.freq_counter.reset();
            let new_freq = (self.nr13.0 as u32) | ((self.nr14.freq_hi() as u32) << 8);
            self.freq_counter.limit = 2048u32.saturating_sub(new_freq) << 2;

            self.seq_pointer = (self.seq_pointer + 1) % 8;
        }

        self.output = if self.is_running() {
            let duty = DUTY_TABLE[self.nr11.wave_duty() as usize][self.seq_pointer];
            self.volume * duty
        } else {
            0
        };
    }

    pub(crate) fn read(&self, reg: APUChannelReg) -> u8 {
        match reg {
            APUChannelReg::NRx0 => self.nr10.0 & 0x7F,
            APUChannelReg::NRx1 => self.nr11.0,
            APUChannelReg::NRx2 => self.nr12.0,
            APUChannelReg::NRx3 => self.nr13.0,
            APUChannelReg::NRx4 => self.nr14.0 & 0xC7,
        }
    }

    pub(crate) fn write(&mut self, reg: APUChannelReg, val: u8) {
        match reg {
            APUChannelReg::NRx0 => {
                self.nr10.0 = val & 0x7F;
                self.sweep.limit = self.nr10.sweep_pace() as u32;
                self.sweep.reset();
            },
            APUChannelReg::NRx1 => {
                self.nr11.0 = val;
                self.length_timer.limit = 64 - self.nr11.initial_length_timer() as u32;
                self.length_timer.reset();
            },
            APUChannelReg::NRx2 => {
                self.nr12.0 = val;
                self.dac_enabled = val > 7;
                self.envelope.set_period(self.nr12.sweep_pace() as u32);
                self.envelope.reset();
                self.volume = self.nr12.initial_volume();
            },
            APUChannelReg::NRx3 => self.nr13.0 = val,
            APUChannelReg::NRx4 => {
                self.nr14.0 = val;
                if self.nr14.length_enable() {
                    if self.length_timer.expired() {
                        self.length_timer.reset();
                        self.length_timer.limit = 64;
                    }
                    self.enabled = true;
                }
                if self.nr14.trigger() {
                    self.trigger();
                }
            },
        }
    }

    pub fn output(&self) -> u8 {
        self.output
    }

    pub fn reset_counters(&mut self) {
        self.length_timer.reset();
        self.envelope.reset();
    }
}

// Square channel 2
pub struct Square2 {
    nr21: NRx1,
    nr22: NRx2,
    nr23: NRx3,
    nr24: NRx4,
    length_timer: Counter,
    freq_counter: Counter,
    envelope: Envelope,
    seq_pointer: usize,
    volume: u8,
    output: u8,
    enabled: bool,
    dac_enabled: bool
}

impl Square2 {
    pub fn new() -> Self {
        Self {
            nr21: NRx1(0x3F),
            nr22: NRx2(0),
            nr23: NRx3(0xFF),
            nr24: NRx4(0xBF),
            length_timer: Counter::new(64, 2),
            freq_counter: Counter::new(0, 1),
            envelope: Envelope::new(),
            seq_pointer: 0,
            volume: 0,
            output: 0,
            enabled: false,
            dac_enabled: false
        }
    }

    fn step_length_timer(&mut self) {
        if !self.length_timer.expired() && self.nr24.length_enable() {
            self.enabled = !self.length_timer.step();
        }
    }

    fn step_envelope(&mut self) {
        self.volume = self.envelope.step(self.volume, &self.nr22);
    }

    pub fn step_functions(&mut self) {
        self.step_length_timer();
        self.step_envelope();
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_timer.expired() {
            self.length_timer.reset();
            self.length_timer.limit = 64;
        }

        self.freq_counter.limit = 2048u32.saturating_sub(self.freq_counter.limit) << 2;
        self.freq_counter.reset();
        self.envelope.running = true;
        self.envelope.set_period(self.nr22.sweep_pace() as u32);
        self.envelope.reset();
        self.volume = self.nr22.initial_volume();
        self.seq_pointer = 0;
    }

    pub fn is_running(&self) -> bool {
        self.enabled && self.dac_enabled
    }

    pub fn step(&mut self) {
        if self.freq_counter.step() {
            self.freq_counter.reset();
            let new_freq = (self.nr23.0 as u32) | ((self.nr24.freq_hi() as u32) << 8);
            self.freq_counter.limit = 2048u32.saturating_sub(new_freq) << 2;

            self.seq_pointer = (self.seq_pointer + 1) % 8;
        }

        self.output = if self.is_running() {
            let duty = DUTY_TABLE[self.nr21.wave_duty() as usize][self.seq_pointer];
            self.volume * duty
        } else {
            0
        };
    }

    pub(crate) fn read(&self, reg: APUChannelReg) -> u8 {
        match reg {
            APUChannelReg::NRx1 => self.nr21.0,
            APUChannelReg::NRx2 => self.nr22.0,
            APUChannelReg::NRx3 => self.nr23.0,
            APUChannelReg::NRx4 => self.nr24.0,
            _ => 0xFF
        }
    }

    pub(crate) fn write(&mut self, reg: APUChannelReg, val: u8) {
        match reg {
            APUChannelReg::NRx1 => {
                self.nr21.0 = val;
                self.length_timer.limit = 64 - self.nr21.initial_length_timer() as u32;
                self.length_timer.reset();
            },
            APUChannelReg::NRx2 => {
                self.nr22.0 = val;
                self.dac_enabled = val > 7;
                self.envelope.set_period(self.nr22.sweep_pace() as u32);
                self.envelope.reset();
                self.volume = self.nr22.initial_volume();
            },
            APUChannelReg::NRx3 => self.nr23.0 = val,
            APUChannelReg::NRx4 => {
                self.nr24.0 = val;
                if self.nr24.length_enable() {
                    if self.length_timer.expired() {
                        self.length_timer.reset();
                        self.length_timer.limit = 64;
                    }
                    self.enabled = true;
                }
                if self.nr24.trigger() {
                    self.trigger();
                }
            },
            _ => ()
        }
    }

    pub fn output(&self) -> u8 {
        self.output
    }

    pub fn reset_counters(&mut self) {
        self.length_timer.reset();
        self.envelope.reset();
    }
}

// Wave channel
pub struct WaveChannel {
    nr30: u8,
    nr31: NRx1,
    nr32: u8,
    nr33: NRx3,
    nr34: NRx4,
    length_timer: Counter,
    freq_counter: Counter,
    pos: usize,
    output: u8,
    enabled: bool,
    wave_ram: [u8; 0x10]
}

impl WaveChannel {
    pub fn new() -> Self {
        Self {
            nr30: 0,
            nr31: NRx1(0x3F),
            nr32: 0,
            nr33: NRx3(0xFF),
            nr34: NRx4(0xBF),
            length_timer: Counter::new(64, 2),
            freq_counter: Counter::new(0, 1),
            pos: 0,
            output: 0,
            enabled: false,
            wave_ram: [0; 0x10]
        }
    }

    fn step_length_timer(&mut self) {
        if !self.length_timer.expired() && self.nr34.length_enable() {
            self.enabled = !self.length_timer.step();
        }
    }

    pub fn step_functions(&mut self) {
        self.step_length_timer();
    }

    pub(crate) fn read(&self, reg: APUChannelReg) -> u8 {
        match reg {
            APUChannelReg::NRx0 => self.nr30,
            APUChannelReg::NRx1 => self.nr31.0,
            APUChannelReg::NRx2 => self.nr32,
            APUChannelReg::NRx3 => self.nr33.0,
            APUChannelReg::NRx4 => self.nr34.0,
        }
    }

    pub(crate) fn write(&mut self, reg: APUChannelReg, val: u8) {
        match reg {
            APUChannelReg::NRx0 => {
                self.nr30 = val & 0x80;
            },
            APUChannelReg::NRx1 => {
                self.nr31.0 = val;
                self.length_timer.limit = 64 - self.nr31.initial_length_timer() as u32;
                self.length_timer.reset();
            },
            APUChannelReg::NRx2 => {
                self.nr32 = val & 0x60;
            },
            APUChannelReg::NRx3 => self.nr33.0 = val,
            APUChannelReg::NRx4 => {
                self.nr34.0 = val & 0xC7;
                if self.nr34.length_enable() {
                    if self.length_timer.expired() {
                        self.length_timer.reset();
                        self.length_timer.limit = 64;
                    }
                    self.enabled = true;
                }
                if self.nr34.trigger() {
                    self.trigger();
                }
            },
        }
    }

    pub fn write_wave_ram(&mut self, index: usize, val: u8) {
        if !self.enabled || index == self.pos {
            self.wave_ram[index] = val;
        }
    }

    pub fn read_wave_ram(&self, index: usize) -> u8 {
        if !self.enabled || index == self.pos {
            self.wave_ram[index]
        } else {
            0xFF
        }
    }

    pub fn output(&self) -> u8 {
        self.output
    }

    pub fn reset_counters(&mut self) {
        self.length_timer.reset();
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_timer.expired() {
            self.length_timer.reset();
            self.length_timer.limit = 64;
        }

        let new_freq = (self.nr33.0 as u32) | ((self.nr34.freq_hi() as u32) << 8);
        self.freq_counter.limit = 2048u32.saturating_sub(new_freq) << 1;
        self.freq_counter.reset();
        self.pos = 0;
    }

    pub fn step(&mut self) {
        if self.freq_counter.step() {
            self.freq_counter.reset();
            self.pos = (self.pos + 1) & 0x1F;
            if self.enabled && (self.nr30 & 0x80 != 0) {
                let mut output = self.wave_ram[self.pos >> 1];
                if (self.pos & 1) == 0 {
                    output >>= 4;
                }
                output &= 0xF;
                let output_level = (self.nr32 & 0x60) >> 5;
                self.output = if output_level > 0 {
                     output >> (output_level - 1)
                } else {
                    0
                }
            } else {
                self.output = 0;
            }
        }
    }

    pub fn clear_wave_pattern(&mut self) {
        self.wave_ram.fill(0);
    }

    pub fn is_running(&self) -> bool {
        self.enabled && (self.nr30 & 0x80 != 0)
    }
}

// Noise channel
bitfield! {
    pub struct NR43(u8);
    impl Debug;

    pub clock_shift, _: 7, 4;
    pub lfsr_width, _: 3;
    pub clock_divider, _: 2, 0;
}

static DIVISORS: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

pub struct Noise {
    nr41: NRx1,
    nr42: NRx2,
    nr43: NR43,
    nr44: NRx4,
    length_timer: Counter,
    freq_counter: Counter,
    envelope: Envelope,
    volume: u8,
    output: u8,
    enabled: bool,
    dac_enabled: bool,
    lfsr: u16,
}

impl Noise {
    pub fn new() -> Self {
        Self {
            nr41: NRx1(0xFF),
            nr42: NRx2(0),
            nr43: NR43(0),
            nr44: NRx4(0xBF),
            length_timer: Counter::new(64, 2),
            freq_counter: Counter::new(0, 1),
            envelope: Envelope::new(),
            volume: 0,
            output: 0,
            enabled: false,
            dac_enabled: false,
            lfsr: 0x7FFF,
        }
    }

    fn step_length_timer(&mut self) {
        if !self.length_timer.expired() && self.nr44.length_enable() {
            self.enabled = !self.length_timer.step();
        }
    }

    fn step_envelope(&mut self) {
        self.volume = self.envelope.step(self.volume, &self.nr42);
    }

    pub fn step_functions(&mut self) {
        self.step_length_timer();
        self.step_envelope();
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_timer.expired() {
            self.length_timer.reset();
            self.length_timer.limit = 64;
        }

        self.freq_counter.limit = DIVISORS[self.nr43.clock_divider() as usize] << self.nr43.clock_shift();
        self.freq_counter.reset();
        self.envelope.running = true;
        self.envelope.set_period(self.nr42.sweep_pace() as u32);
        self.envelope.reset();
        self.volume = self.nr42.initial_volume();
        self.lfsr = 0x7FFF;
    }

    pub fn is_running(&self) -> bool {
        self.enabled && self.dac_enabled
    }

    pub fn step(&mut self) {
        if self.freq_counter.step() {
            self.freq_counter.limit = DIVISORS[self.nr43.clock_divider() as usize] << self.nr43.clock_shift();
            self.freq_counter.reset();
            let res = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr >>= 1;
            self.lfsr |= res << 14;
            if self.nr43.lfsr_width() {
                self.lfsr &= !0x40;
                self.lfsr |= res << 6;
            }
            self.output = if self.is_running() && (self.lfsr & 1) == 0 {
                self.volume
            } else {
                0
            };
        }
    }

    pub(crate) fn read(&self, reg: APUChannelReg) -> u8 {
        match reg {
            APUChannelReg::NRx0 => self.nr41.0,
            APUChannelReg::NRx1 => self.nr42.0,
            APUChannelReg::NRx2 => self.nr43.0,
            APUChannelReg::NRx3 => self.nr44.0,
            _ => 0xFF
        }
    }

    pub(crate) fn write(&mut self, reg: APUChannelReg, val: u8) {
        match reg {
            APUChannelReg::NRx0 => {
                self.nr41.0 = val;
                self.length_timer.limit = 64 - self.nr41.initial_length_timer() as u32;
                self.length_timer.reset();
            },
            APUChannelReg::NRx1 => {
                self.nr42.0 = val;
                self.dac_enabled = (val & 0xF8) != 0;
                self.envelope.set_period(self.nr42.sweep_pace() as u32);
                self.envelope.reset();
                self.volume = self.nr42.initial_volume();
            },
            APUChannelReg::NRx2 => self.nr43.0 = val,
            APUChannelReg::NRx3 => {
                self.nr44.0 = val;
                if self.nr44.length_enable() {
                    if self.length_timer.expired() {
                        self.length_timer.reset();
                        self.length_timer.limit = 64;
                    }
                    self.enabled = true;
                }
                if self.nr44.trigger() {
                    self.trigger();
                }
            },
            _ => ()
        }
    }

    pub fn output(&self) -> u8 {
        self.output
    }

    pub fn reset_counters(&mut self) {
        self.length_timer.reset();
        self.envelope.reset();
    }
}
