use std::mem::transmute;
use crate::core::bus::BusController;
use crate::core::ppu::PpuState;

const DMA_SIZE: u16 = 0xA0; // The size of the DMA transfer (160 bytes).
const DMA_BASE_ADDR: u16 = 0xFE00; // The base address for DMA operations.

/// Represents the state of the DMA controller.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DmaState {
    /// DMA transfer has been triggered.
    Triggered,
    /// DMA is waiting to start.
    Waiting,
    /// DMA restart has been triggered with a specific memory index.
    RestartTriggered(u8),
    /// DMA is waiting to restart with a specific memory index.
    WaitingRestart(u8),
    /// DMA transfer is currently running.
    Running,
    /// DMA transfer has completed.
    Completed,
}

/// DMA Controller responsible for managing Direct Memory Access operations.
pub struct DmaController {
    /// The bus controller used for memory operations.
    bus: BusController,
    /// The memory index for the current DMA operation.
    mem_index: u8,
    /// The base address for the current DMA operation.
    base_addr: u16,
    /// The current index within the DMA transfer.
    dma_index: u16,
    /// The current state of the DMA controller.
    state: DmaState,
}

impl DmaController {
    /// Creates a new `DmaController` instance.
    ///
    /// # Arguments
    ///
    /// * `bus` - The bus controller to be used for DMA operations.
    pub(crate) fn new(bus: BusController) -> Self {
        DmaController {
            bus,
            mem_index: 0,
            base_addr: 0,
            dma_index: 0,
            state: DmaState::Completed,
        }
    }

    /// Internal method to trigger a DMA transfer.
    ///
    /// # Arguments
    ///
    /// * `index` - The memory index to start the transfer from.
    fn do_trigger(&mut self, index: u8) {
        self.mem_index = index;
        self.base_addr = (index as u16) << 8;
        self.dma_index = 0;
    }

    /// Triggers a DMA transfer or schedules a restart if a transfer is already in progress.
    ///
    /// # Arguments
    ///
    /// * `index` - The memory index to start the transfer from.
    pub fn trigger(&mut self, index: u8) {
        self.state = match self.state {
            DmaState::Completed => {
                self.do_trigger(index);
                DmaState::Triggered
            }
            _ => DmaState::RestartTriggered(index),
        };
    }

    /// Checks if a given memory address is accessible during the current DMA state.
    ///
    /// # Arguments
    ///
    /// * `addr` - The memory address to check.
    ///
    /// # Returns
    ///
    /// `true` if the address is accessible, `false` otherwise.
    pub fn is_addr_accessible(&self, addr: u16) -> bool {
        match self.state {
            DmaState::Completed | DmaState::Triggered | DmaState::Waiting => true,
            _ => {
                ((addr & 0xFF00) != self.base_addr && !(0xFE00..=0xFE9F).contains(&addr))
                    || (0xFF80..=0xFFFE).contains(&addr)
            }
        }
    }

    /// Returns the current state of the DMA controller.
    ///
    /// # Returns
    ///
    /// The current `DmaState`.
    pub fn state(&self) -> DmaState {
        self.state
    }

    /// Advances the DMA controller by one clock cycle, performing any necessary operations.
    pub fn clock(&mut self) {
        self.state = match self.state {
            state @ (DmaState::Running | DmaState::RestartTriggered(_) | DmaState::WaitingRestart(_)) => {
                self.bus.dma_write(
                    DMA_BASE_ADDR + self.dma_index,
                    self.bus.dma_read(self.base_addr + self.dma_index),
                );
                self.dma_index += 1;
                if self.dma_index == DMA_SIZE {
                    DmaState::Completed
                } else {
                    match state {
                        DmaState::Running => {
                            if self.dma_index == DMA_SIZE {
                                DmaState::Completed
                            } else {
                                DmaState::Running
                            }
                        }
                        DmaState::RestartTriggered(mem_index) => DmaState::WaitingRestart(mem_index),
                        DmaState::WaitingRestart(mem_index) => {
                            self.do_trigger(mem_index);
                            DmaState::Running
                        }
                        _ => unreachable!(),
                    }
                }
            }
            DmaState::Triggered => DmaState::Waiting,
            DmaState::Waiting => DmaState::Running,
            state => state,
        };
    }

    /// Returns the memory index for the current DMA operation.
    ///
    /// # Returns
    ///
    /// The memory index.
    pub fn mem_index(&self) -> u8 {
        self.mem_index
    }
}

