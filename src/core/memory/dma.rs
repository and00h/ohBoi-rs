use std::mem::transmute;
use log::debug;
use crate::core::bus::BusController;
use crate::core::ppu::PpuState;

const DMA_SIZE: u16 = 0xA0;
const DMA_BASE_ADDR: u16 = 0xFE00;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DmaState {
    Triggered,
    Waiting,
    RestartTriggered(u8),
    WaitingRestart(u8),
    Running,
    Completed
}

pub struct DmaController {
    bus: BusController,
    mem_index: u8,
    base_addr: u16,
    dma_index: u16,
    state: DmaState,
}

impl DmaController {
    pub(crate) fn new(bus: BusController) -> Self {
        DmaController {
            bus,
            mem_index: 0,
            base_addr: 0,
            dma_index: 0,
            state: DmaState::Completed
        }
    }

    fn do_trigger(&mut self, index: u8) {
        self.mem_index = index;
        self.base_addr = (index as u16) << 8;
        self.dma_index = 0;
    }
    pub fn trigger(&mut self, index: u8) {
        self.state = match self.state {
            DmaState::Completed => { self.do_trigger(index); DmaState::Triggered },
            _ => DmaState::RestartTriggered(index)
        };
    }

    pub fn is_addr_accessible(&self, addr: u16) -> bool {
        match self.state {
            DmaState::Completed | DmaState::Triggered | DmaState::Waiting => true,
            _ => ((addr & 0xFF00) != self.base_addr && !(0xFE00..=0xFE9F).contains(&addr)) || (0xFF80..=0xFFFE).contains(&addr)
        }
    }
    pub fn state(&self) -> DmaState {
        self.state
    }

    pub fn clock(&mut self) {
        self.state = match self.state {
            state @ (DmaState::Running | DmaState::RestartTriggered(_) | DmaState::WaitingRestart(_)) => {
                self.bus.dma_write(DMA_BASE_ADDR + self.dma_index, self.bus.dma_read(self.base_addr + self.dma_index));
                self.dma_index += 1;
                if self.dma_index == DMA_SIZE { DmaState::Completed } else {
                    match state {
                        DmaState::Running => if self.dma_index == DMA_SIZE { DmaState::Completed } else { DmaState::Running },
                        DmaState::RestartTriggered(mem_index) => DmaState::WaitingRestart(mem_index),
                        DmaState::WaitingRestart(mem_index) => {
                            self.do_trigger(mem_index);
                            DmaState::Running
                        },
                        _ => unreachable!()
                    }
                }
            },
            DmaState::Triggered => DmaState::Waiting,
            DmaState::Waiting => DmaState::Running,
            state => state
        };
    }

