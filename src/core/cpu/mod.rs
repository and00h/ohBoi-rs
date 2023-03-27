mod instructions;
mod prefixed_insts;

use std::cell::{Ref, RefCell};
use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::rc::{Rc, Weak};
use log::{debug, trace, warn};
use crate::core::bus::{Bus, BusController};
use crate::core::interrupts::{Interrupt, InterruptController};

#[derive(Clone, Copy, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum Register8 {
    B = 0, C = 1, D = 2, E = 3, H = 4, L = 5, F = 6, A = 7
}

impl Register8 {
    pub fn from_word_reg(reg: Register16) -> (Self, Self) {
        match reg {
            Register16::AF => (A, F),
            Register16::BC => (B, C),
            Register16::DE => (D, E),
            Register16::HL => (H, L)
        }
    }
}

#[repr(usize)]
#[derive(Clone, Copy, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) enum Register16 {
    AF = 0x76, BC = 0x1, DE = 0x23, HL = 0x45
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub(crate) enum CpuFlag {
    Zero        = 0b10000000,
    Sub         = 0b01000000,
    HalfCarry   = 0b00100000,
    Carry       = 0b00010000
}

pub(crate) struct Registers {
    regs: Vec<u8>,
    cgb: bool
}

use Register8::*;
use Register16::*;
use crate::core::cpu::instructions::{InstArg, INSTRUCTIONS, InstructionType};
use crate::core::cpu::prefixed_insts::PREFIXED_INSTS;

impl Registers {
    pub fn new(cgb: bool) -> Self {
        Registers {
            regs: vec![
                0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D, 0xB0,
                if cgb { 0x11 } else { 0x01 }
            ],
            cgb
        }
    }

    pub fn reset(&mut self) {
        self.regs = vec![0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D, 0xB0, if self.cgb { 0x11 } else { 0x01 }];
    }

    #[inline]
    pub fn get_reg8(&self, reg: Register8) -> u8 {
        self.regs[reg as usize]
    }

    #[inline]
    pub fn set_reg8(&mut self, reg: Register8, data: u8) {
        self.regs[reg as usize] = if let F = reg {
            data & 0xF0
        } else {
            data
        };
    }

    #[inline]
    pub fn get_reg16(&self, reg: Register16) -> u16 {
        let (i1, i2) = ((reg as usize & 0xF0) >> 4, reg as usize & 0xF);
        let mut res = ((self.regs[i1] as u16) << 8) | self.regs[i2] as u16;
        if reg == AF {
            res &= 0xFFF0;
        }

        res
    }

    pub fn set_reg16(&mut self, reg: Register16, data: u16) {
        let (i1, i2) = ((reg as usize & 0xF0) >> 4, reg as usize & 0xF);
        self.regs[i1] = ((data & 0xFF00) >> 8) as u8;
        self.regs[i2] = if i2 == F as usize {
            data & 0xF0
        } else {
            data & 0xFF
        } as u8;
    }

    #[inline]
    pub fn load(&mut self, dst: Register8, src: Register8) {
        self.set_reg8(dst, self.get_reg8(src));
    }

    #[inline]
    pub fn set_flag_cond(&mut self, flag: CpuFlag, condition: bool) {
        if condition {
            self.regs[F as usize] |= flag as u8;
        } else {
            self.regs[F as usize] &= !(flag as u8);
        }
    }

    #[inline]
    pub fn set_flag(&mut self, flag: CpuFlag) {
        self.regs[F as usize] |= flag as u8;
    }

    #[inline]
    pub fn reset_flag(&mut self, flag: CpuFlag) {
        self.regs[F as usize] &= !(flag as u8);
    }

    #[inline]
    pub fn test_flag(&mut self, flag: CpuFlag) -> bool {
        (self.regs[F as usize] & (flag as u8)) != 0
    }
}

impl Debug for Registers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        static FLAGS: [CpuFlag; 4] = [CpuFlag::Zero, CpuFlag::Sub, CpuFlag::HalfCarry, CpuFlag::Carry];
        write!(f, "AF = {:#04x}, BC = {:#04x}, DE = {:#04x}, HL = {:#04x}. Flags set: ",
               self.get_reg16(AF), self.get_reg16(BC),
               self.get_reg16(DE), self.get_reg16(HL))?;
        for flag in FLAGS {
            write!(f, "{:?} ", flag)?;
        }
        Ok(())
    }
}