/// Represents the state of the HDMA controller.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum HdmaState {
    /// HDMA transfer during HBlank period.
    HBlankTransfer,
    /// Waiting for HBlank to start the transfer.
    HBlankTransferWait,
    /// Finished a block of HBlank transfer.
    HBlankTransferFinishedBlock,
    /// General DMA transfer.
    GdmaTransfer,
    /// HDMA controller is idle.
    Idle,
}

/// HDMA Controller responsible for managing High-Speed Direct Memory Access operations.
pub struct HdmaController {
    /// The bus controller used for memory operations.
    bus: BusController,
    /// HDMA source high byte.
    hdma1: u8,
    /// HDMA source low byte.
    hdma2: u8,
    /// HDMA destination high byte.
    hdma3: u8,
    /// HDMA destination low byte.
    hdma4: u8,
    /// HDMA length/mode register.
    hdma5: u8,
    /// Calculated HDMA source address.
    hdma_source: u16,
    /// Calculated HDMA destination address.
    hdma_dest: u16,
    /// Length of the HDMA transfer.
    hdma_len: u16,
    /// Indicates if the HDMA is active.
    hdma_active: bool,
    /// Current index within the HDMA transfer.
    hdma_index: u16,
    /// Current state of the HDMA controller.
    state: HdmaState,
}

impl HdmaController {
    /// Creates a new `HdmaController` instance.
    ///
    /// # Arguments
    ///
    /// * `bus` - The bus controller to be used for HDMA operations.
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
            state: HdmaState::Idle,
        }
    }

    /// Writes a value to the HDMA controller registers.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address of the register to write to.
    /// * `val` - The value to write.
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
            }
            _ => panic!("Invalid write to HDMA controller at address {:04X}", addr),
        }
    }

    /// Reads a value from the HDMA controller registers.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address of the register to read from.
    ///
    /// # Returns
    ///
    /// The value of the register.
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF51..=0xFF54 => 0xFF,
            0xFF55 => self.hdma5,
            _ => panic!("Invalid read from HDMA controller at address {:04X}", addr),
        }
    }

    /// Starts an HDMA transfer based on the current register values.
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
    }

    /// Performs a block of HDMA transfer.
    fn transfer(&mut self) {
        for _ in 0..8 {
            let dst = self.hdma_dest + self.hdma_index;
            if self.hdma_index == self.hdma_len || dst >= 0xA000 {
                break;
            }
            let val = self.bus.dma_read(self.hdma_source + self.hdma_index);
            self.bus.dma_write(dst, val);
            self.hdma_index += 1;
        }
    }

    /// Advances the HDMA controller by one clock cycle, performing any necessary operations.
    pub fn clock(&mut self) {
        if !self.hdma_active {
            return;
        }
        match self.state {
            HdmaState::HBlankTransferWait => {
                let ppu_state: PpuState = unsafe { transmute(self.bus.read(0xFF41) & 0x03) };
                if matches!(ppu_state, PpuState::HBlank) {
                    self.state = HdmaState::HBlankTransfer;
                }
            }
            HdmaState::HBlankTransferFinishedBlock => {
                let ppu_state: PpuState = unsafe { transmute(self.bus.read(0xFF41) & 0x03) };
                if !matches!(ppu_state, PpuState::HBlank) {
                    self.state = HdmaState::HBlankTransferWait;
                }
            }
            HdmaState::HBlankTransfer => {
                self.transfer();
                if self.hdma_index % 0x10 == 0 {
                    self.hdma5 = (((self.hdma_len - self.hdma_index) >> 4) as u8).wrapping_sub(1);
                    if self.hdma5 == 0xFF {
                        self.hdma_active = false;
                        self.state = HdmaState::Idle;
                    } else {
                        self.state = HdmaState::HBlankTransferFinishedBlock;
                    }
                }
                let dst = self.hdma_dest + self.hdma_index;
                if dst >= 0xA000 {
                    self.hdma_active = false;
                    self.hdma5 = 0xFF;
                    self.state = HdmaState::Idle;
                }
            }
            HdmaState::GdmaTransfer => {
                self.transfer();
                if self.hdma_index == self.hdma_len || self.hdma_dest + self.hdma_index >= 0xA000 {
                    self.hdma_active = false;
                    self.hdma5 = 0xFF;
                    self.state = HdmaState::Idle;
                }
            }
            HdmaState::Idle => {}
        }
    }

    /// Returns the current state of the HDMA controller.
    ///
    /// # Returns
    ///
    /// The current `HdmaState`.
    pub fn state(&self) -> HdmaState {
        self.state
    }
}