    pub fn mem_index(&self) -> u8 {
        self.mem_index
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum HdmaState {
    HBlankTransfer,
    HBlankTransferWait,
    HBlankTransferFinishedBlock,
    GdmaTransfer,
    Idle
}

pub struct HdmaController {
    bus: BusController,
    hdma1: u8,
    hdma2: u8,
    hdma3: u8,
    hdma4: u8,
    hdma5: u8,
    hdma_source: u16,
    hdma_dest: u16,
    hdma_len: u16,
    hdma_active: bool,
    hdma_index: u16,
    state: HdmaState
}

impl HdmaController {
    pub fn new(bus: BusController) -> Self {
        HdmaController {
            bus,
            hdma1: 0,
            hdma2: 0,
            hdma3: 0,
            hdma4: 0,
            hdma5: 0,
            hdma_source: 0,
            hdma_dest: 0,
            hdma_len: 0,
            hdma_active: false,
            hdma_index: 0,
            state: HdmaState::Idle
        }
    }
    
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF51 => self.hdma1 = val,
            0xFF52 => self.hdma2 = val & 0xF0,
            0xFF53 => self.hdma3 = val & 0x1F,
            0xFF54 => self.hdma4 = val & 0xF0,
            0xFF55 => { 
                if self.hdma_active && val & 0x80 == 0 { 
                    self.hdma_active = false;
                    self.hdma5 |= 0x80;
                    self.state = HdmaState::Idle;
                    return; 
                } else {
                    self.hdma5 = val;
                    self.start_hdma();   
                }
            },
            _ => panic!("Invalid write to HDMA controller at address {:04X}", addr)
        }
    }
    
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF51..=0xFF54 => 0xFF,
            0xFF55 => { 
                self.hdma5 
            },
            _ => panic!("Invalid read from HDMA controller at address {:04X}", addr)
        }
    }
    
    pub fn start_hdma(&mut self) {
        self.hdma_source = (self.hdma1 as u16) << 8 | (self.hdma2 as u16);
        self.hdma_dest = (((self.hdma3 as u16) << 8) | (self.hdma4 as u16)) + 0x8000;
        self.hdma_len = (((self.hdma5 & 0x7F) as u16) + 1) << 4;
        self.hdma_active = true;
        self.hdma_index = 0;
        if self.hdma5 & 0x80 == 0 {
            self.state = HdmaState::GdmaTransfer;
        } else {
            self.state = HdmaState::HBlankTransferWait;
        }
        self.hdma5 &= 0x7F;
        //debug!("HDMA transfer from {:04X} to {:04X} of length {}", self.hdma_source, self.hdma_dest, self.hdma_len);
        //debug!("HDMA type: {}", if self.hdma5 & 0x80 == 0 { "GDMA" } else { "HDMA" });
    }
    
    fn transfer(&mut self) {
        //debug!("HDMA transfer {}/{}: {:#04X} => {:#04X}", self.hdma_index, self.hdma_len, self.hdma_source + self.hdma_index, self.hdma_dest + self.hdma_index);
        for _ in 0..8 {
            let dst = (self.hdma_dest + self.hdma_index);
            if self.hdma_index == self.hdma_len || dst >= 0xA000 { break; }
            let val = self.bus.dma_read(self.hdma_source + self.hdma_index);
            self.bus.dma_write(dst, val);
            self.hdma_index += 1;
        }
    }
    
    pub fn clock(&mut self) {
        if !self.hdma_active { return; }
        match self.state {
            HdmaState::HBlankTransferWait => {
                let ppu_state: PpuState = unsafe { transmute(self.bus.read(0xFF41) & 0x03) }; 
                if matches!(ppu_state, PpuState::HBlank) { 
                    //debug!("HDMA transfer started");
                    self.state = HdmaState::HBlankTransfer; 
                }
            },
            HdmaState::HBlankTransferFinishedBlock => {
                let ppu_state: PpuState = unsafe { transmute(self.bus.read(0xFF41) & 0x03) };
                if !matches!(ppu_state, PpuState::HBlank) {
                    //debug!("HDMA transfer finished block");
                    self.state = HdmaState::HBlankTransferWait; 
                }
            },
            HdmaState::HBlankTransfer => {
                self.transfer();
                if self.hdma_index % 0x10 == 0 {
                    self.hdma5 = (((self.hdma_len - self.hdma_index) >> 4) as u8).wrapping_sub(1);
                    if self.hdma5 == 0xFF {
                        //debug!("HDMA HBlank transfer finished");
                        self.hdma_active = false;
                        self.state = HdmaState::Idle;
                    } else {
                        self.state = HdmaState::HBlankTransferFinishedBlock;
                    }
                }
                let dst = (self.hdma_dest + self.hdma_index);
                if dst >= 0xA000 { 
                    self.hdma_active = false;
                    self.hdma5 = 0xFF;
                    self.state = HdmaState::Idle;
                }
            },
            HdmaState::GdmaTransfer => {
                self.transfer();
                if self.hdma_index == self.hdma_len || self.hdma_dest + self.hdma_index >= 0xA000 {
                    //debug!("GDMA transfer finished");
                    self.hdma_active = false;
                    self.hdma5 = 0xFF;
                    self.state = HdmaState::Idle;
                }
            },
            HdmaState::Idle => {}
        }
    }
    
    pub fn state(&self) -> HdmaState {
        self.state
    }
}