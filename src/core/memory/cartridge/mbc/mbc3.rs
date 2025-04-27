use bitfield::bitfield;
use log::warn;
use crate::core::memory::cartridge::CartridgeHeader;
use crate::core::memory::cartridge::mbc::{RAM_BANK_SIZE, ROM_BANK_SIZE};
use super::Mbc;
use std::time::SystemTime;

bitfield! {
    struct RtcDayHi(u8);
    impl Debug;
    pub day_carry, set_day_carry: 7;
    pub halt, set_halt: 6;
    pub unused, _: 5, 1;
    pub day_hi, set_hi: 0;
}

impl RtcDayHi {
    pub fn new() -> Self {
        Self(0)
    }
}
struct Rtc {
    seconds: u8,
    minutes: u8,
    hours: u8,
    day: u8,
    day_hi: RtcDayHi,
    latched_seconds: u8,
    latched_minutes: u8,
    latched_hours: u8,
    latched_day: u8,
    latched_day_hi: RtcDayHi,
    current_time: SystemTime
}

impl Rtc {
    pub fn new() -> Self {
        Self {
            seconds: 0,
            minutes: 0,
            hours: 0,
            day: 0,
            day_hi: RtcDayHi::new(),
            latched_seconds: 0,
            latched_minutes: 0,
            latched_hours: 0,
            latched_day: 0,
            latched_day_hi: RtcDayHi::new(),
            current_time: SystemTime::now(),
        }
    }

    pub fn read_saved_time_from_buf(&mut self, buf: &[u8], size: usize) {
        if size < 40 {
            warn!("Buffer size is too small to read RTC data");
            return;
        }
        self.seconds = buf[0];
        self.minutes = buf[4];
        self.hours = buf[8];
        self.day = buf[12];
        self.day_hi.0 = buf[16];

        self.latched_seconds = buf[20];
        self.latched_minutes = buf[24];
        self.latched_hours = buf[28];
        self.latched_day = buf[32];
        self.latched_day_hi.0 = buf[36];

        if size > 40 {
            self.current_time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(u64::from_le_bytes(buf[40..48].try_into().unwrap()));
        } else {
            self.current_time = SystemTime::now();
        }
    }

    pub fn rtc_buffer_for_sav(&mut self) -> Vec<u8> {
        let mut buf = vec![0; 48];
        self.update_time();

        buf[0] = self.seconds;
        buf[4] = self.minutes;
        buf[8] = self.hours;
        buf[12] = self.day;
        buf[16] = self.day_hi.0;

        buf[20] = self.latched_seconds;
        buf[24] = self.latched_minutes;
        buf[28] = self.latched_hours;
        buf[32] = self.latched_day;
        buf[36] = self.latched_day_hi.0;

        let elapsed_secs = self.current_time.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
        buf[40..48].copy_from_slice(&elapsed_secs.to_le_bytes());

        buf
    }

    pub fn update_time(&mut self) {
        let now = SystemTime::now();
        if self.day_hi.halt() {
            return;
        }
        let new_time = now.duration_since(self.current_time);
        if let Ok(duration) = new_time {
            let difference = duration.as_secs() as u32;
            let new_secs = self.seconds as u32 + difference;
            let new_mins = self.minutes as u32 + (new_secs / 60);
            let new_hours = self.hours as u32 + (new_mins / 60);
            let new_days = self.day as u32 + (new_hours / 24);
            if new_secs == self.seconds as u32 {
                return;
            }
            self.seconds = (new_secs % 60) as u8;
            self.minutes = (new_mins % 60) as u8;
            self.hours = (new_hours % 24) as u8;
            let days = ((self.day_hi.day_hi() as u32) << 8) | (self.day as u32);
            let new_days = days + (new_hours / 24);
            self.day = (new_days % 256) as u8;
            self.day_hi.0 &= 0xFE;
            self.day_hi.set_hi((new_days >> 8) != 0);
            if new_days > 511 {
                self.day_hi.set_day_carry(true);
            }
        }
        self.current_time = now;
    }

    pub fn latch_time(&mut self) {
        self.latched_seconds = self.seconds;
        self.latched_minutes = self.minutes;
        self.latched_hours = self.hours;
        self.latched_day = self.day;
        self.latched_day_hi.0 = self.day_hi.0;
    }
}

