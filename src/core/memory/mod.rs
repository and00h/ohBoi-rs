use crate::core::bus::BusController;

const DMA_SIZE: u16 = 0xA0;
const DMA_BASE_ADDR: u16 = 0xFE00;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DmaState {
    Triggered,
    Waiting,
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

    pub fn trigger(&mut self, index: u8) {
        self.state = DmaState::Triggered;

        self.mem_index = index;
        self.base_addr = (index as u16) << 8;
        self.dma_index = 0;
    }

    pub fn set_running(&mut self) {
        self.state = DmaState::Running;
    }

    pub fn state(&self) -> DmaState {
        self.state
    }

    pub fn clock(&mut self, mut cycles: u32) {
        debug_assert!(cycles % 4 == 0);
        while cycles > 0 {
            self.state = match self.state {
                DmaState::Running => {
                    self.bus.write(DMA_BASE_ADDR + self.dma_index, self.bus.read(self.base_addr + self.dma_index));
                    self.dma_index += 1;
                    if self.dma_index == DMA_SIZE { DmaState::Completed } else { DmaState::Running }
                },
                state @ _ => state
            };
            cycles -= 4;
        }
    }
}