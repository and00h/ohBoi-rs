// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

mod instructions;
mod prefixed_insts;
mod registers;
pub use registers::*;

use cfg_if::cfg_if;
use std::cell::RefCell;
use std::collections::vec_deque::VecDeque;
use std::fmt::{Debug};
use std::rc::Rc;
use log::{debug, trace, warn};

use crate::{
    bus::BusController,
    interrupts::{Interrupt, InterruptController}
};

use registers::Register8::*;
use registers::Register16::*;
use instructions::{InstArg, INSTRUCTIONS};
use prefixed_insts::PREFIXED_INSTS;

cfg_if! { 
    if #[cfg(feature = "debugging")] {
        use std::collections::HashMap;
        use strfmt::strfmt;
        use instructions::{MNEMONICS, NARGS};
        use prefixed_insts::PREFIXED_MNEMONICS;
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub(crate) enum CpuFlag {
    Zero        = 0b10000000,
    Sub         = 0b01000000,
    HalfCarry   = 0b00100000,
    Carry       = 0b00010000
}

#[derive(Debug, Copy, Clone)]
pub enum Speed {
    Normal,
    Double,
}

pub(crate) static INTERRUPT_VECTORS: [(Interrupt, u16); 5] = [
    (Interrupt::Joypad, 0x60),
    (Interrupt::Serial, 0x58),
    (Interrupt::Timer, 0x50),
    (Interrupt::Lcd, 0x48),
    (Interrupt::Vblank, 0x40),
];

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum CpuState {
    Fetching { halt_bug: bool },
    Decoding(u8),
    Halting,
    Halted,
    HdmaHalted,
    StartedExecution,
    ReadArg,
    ReadArgLo,
    ReadArgHi(u8),
    ALU16ReadHi,
    ALU16WriteHi(bool),
    ALU16AddSPSignedLo(u8),
    ALU16AddSPSignedHi(u8),
    LoadHLSPOffset(u8),
    UpdatePC(u16),
    BranchDecision,
    Internal,
    ReadMemory(u16),
    ReadMemoryLo(u16),
    ReadMemoryHi(u16, u8),
    WriteMemory(u16, u8),
    WriteMemoryLo(u16, u8),
    WriteMemoryHi(u16, u8),
    PushHi,
    PushLo,
    PopHi,
    PopLo,
    Stopped(usize),
    FinishedExecution,
    EnablingInterrupts,
    ServicingInterrupts,
    InterruptWaitState1,
    InterruptWaitState2,
    InterruptPushPCLo,
    InterruptPushPCHi,
    InterruptUpdatePC
}

impl CpuState {
    pub fn is_intermediate(&self) -> bool {
        matches!(self, Self::Decoding(_)
            | Self::StartedExecution | Self::FinishedExecution
            | Self::EnablingInterrupts | Self::ServicingInterrupts | Self::Halting)
    }
}

pub(crate) struct Cpu {
    interrupt_controller: Rc<RefCell<InterruptController>>,
    bus: BusController,
    state: CpuState,

    registers: Registers,
    pc: u16,
    sp: u16,

    instruction: fn(&mut Cpu),
    instruction_arg: InstArg,

    interrupts_to_handle: VecDeque<u16>,
    speed: Speed,

    cycles: u64,
    elapsed: u64,
    cgb: bool,
    speed_switch_armed: bool,
    #[cfg(feature = "debugging")]
    current_inst_pc: u16
}

impl Cpu {
    pub fn new(interrupt_controller: Rc<RefCell<InterruptController>>, bus: BusController, cgb: bool) -> Self {
        trace!("Building CPU");
        Self {
            interrupt_controller,
            bus,
            state: CpuState::ServicingInterrupts,
            registers: Registers::new(cgb),
            pc: 0x0100,
            sp: 0xFFFE,
            instruction: INSTRUCTIONS[0],
            instruction_arg: InstArg::None,
            interrupts_to_handle: VecDeque::new(),
            speed: Speed::Normal,
            cycles: 0,
            elapsed: 0,
            cgb,
            speed_switch_armed: false,
            #[cfg(feature = "debugging")]
            current_inst_pc: 0x0100
        }
    }

    pub fn reset(&mut self) {
        debug!("Resetting CPU");
        self.sp = 0xFFFE;
        self.pc = 0x0100;
        self.state = CpuState::Fetching { halt_bug: false };

        self.speed = Speed::Normal;
        self.speed_switch_armed = false;
        self.cycles = 0;
        self.registers.reset();
        (*self.interrupt_controller).borrow_mut().reset();
        
        #[cfg(feature = "debugging")] {
            self.current_inst_pc = 0x0100;
        }
    }
    
    pub fn speed(&self) -> Speed {
        self.speed
    }

    fn fetch(&mut self, halt_bug: bool) {
        let opcode = self.bus.read(self.pc);
        #[cfg(feature = "debugging")] {
            self.current_inst_pc = self.pc;
        }
        if !halt_bug { self.pc += 1; }
        self.state = CpuState::Decoding(opcode);
    }

    fn decode(&mut self, opcode: u8) {
        self.instruction = INSTRUCTIONS[opcode as usize];
        self.state = CpuState::StartedExecution;
        self.cycles += self.elapsed;
        self.elapsed = 0;
        debug!("Starting execution of instruction {:02X} at cycle {}", opcode, self.cycles);
    }

    fn halting(&mut self) {
        let i = (*self.interrupt_controller).borrow();
        if i.ime {
            self.state = if i.interrupts_pending() { CpuState::ServicingInterrupts } else { CpuState::Halted };
        } else if i.interrupts_pending() {
            self.state = CpuState::Fetching { halt_bug: true }
        } else {
            self.state = CpuState::Halted;
        }
    }

    fn check_interrupts(&mut self) -> CpuState {
        let mut interrupts = (*self.interrupt_controller).borrow_mut();
        let pending = interrupts.interrupts_pending();
        self.interrupts_to_handle = if interrupts.ime && pending {
            interrupts.ime = false;
            INTERRUPT_VECTORS.iter().filter_map(|(int, vector_addr)| {
                if interrupts.is_enabled(*int) && interrupts.is_raised(*int) {
                    interrupts.serve(*int);
                    Some(*vector_addr)
                } else {
                    None
                }
            }).collect()
        } else {
            VecDeque::default()
        };
        if !self.interrupts_to_handle.is_empty() {
            CpuState::InterruptWaitState1
        } else {
            CpuState::Fetching { halt_bug: false }
        }
    }

    // Do not optimize away

    pub fn arm_speed_switch(&mut self) {
        core::hint::black_box(&self.speed);
        self.speed_switch_armed = true;
    }

    pub fn is_speed_switching(&self) -> bool {
        self.speed_switch_armed
    }

    fn interrupt_service_routine(&mut self) {
        self.state =
            match self.state {
                CpuState::ServicingInterrupts => { self.check_interrupts() },
                CpuState::InterruptWaitState1 => {
                    self.cycles += self.elapsed;
                    self.elapsed = 0;
                    debug!("Starting interrupt servicing at cycle {}", self.cycles);
                    CpuState::InterruptWaitState2
                },
                CpuState::InterruptWaitState2 => {
                    CpuState::InterruptPushPCHi
                },
                CpuState::InterruptPushPCHi => {
                    self.sp = self.sp.wrapping_sub(1);
                    self.bus.write(self.sp, (self.pc >> 8) as u8);
                    CpuState::InterruptPushPCLo
                },
                CpuState::InterruptPushPCLo => {
                    self.sp = self.sp.wrapping_sub(1);
                    self.bus.write(self.sp, (self.pc & 0xFF) as u8);
                    CpuState::InterruptUpdatePC
                },
                CpuState::InterruptUpdatePC => {
                    self.pc = self.interrupts_to_handle.pop_front().unwrap();
                    debug!("Finished servicing interrupt at cycle {}", self.cycles + self.elapsed);
                    self.cycles += self.elapsed;
                    self.elapsed = 0;
                    if self.interrupts_to_handle.is_empty() {
                        CpuState::Fetching { halt_bug: false }
                    } else {
                        CpuState::InterruptWaitState1
                    }
                }
                _ => unreachable!()
            }
    }

    pub(crate) fn advance(&mut self) {
        use self::CpuState::*;
        let old_state = self.state;
        match self.state {
            Fetching { halt_bug } => self.fetch(halt_bug),
            Decoding(opcode) => self.decode(opcode),
            EnablingInterrupts => self.execute_ei(),
            Halting => self.halting(),
            Halted =>
                if (*self.interrupt_controller).borrow().interrupts_pending() {
                    self.state = ServicingInterrupts;
                },
            FinishedExecution => {
                debug!("Finished execution at cycle {}", self.cycles + self.elapsed);
                self.state = ServicingInterrupts
            },
            ServicingInterrupts
            | InterruptWaitState1 | InterruptWaitState2
            | InterruptPushPCHi | InterruptPushPCLo
            | InterruptUpdatePC => self.interrupt_service_routine(),
            HdmaHalted => warn!("Dafuq"),
            _ => (self.instruction)(self)
        }
        debug_assert!(old_state != self.state || matches!(self.state, Stopped(_) | Halted | HdmaHalted));
        // trace!("{:?} => {:?}", old_state, self.state);
    }
    
    #[cfg(feature = "debugging")]
    pub fn get_current_instructions(&self, window_size: Option<i32>) -> Vec<(usize, String)> {
        let window_size = window_size.unwrap_or(16);
        let mut instructions = Vec::new();
        let mut i = 0;
        let mut cur_inst_addr = self.current_inst_pc;
        while i < window_size {
            let opcode = self.bus.read(cur_inst_addr);
            let mut vars = HashMap::new();
            let mut next_inst_addr = cur_inst_addr.wrapping_add(1);
            let mut format = String::from(MNEMONICS[opcode as usize]);
            match NARGS[opcode as usize] {
                1 => {
                    let arg = self.bus.read(cur_inst_addr.wrapping_add(1));
                    if opcode != 0xCB {
                        vars.insert("arg8".to_string(), format!("${:02X}", arg));
                    } else {
                        format = String::from(PREFIXED_MNEMONICS[arg as usize]);
                    }
                    next_inst_addr = cur_inst_addr.wrapping_add(2);
                },
                2 => {
                    let arg = self.bus.read(cur_inst_addr.wrapping_add(1));
                    let arg2 = self.bus.read(cur_inst_addr.wrapping_add(2));
                    let complete_arg = (arg2 as u16) << 8 | arg as u16;
                    vars.insert("arg16".to_string(), format!("${:04X}", complete_arg));
                    next_inst_addr = cur_inst_addr.wrapping_add(3);
                },
                _ => {}
            }
            instructions.push((cur_inst_addr as usize, strfmt(&format, &vars).unwrap()));
            if next_inst_addr < cur_inst_addr {
                break;
            }
            cur_inst_addr = next_inst_addr;
            i += 1;
        }
        instructions
    }
    
    #[cfg(feature = "debugging")]
    pub fn get_current_inst_pc(&self) -> u16 {
        self.current_inst_pc
    }

    #[cfg(feature = "debugging")]
    pub fn get_registers(&self) -> Registers {
        self.registers.clone()
    }
    
    pub fn hdma_halt(&mut self) {
        self.state = CpuState::HdmaHalted;
    }
    
    pub fn hdma_continue(&mut self) {
        if !matches!(self.state, CpuState::HdmaHalted) {
            panic!("What the fuck");
        }
        self.state = CpuState::ServicingInterrupts;
    }
    pub fn state(&self) -> &CpuState {
        &self.state
    }

    pub fn clock(&mut self) {
        loop {
            self.advance();
            if !self.state.is_intermediate() { break }
        }
        self.elapsed += 1;
    }

    fn execute_ei(&mut self) {
        (*self.interrupt_controller).borrow_mut().ime = true;
        self.state = CpuState::Fetching { halt_bug: false };
    }

    // Instructions

    fn load_word_imm(&mut self, reg: Register16) {
        let (hi, lo) = Register8::from_word_reg(reg);
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArgLo,
                CpuState::ReadArgLo => {
                    self.registers.set_reg8(lo, self.bus.read(self.pc));
                    self.pc += 1;
                    CpuState::ReadArgHi(0)
                },
                CpuState::ReadArgHi(_) => {
                    self.registers.set_reg8(hi, self.bus.read(self.pc));
                    self.pc += 1;
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            };
    }

    fn load_word_sp(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArgLo,
                CpuState::ReadArgLo => {
                    self.sp = (self.sp & 0xFF00) | self.bus.read(self.pc) as u16;
                    self.pc += 1;
                    CpuState::ReadArgHi(0xFF)
                },
                CpuState::ReadArgHi(_) => {
                    self.sp = ((self.bus.read(self.pc) as u16) << 8) | (self.sp & 0xFF);
                    self.pc += 1;
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn inc_sp(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => {
                    let (data, overflow) = ((self.sp & 0xFF) as u8).overflowing_add(1);
                    self.sp = (self.sp & 0xFF00) | data as u16;
                    CpuState::ALU16WriteHi(overflow)
                },
                CpuState::ALU16WriteHi(overflow) => {
                    if overflow {
                        self.sp = self.sp.wrapping_add(0x100);
                    }
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn dec_sp(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => {
                    let (data, overflow) = ((self.sp & 0xFF) as u8).overflowing_sub(1);
                    self.sp = (self.sp & 0xFF00) | data as u16;
                    CpuState::ALU16WriteHi(overflow)
                },
                CpuState::ALU16WriteHi(overflow) => {
                    if overflow {
                        self.sp = self.sp.wrapping_sub(0x100);
                    }
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn inc_hl_indirect(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadMemory(self.registers.get_reg16(HL)),
                CpuState::ReadMemory(addr) => {
                    let data = self.bus.read(addr).wrapping_add(1);
                    CpuState::WriteMemory(addr, data)
                },
                CpuState::WriteMemory(addr, data) => {
                    self.registers.set_flag_cond(CpuFlag::HalfCarry, (data & 0xF) == 0);
                    self.registers.set_flag_cond(CpuFlag::Zero, data == 0);
                    self.registers.reset_flag(CpuFlag::Sub);

                    self.bus.write(addr, data);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn dec_hl_indirect(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadMemory(self.registers.get_reg16(HL)),
                CpuState::ReadMemory(addr) => {
                    let data = self.bus.read(addr).wrapping_sub(1);
                    CpuState::WriteMemory(addr, data)
                },
                CpuState::WriteMemory(addr, data) => {
                    self.registers.set_flag_cond(CpuFlag::HalfCarry, (data & 0xF) == 0xF);
                    self.registers.set_flag_cond(CpuFlag::Zero, data == 0);
                    self.registers.set_flag(CpuFlag::Sub);
                    self.bus.write(addr, data);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn store_hl_immediate(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArg,
                CpuState::ReadArg => {
                    let data = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::WriteMemory(self.registers.get_reg16(HL), data)
                },
                CpuState::WriteMemory(addr, data) => {
                    self.bus.write(addr, data);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn store_indirect(&mut self, reg: Register16) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::WriteMemory(self.registers.get_reg16(reg), self.registers.get_reg8(A)),
                CpuState::WriteMemory(addr, val) => {
                    self.bus.write(addr, val);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn store_sp(&mut self) {
        self.state =
            match self.state  {
                CpuState::StartedExecution => CpuState::ReadArgLo,
                CpuState::ReadArgLo => {
                    let lo = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::ReadArgHi(lo)
                },
                CpuState::ReadArgHi(lo) => {
                    let hi = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::WriteMemoryLo(((hi as u16) << 8) | (lo as u16), (self.sp & 0xFF) as u8)
                },
                CpuState::WriteMemoryLo(addr, sp) => {
                    self.bus.write(addr, sp);
                    CpuState::WriteMemoryHi(addr + 1, (self.sp >> 8) as u8)
                },
                CpuState::WriteMemoryHi(addr, sp) => {
                    self.bus.write(addr, sp);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn inc16(&mut self, reg: Register16) {
        let (hi, lo) = Register8::from_word_reg(reg);
        self.state =
            match self.state {
                CpuState::StartedExecution => {
                    let (data, overflow) = self.registers.get_reg8(lo).overflowing_add(1);
                    self.registers.set_reg8(lo, data);
                    CpuState::ALU16WriteHi(overflow)
                },
                CpuState::ALU16WriteHi(overflow) => {
                    if overflow {
                        let data = self.registers.get_reg8(hi).wrapping_add(1);
                        self.registers.set_reg8(hi, data);
                    }
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn inc8(&mut self, reg: Register8) {
        let data = self.registers.get_reg8(reg);

        self.registers.set_flag_cond(CpuFlag::HalfCarry, (data & 0xF) == 0xF);
        let (data, overflow) = data.overflowing_add(1);

        self.registers.set_flag_cond(CpuFlag::Zero, overflow);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.set_reg8(reg, data);
    }

    fn dec8(&mut self, reg: Register8) {
        let mut data = self.registers.get_reg8(reg);

        self.registers.set_flag_cond(CpuFlag::HalfCarry, (data & 0xF) == 0);
        data = data.wrapping_sub(1);

        self.registers.set_flag_cond(CpuFlag::Zero, data == 0);
        self.registers.set_flag(CpuFlag::Sub);

        self.registers.set_reg8(reg, data);
    }

    fn store_hl_inc(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::WriteMemory(self.registers.get_reg16(HL), self.registers.get_reg8(A)),
                CpuState::WriteMemory(addr, val) => {
                    self.bus.write(addr, val);
                    self.registers.set_reg16(HL, addr + 1);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn store_hl_dec(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::WriteMemory(self.registers.get_reg16(HL), self.registers.get_reg8(A)),
                CpuState::WriteMemory(addr, val) => {
                    self.bus.write(addr, val);
                    self.registers.set_reg16(HL, addr - 1);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn load_hl_inc(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadMemory(self.registers.get_reg16(HL)),
                CpuState::ReadMemory(addr) => {
                    self.registers.set_reg8(A, self.bus.read(addr));
                    self.registers.set_reg16(HL, addr + 1);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn load_hl_dec(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadMemory(self.registers.get_reg16(HL)),
                CpuState::ReadMemory(addr) => {
                    self.registers.set_reg8(A, self.bus.read(addr));
                    self.registers.set_reg16(HL, addr - 1);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn store_hl_imm(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArg,
                CpuState::ReadArg => {
                    let val = self.bus.read(self.pc);
                    self.pc += 1;
                    let addr = self.registers.get_reg16(HL);
                    CpuState::WriteMemory(addr, val)
                },
                CpuState::WriteMemory(addr, val) => {
                    self.bus.write(addr, val);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn add_hl_sp(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => {
                    let hl = self.registers.get_reg16(HL);

                    self.registers.set_flag_cond(CpuFlag::HalfCarry, (hl & 0x0FFF) > (0x0FFFu16.wrapping_sub(self.sp & 0x0FFF)));
                    self.registers.set_flag_cond(CpuFlag::Carry, hl > (0xFFFFu16.wrapping_sub(self.sp)));
                    self.registers.reset_flag(CpuFlag::Sub);

                    let (res, overflow) = self.registers.get_reg8(L).overflowing_add((self.sp & 0xFF) as u8);

                    self.registers.set_reg8(L, res);
                    CpuState::ALU16WriteHi(overflow)
                },
                CpuState::ALU16WriteHi(overflow) => {
                    let mut data = (self.sp >> 8) as u8;
                    if overflow { data = data.wrapping_add(1); }

                    self.registers.set_reg8(H, self.registers.get_reg8(H).wrapping_add(data));
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }


    fn zero_latency<F: FnMut(&mut Cpu)>(&mut self, mut f: F) {
        if let CpuState::StartedExecution = self.state {
            f(self);
            self.state = CpuState::FinishedExecution;
        } else {
            unreachable!()
        }
    }

    fn one_arg<F: FnMut(&mut Cpu, u8)>(&mut self, mut f: F) {
        self.state = match self.state {
            CpuState::StartedExecution => CpuState::ReadArgLo,
            CpuState::ReadArgLo => {
                let arg = self.bus.read(self.pc);
                self.pc += 1;
                f(self, arg);
                CpuState::FinishedExecution
            },
            _ => unreachable!()
        }
    }

    fn hl_src_reg_op<F: FnMut(&mut Cpu, u8)>(&mut self, mut f: F) {
        self.state = match self.state {
            CpuState::StartedExecution => CpuState::ReadMemory(self.registers.get_reg16(HL)),
            CpuState::ReadMemory(addr) => {
                let data = self.bus.read(addr);
                f(self, data);
                CpuState::FinishedExecution
            },
            _ => unreachable!()
        }
    }

    fn hl_dst_reg_op<F: FnMut(&mut Cpu) -> u8>(&mut self, mut f: F) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::WriteMemory(self.registers.get_reg16(HL), f(self)),
                CpuState::WriteMemory(addr, val) => {
                    self.bus.write(addr, val);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn adc(&mut self, val: u8) {
        let a = self.registers.get_reg8(A);
        let carry = if self.registers.test_flag(CpuFlag::Carry) {
            1
        } else {
            0
        };
        let (res, overflow1) = a.overflowing_add(val);
        let (res, overflow2) = res.overflowing_add(carry);

        self.registers.set_flag_cond(CpuFlag::Carry, overflow1 || overflow2);
        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (a & 0xF) + (val & 0xF) + carry > 0xF);
        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers.reset_flag(CpuFlag::Sub);

        self.registers.set_reg8(A, res);
    }

    fn add(&mut self, val: u8) {
        let a = self.registers.get_reg8(A);
        let res = a.wrapping_add(val);

        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (a & 0xF) + (val & 0xF) > 0xF);
        self.registers.set_flag_cond(CpuFlag::Carry, res < a);
        self.registers.reset_flag(CpuFlag::Sub);

        self.registers.set_reg8(A, res);
    }

    fn add_hl(&mut self, reg: Register16) {
        let (hi, lo) = Register8::from_word_reg(reg);
        self.state =
        match self.state {
            CpuState::StartedExecution => {
                let hl = self.registers.get_reg16(HL);
                let data = self.registers.get_reg16(reg);

                self.registers.set_flag_cond(CpuFlag::HalfCarry, (hl & 0x0FFF) > (0x0FFFu16.wrapping_sub(data & 0x0FFF)));
                self.registers.set_flag_cond(CpuFlag::Carry, hl > (0xFFFFu16.wrapping_sub(data)));
                self.registers.reset_flag(CpuFlag::Sub);

                let (res, overflow) = self.registers.get_reg8(L).overflowing_add(self.registers.get_reg8(lo));

                self.registers.set_reg8(L, res);
                CpuState::ALU16WriteHi(overflow)
            },
            CpuState::ALU16WriteHi(overflow) => {
                let mut data = self.registers.get_reg8(hi);
                if overflow { data = data.wrapping_add(1); }

                self.registers.set_reg8(H, self.registers.get_reg8(H).wrapping_add(data));
                CpuState::FinishedExecution
            },
            _ => unreachable!()
        }
    }

    fn and(&mut self, val: u8) {
        let res = self.registers.get_reg8(A) & val;

        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.set_flag(CpuFlag::HalfCarry);
        self.registers.reset_flag(CpuFlag::Carry);

        self.registers.set_reg8(A, res);
    }

    fn cb(&mut self) {
        self.state = match self.state {
            CpuState::StartedExecution => CpuState::ReadArg,
            CpuState::ReadArg => {
                let arg = self.bus.read(self.pc);
                debug!("Executing CB instruction {:02X}", arg);
                self.pc += 1;
                self.instruction_arg = InstArg::Byte(arg);
                PREFIXED_INSTS[arg as usize](self)
            },
            _ => {
                let arg = self.instruction_arg.lo();
                PREFIXED_INSTS[arg as usize](self)
            }
        }
    }

    fn ccf(&mut self) {
        let carry = self.registers.test_flag(CpuFlag::Carry);
        self.registers.set_flag_cond(CpuFlag::Carry, !carry);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
    }

    fn cp(&mut self, val: u8) {
        let a = self.registers.get_reg8(A);

        self.registers.set_flag_cond(CpuFlag::Zero, a == val);
        self.registers.set_flag(CpuFlag::Sub);
        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (a & 0xF) < (val & 0xF));
        self.registers.set_flag_cond(CpuFlag::Carry, a < val);
    }

    fn cpl(&mut self) {
        self.registers.set_flag(CpuFlag::Sub);
        self.registers.set_flag(CpuFlag::HalfCarry);
        let a = self.registers.get_reg8(A);
        self.registers.set_reg8(A, !a);
    }

    fn daa(&mut self) {
        let mut val = self.registers.get_reg8(A) as u32;
        if self.registers.test_flag(CpuFlag::Sub) {
            if self.registers.test_flag(CpuFlag::HalfCarry) {
                val = (val.wrapping_sub(6)) & 0xFF;
            }
            if self.registers.test_flag(CpuFlag::Carry) {
                val = (val.wrapping_sub(0x60)) & 0xFF;
            }
        } else {
            if self.registers.test_flag(CpuFlag::HalfCarry) || (val & 0xF) > 9 {
                val = val.wrapping_add(0x06);
            }
            if self.registers.test_flag(CpuFlag::Carry) || val > 0x9F {
                val = val.wrapping_add(0x60);
            }
        }

        self.registers.reset_flag(CpuFlag::HalfCarry);
        self.registers.reset_flag(CpuFlag::Zero);
        if val > 0xFF {
            self.registers.set_flag(CpuFlag::Carry);
        }
        val &= 0xFF;
        if val == 0 {
            self.registers.set_flag(CpuFlag::Zero);
        }
        self.registers.set_reg8(A, val as u8);
    }

    fn dec16(&mut self, reg: Register16) {
        let (hi, lo) = Register8::from_word_reg(reg);
        self.state =
        match self.state {
            CpuState::StartedExecution => {
                let (data, overflow) = self.registers.get_reg8(lo).overflowing_sub(1);
                self.registers.set_reg8(lo, data);
                CpuState::ALU16WriteHi(overflow)
            },
            CpuState::ALU16WriteHi(overflow) => {
                if overflow {
                    let data = self.registers.get_reg8(hi).wrapping_sub(1);
                    self.registers.set_reg8(hi, data);
                }
                CpuState::FinishedExecution
            },
            _ => unreachable!()
        };
    }

    fn di(&mut self) {
        (*self.interrupt_controller).borrow_mut().ime = false;
    }

    fn ei(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::EnablingInterrupts,
                _ => unreachable!()
            };
    }

    fn halt(&mut self) {
        if matches!(self.state, CpuState::StartedExecution) {
            self.state = CpuState::Halting
        }
    }

    fn jump_rel(&mut self, condition: bool) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArg,
                CpuState::ReadArg => {
                    let offset = self.bus.read(self.pc) as i8;
                    self.pc += 1;
                    if condition {
                        CpuState::UpdatePC(self.pc.wrapping_add(offset as u16))
                    } else {
                        CpuState::FinishedExecution
                    }
                },
                CpuState::UpdatePC(new_pc) => {
                    self.pc = new_pc;
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn jump_rel_conditional(&mut self, flag: CpuFlag, negate: bool) {
        let condition = self.registers.test_flag(flag) ^ negate;
        self.jump_rel(condition);
    }

    fn jump(&mut self, condition: bool) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArgLo,
                CpuState::ReadArgLo => {
                    let lo = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::ReadArgHi(lo)
                },
                CpuState::ReadArgHi(lo) => {
                    let addr = ((self.bus.read(self.pc) as u16) << 8) | (lo as u16);
                    self.pc += 1;
                    if condition {
                        CpuState::UpdatePC(addr)
                    } else {
                        CpuState::FinishedExecution
                    }
                },
                CpuState::UpdatePC(addr) => { self.pc = addr; CpuState::FinishedExecution },
                _ => unreachable!()
            };
    }

    fn jump_conditional(&mut self, flag: CpuFlag, negate: bool) {
        let condition = self.registers.test_flag(flag) ^ negate;
        self.jump(condition);
    }

    fn ret(&mut self, condition: bool) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadMemoryLo(self.sp),
                CpuState::ReadMemoryLo(addr) => {
                    let pc_lo = self.bus.read(addr);
                    self.sp += 1;
                    CpuState::ReadMemoryHi(self.sp, pc_lo)
                },
                CpuState::ReadMemoryHi(addr, pc_lo) => {
                    let new_pc = ((self.bus.read(addr) as u16) << 8) | (pc_lo as u16);
                    self.sp += 1;
                    CpuState::UpdatePC(new_pc)
                },
                CpuState::UpdatePC(new_pc) => {
                    self.pc = new_pc;
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn ret_conditional(&mut self, flag: CpuFlag, negate: bool) {
        let condition = self.registers.test_flag(flag) ^ negate;
        match self.state {
            CpuState::StartedExecution => self.state = CpuState::BranchDecision,
            CpuState::BranchDecision => self.state = {
                if condition {
                    CpuState::ReadMemoryLo(self.sp)
                } else {
                    CpuState::FinishedExecution
                }
            },
            _ => self.ret(condition)
        }
    }

    fn reti(&mut self) {
        self.ret(true);
        if let CpuState::FinishedExecution = self.state {
            (*self.interrupt_controller).borrow_mut().ime = true;
        }
    }

    fn pop(&mut self, reg: Register16) {
        let (hi, lo) = Register8::from_word_reg(reg);
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::PopLo,
                CpuState::PopLo => {
                    self.registers.set_reg8(lo, self.bus.read(self.sp));
                    self.sp += 1;
                    CpuState::PopHi
                },
                CpuState::PopHi => {
                    self.registers.set_reg8(hi, self.bus.read(self.sp));
                    self.sp += 1;
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn push(&mut self, reg: Register16) {
        let (hi, lo) = Register8::from_word_reg(reg);
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::Internal,
                CpuState::Internal => {
                    CpuState::PushHi
                },
                CpuState::PushHi => {
                    self.sp = self.sp.wrapping_sub(1);
                    self.bus.write(self.sp, self.registers.get_reg8(hi));
                    CpuState::PushLo
                },
                CpuState::PushLo => {
                    self.sp = self.sp.wrapping_sub(1);
                    self.bus.write(self.sp, self.registers.get_reg8(lo));
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn rst(&mut self, addr: u16) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::Internal,
                CpuState::Internal => {
                    CpuState::PushHi
                }
                CpuState::PushHi => {
                    self.sp = self.sp.wrapping_sub(1);
                    self.bus.write(self.sp, (self.pc >> 8) as u8);
                    CpuState::PushLo
                },
                CpuState::PushLo => {
                    self.sp = self.sp.wrapping_sub(1);
                    self.bus.write(self.sp, (self.pc & 0xFF) as u8);
                    self.pc = addr;
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn call(&mut self, condition: bool) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArgLo,
                CpuState::ReadArgLo => {
                    let lo = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::ReadArgHi(lo)
                },
                CpuState::ReadArgHi(lo) => {
                    self.instruction_arg = InstArg::Word(self.bus.read(self.pc), lo);
                    self.pc += 1;
                    if condition {
                        CpuState::Internal
                    } else {
                        CpuState::FinishedExecution
                    }
                },
                CpuState::Internal => {
                    CpuState::PushHi
                },
                CpuState::PushHi => {
                    self.sp -= 1;
                    self.bus.write(self.sp, (self.pc >> 8) as u8);
                    CpuState::PushLo
                },
                CpuState::PushLo => {
                    self.sp -= 1;
                    self.bus.write(self.sp, (self.pc & 0xFF) as u8);
                    self.pc = self.instruction_arg.word();

                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn call_conditional(&mut self, flag: CpuFlag, negate: bool) {
        let condition = self.registers.test_flag(flag) ^ negate;
        self.call(condition);
    }

    fn store_highmem_immediate(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArg,
                CpuState::ReadArg => {
                    let arg = self.bus.read(self.pc) as u16;
                    self.pc += 1;
                    CpuState::WriteMemory(0xFF00 + arg, self.registers.get_reg8(A))
                },
                CpuState::WriteMemory(addr, val) => {
                    self.bus.write(addr, val);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            };
    }

    fn store_highmem_reg(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => {
                    let offset = self.registers.get_reg8(C) as u16;
                    CpuState::WriteMemory(0xFF00 + offset, self.registers.get_reg8(A))
                },
                CpuState::WriteMemory(addr, val) => {
                    self.bus.write(addr, val);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            };
    }

    fn load_highmem_immediate(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArg,
                CpuState::ReadArg => {
                    let arg = self.bus.read(self.pc) as u16;
                    self.pc += 1;
                    CpuState::ReadMemory(0xFF00 + arg)
                },
                CpuState::ReadMemory(addr) => {
                    self.registers.set_reg8(A, self.bus.read(addr));
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            };
    }

    fn load_highmem_reg(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => {
                    let offset = self.registers.get_reg8(C) as u16;
                    CpuState::ReadMemory(0xFF00 + offset)
                },
                CpuState::ReadMemory(addr) => {
                    self.registers.set_reg8(A, self.bus.read(addr));
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            };
    }

    fn store_accumulator(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArgLo,
                CpuState::ReadArgLo => {
                    let lo = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::ReadArgHi(lo)
                },
                CpuState::ReadArgHi(lo) => {
                    let addr = ((self.bus.read(self.pc) as u16) << 8) | (lo as u16);
                    self.pc += 1;
                    CpuState::WriteMemory(addr, self.registers.get_reg8(A))
                },
                CpuState::WriteMemory(addr, val) => {
                    self.bus.write(addr, val);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn load_accumulator(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArgLo,
                CpuState::ReadArgLo => {
                    let lo = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::ReadArgHi(lo)
                },
                CpuState::ReadArgHi(lo) => {
                    let addr = ((self.bus.read(self.pc) as u16) << 8) | (lo as u16);
                    self.pc += 1;
                    CpuState::ReadMemory(addr)
                },
                CpuState::ReadMemory(addr) => {
                    self.registers.set_reg8(A, self.bus.read(addr));
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn add_sp(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArg,
                CpuState::ReadArg => {
                    let arg = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::ALU16AddSPSignedLo(arg)
                },
                CpuState::ALU16AddSPSignedLo(arg) => {
                    let sign_ext_imm = (arg as i8) as u16;
                    self.registers.set_flag_cond(
                        CpuFlag::HalfCarry,
                        (self.sp & 0xF) as u8 > (0xF - (arg & 0xF)),
                    );
                    self.registers
                        .set_flag_cond(CpuFlag::Carry, self.sp as u8 > 0xFF - arg);
                    let res = self.sp.wrapping_add(sign_ext_imm);
                    self.sp = (self.sp & 0xFF00) | (res & 0xFF);
                    self.registers.reset_flag(CpuFlag::Zero);
                    self.registers.reset_flag(CpuFlag::Sub);
                    CpuState::ALU16AddSPSignedHi((res >> 8) as u8)
                },
                CpuState::ALU16AddSPSignedHi(hi) => {
                    self.sp = ((hi as u16) << 8) | (self.sp & 0xFF);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn ldhl_sp_offset(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => CpuState::ReadArg,
                CpuState::ReadArg => {
                    let arg = self.bus.read(self.pc);
                    self.pc += 1;
                    CpuState::LoadHLSPOffset(arg)
                },
                CpuState::LoadHLSPOffset(offset) => {
                    let sign_ext_imm = (offset as i8) as u16;
                    self.registers.set_flag_cond(
                        CpuFlag::HalfCarry,
                        (self.sp & 0xF) as u8 > (0xF - (offset & 0xF)),
                    );
                    self.registers
                        .set_flag_cond(CpuFlag::Carry, self.sp as u8 > 0xFF - offset);
                    let res = self.sp.wrapping_add(sign_ext_imm);
                    self.registers.reset_flag(CpuFlag::Zero);
                    self.registers.reset_flag(CpuFlag::Sub);
                    self.registers.set_reg16(HL, res);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn ld_sp_hl(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => {
                    self.sp = (self.sp & 0xFF00) | (self.registers.get_reg8(L) as u16);
                    CpuState::Internal
                },
                CpuState::Internal => {
                    self.sp = ((self.registers.get_reg8(H) as u16) << 8) | (self.sp & 0xFF);
                    CpuState::FinishedExecution
                },
                _ => unreachable!()
            }
    }

    fn load_indirect(&mut self, reg: Register8, src: Register16) {
        self.state =
        match self.state {
            CpuState::StartedExecution => CpuState::ReadMemory(self.registers.get_reg16(src)),
            CpuState::ReadMemory(addr) => {
                self.registers.set_reg8(A, self.bus.read(addr));
                CpuState::FinishedExecution
            },
            _ => unreachable!()
        };
    }


    fn or(&mut self, val: u8) {
        let res = self.registers.get_reg8(A) | val;

        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
        self.registers.reset_flag(CpuFlag::Carry);

        self.registers.set_reg8(A, res);
    }

    fn rla(&mut self) {
        let c = if self.registers.test_flag(CpuFlag::Carry) {
            1
        } else {
            0
        };
        let mut a = self.registers.get_reg8(A);

        self.registers.set_flag_cond(CpuFlag::Carry, (a & 0x80) == 0x80);
        a <<= 1;
        a |= c;

        self.registers.set_reg8(A, a);

        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
    }

    fn rlca(&mut self) {
        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);

        let a = self.registers.get_reg8(A);
        self.registers.set_flag_cond(CpuFlag::Carry, (a & 0x80) == 0x80);
        self.registers.set_reg8(A, a.rotate_left(1));
    }

    fn rra(&mut self) {
        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);

        let c = if self.registers.test_flag(CpuFlag::Carry) { 1 << 7 } else { 0 };
        let mut a = self.registers.get_reg8(A);

        self.registers.set_flag_cond(CpuFlag::Carry, (a & 1) == 1);

        a >>= 1;
        a |= c;
        self.registers.set_reg8(A, a);
    }

    fn rrca(&mut self) {
        let a = self.registers.get_reg8(A);
        self.registers.set_reg8(A, a.rotate_right(1));

        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
        self.registers.set_flag_cond(CpuFlag::Carry, (a & 1) == 1);
    }


    fn sbc(&mut self, val: u8) {

        let a = self.registers.get_reg8(A);
        let carry = if self.registers.test_flag(CpuFlag::Carry) {
            1
        } else {
            0
        };
        let res = a.wrapping_sub(val).wrapping_sub(carry);

        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (a & 0xF) < (val & 0xF) + carry);
        self.registers
            .set_flag_cond(CpuFlag::Carry, (a as u16) < val as u16 + carry as u16);
        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers.set_flag(CpuFlag::Sub);

        self.registers.set_reg8(A, res);
    }

    fn scf(&mut self) {
        self.registers.set_flag(CpuFlag::Carry);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
    }

    fn stop(&mut self) {
        self.state =
            match self.state {
                CpuState::StartedExecution => {
                    CpuState::Stopped(2051)
                },
                CpuState::Stopped(remaining) => {
                    if remaining == 0 {
                        self.speed = if matches!(self.speed, Speed::Normal) { Speed::Double } else { Speed::Normal };
                        CpuState::FinishedExecution
                    } else {
                        CpuState::Stopped(remaining - 1)
                    }
                },
                _ => unreachable!()
            }
    }

    fn sub(&mut self, val: u8) {
        let a = self.registers.get_reg8(A);

        self.registers.set_flag_cond(CpuFlag::Zero, a == val);
        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (val & 0xF) > (a & 0xF));
        self.registers.set_flag_cond(CpuFlag::Carry, val > a);
        self.registers.set_flag(CpuFlag::Sub);

        self.registers.set_reg8(A, a.wrapping_sub(val));
    }

    fn xor(&mut self, val: u8) {
        let res = self.registers.get_reg8(A) ^ val;

        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
        self.registers.reset_flag(CpuFlag::Carry);

        self.registers.set_reg8(A, res);
    }
}
