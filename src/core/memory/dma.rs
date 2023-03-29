use crate::core::bus::BusController;

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