pub(crate) static INTERRUPT_VECTORS: [(Interrupt, u16); 5] = [
    (Interrupt::JPAD, 0x40),
    (Interrupt::SERIAL, 0x48),
    (Interrupt::TIMER, 0x50),
    (Interrupt::LCD, 0x58),
    (Interrupt::VBLANK, 0x60)
];

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum CpuState {
    Fetching { halt_bug: bool },
    Decoding(u8),
    Halting,
    Halted,
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
    double_speed: bool,

    cycles: u64,
    cgb: bool,
}

impl Cpu {
    pub fn new(interrupt_controller: Rc<RefCell<InterruptController>>, bus: BusController, cgb: bool) -> Self {
        trace!("Building CPU");
        Self {
            interrupt_controller,
            bus,
            state: CpuState::Fetching { halt_bug: false },
            registers: Registers::new(cgb),
            pc: 0x0100,
            sp: 0xFFFE,
            instruction: INSTRUCTIONS[0],
            instruction_arg: InstArg::None,
            interrupts_to_handle: VecDeque::new(),
            double_speed: false,
            cycles: 0,
            cgb
        }
    }

    pub fn reset(&mut self) {
        debug!("Resetting CPU");
        self.sp = 0xFFFE;
        self.pc = 0x0100;
        self.state = CpuState::Fetching { halt_bug: false };

        self.double_speed = false;
        self.cycles = 0;

        (*self.interrupt_controller).borrow_mut().reset();
    }

    fn fetch(&mut self, halt_bug: bool) {
        let opcode = self.bus.read(self.pc);
        if !halt_bug { self.pc += 1; }
        self.state = CpuState::Decoding(opcode);
    }

    fn decode(&mut self, opcode: u8) {
        self.instruction = INSTRUCTIONS[opcode as usize];
        trace!("Executing instruction {:02X}", opcode);
        self.state = CpuState::StartedExecution;
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

    fn interrupt_service_routine(&mut self) {
        self.state =
            match self.state {
                CpuState::ServicingInterrupts => self.check_interrupts(),
                CpuState::InterruptWaitState1 => CpuState::InterruptWaitState2,
                CpuState::InterruptWaitState2 => {
                    self.sp = self.sp.wrapping_sub(1);
                    CpuState::InterruptPushPCHi
                },
                CpuState::InterruptPushPCHi => {
                    self.bus.write(self.sp, (self.pc >> 8) as u8);
                    self.sp = self.sp.wrapping_sub(1);
                    CpuState::InterruptPushPCLo
                },
                CpuState::InterruptPushPCLo => {
                    self.bus.write(self.sp, (self.pc & 0xFF) as u8);
                    CpuState::InterruptUpdatePC
                },
                CpuState::InterruptUpdatePC => {
                    self.pc = self.interrupts_to_handle.pop_front().unwrap();
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
        use CpuState::*;
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
            FinishedExecution => self.state = ServicingInterrupts,
            ServicingInterrupts
            | InterruptWaitState1 | InterruptWaitState2
            | InterruptPushPCHi | InterruptPushPCLo
            | InterruptUpdatePC => self.interrupt_service_routine(),
            _ => (self.instruction)(self)
        }
        debug_assert!(old_state != self.state || self.state == Halted);
        // trace!("{:?} => {:?}", old_state, self.state);
    }

    pub fn clock(&mut self) {
        loop {
            self.advance();
            if !self.state.is_intermediate() { break }
        }
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
                    let mut data = self.bus.read(addr).wrapping_sub(1);
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
                trace!("Executing CB instruction {:02X}", arg);
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
                CpuState::StartedExecution => CpuState::BranchDecision,
                CpuState::BranchDecision => {
                    if condition {
                        CpuState::ReadMemoryLo(self.sp)
                    } else {
                        CpuState::FinishedExecution
                    }
                },
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
        self.ret(condition);
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
                    self.sp -= 1;
                    CpuState::PushHi
                },
                CpuState::PushHi => {
                    self.bus.write(self.sp, self.registers.get_reg8(hi));
                    self.sp -= 1;
                    CpuState::PushLo
                },
                CpuState::PushLo => {
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
                    self.sp -= 1;
                    CpuState::PushHi
                }
                CpuState::PushHi => {
                    self.bus.write(self.sp, (self.pc >> 8) as u8);
                    self.sp -= 1;
                    CpuState::PushLo
                },
                CpuState::PushLo => {
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
                    self.sp -= 1;
                    CpuState::PushHi
                },
                CpuState::PushHi => {
                    self.bus.write(self.sp, (self.pc >> 8) as u8);
                    self.sp -= 1;
                    CpuState::PushLo
                },
                CpuState::PushLo => {
                    self.bus.write(self.sp, (self.pc & 0xFF) as u8);
                    self.pc = self.instruction_arg.build_word();

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