pub(super) struct Mbc3 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    rtc: Option<Rtc>,
    ram_rtc_enabled: bool,
    ram_bank_rtc_reg: usize,
    battery: bool,
    rom_bank: usize,
    n_rom_banks: usize,
    n_ram_banks: usize,
    latch: bool
}

impl Mbc3 {
    pub fn new(rom: Vec<u8>, cart_header: &CartridgeHeader, sav: Option<Vec<u8>>, battery: bool, rtc: bool) -> Self {
        Self {
            rom,
            ram: match sav {
                Some(sav) => Some(sav),
                None if cart_header.ram_size != 0 => Some(vec![0; cart_header.ram_size]),
                _ => None
            },
            rtc: if rtc { Some(Rtc::new()) } else { None },
            ram_rtc_enabled: false,
            ram_bank_rtc_reg: 0,
            battery,
            rom_bank: 1,
            n_rom_banks: cart_header.rom_size / ROM_BANK_SIZE,
            n_ram_banks: cart_header.ram_size / RAM_BANK_SIZE,
            latch: false
        }
    }
}

impl Mbc for Mbc3 {
    fn read(&self, addr: u16) -> u8 {
        let mut bank_number = if addr < 0x4000 { 0 } else { self.rom_bank };

        bank_number %= self.n_rom_banks;
        let bank_offset = (addr as usize) % ROM_BANK_SIZE;
        let effective_address = ROM_BANK_SIZE * bank_number + bank_offset;

        self.rom[effective_address]
    }

    fn write(&mut self, addr: u16, val: u8) {
        let val = val as usize;
        match addr {
            0..=0x1FFF => self.ram_rtc_enabled = val & 0xF == 0xA,
            0x2000..=0x3FFF => {
                self.rom_bank = val & 0x7F;
                if self.rom_bank == 0 {
                    self.rom_bank = 1;
                }
            },
            0x4000..=0x5FFF => {
                self.ram_bank_rtc_reg = val & 0xF;
            },
            0x6000..=0x7FFF => {
                if let Some(ref mut rtc) = self.rtc {
                    let new_latch_value = val & 0x1 == 1;
                    if !self.latch && new_latch_value {
                        rtc.latch_time();
                    }
                    self.latch = new_latch_value;
                }
            },
            _ => {}
        }
    }

    fn read_ext_ram(&self, addr: u16) -> u8 {
        if !self.ram_rtc_enabled {
            return 0xFF
        }
        if self.ram_bank_rtc_reg < 4 {
            match self.ram {
                Some(ref ram) => {
                    let addr =
                        self.ram_bank_rtc_reg * RAM_BANK_SIZE + (addr as usize - 0xA000);
                    ram[addr]
                },
                _ => 0xFF
            }
        } else {
            match self.rtc {
                Some(ref rtc) => {
                    match self.ram_bank_rtc_reg {
                        0x8 => rtc.latched_seconds,
                        0x9 => rtc.latched_minutes,
                        0xA => rtc.latched_hours,
                        0xB => rtc.latched_day,
                        0xC => rtc.latched_day_hi.0,
                        _ => 0xFF
                    }
                },
                None => 0xFF
            }
        }
    }

    fn write_ext_ram(&mut self, addr: u16, val: u8) {
        if !self.ram_rtc_enabled {
            return
        }
        if self.ram_bank_rtc_reg < 4 {
            match self.ram {
                Some(ref mut ram) => {
                    let addr =
                        self.ram_bank_rtc_reg * RAM_BANK_SIZE + (addr as usize - 0xA000);
                    ram[addr] = val;
                },
                _ => {} // warn!("Tried writing to external RAM when cartridge has none")
            }
        } else {
            match self.rtc {
                Some(ref mut rtc) => {
                    match self.ram_bank_rtc_reg {
                        0x8 => rtc.seconds = val,
                        0x9 => rtc.minutes = val,
                        0xA => rtc.hours = val,
                        0xB => rtc.day = val,
                        0xC => rtc.day_hi.0 = val,
                        _ => {}
                    }
                },
                None => {}
            }
        }
    }

    fn has_battery(&self) -> bool {
        self.battery
    }
    fn has_ram(&self) -> bool {
        matches!(self.ram, Some(_))
    }

    fn ram(&self) -> Option<&Vec<u8>> {
        self.ram.as_ref()
    }

    fn rom(&self) -> &Vec<u8> {
        &self.rom
    }
}