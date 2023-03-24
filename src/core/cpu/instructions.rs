use log::{debug, warn};
use crate::core::cpu::{CpuFlag, Register16, Register8};
use crate::core::cpu::Cpu;
use Register16::*;
use Register8::*;

type NoArgOp = fn(&mut Cpu);
type OneArgOp = fn(&mut Cpu, u8);
type TwoArgOp = fn(&mut Cpu, u16);

pub(super) enum InstructionType {
    NoArgs(NoArgOp),
    OneArg(OneArgOp),
    TwoArgs(TwoArgOp),
}

use InstructionType::{NoArgs, OneArg, TwoArgs};

use super::CpuState;

fn unknown(_cpu: &mut Cpu) {
    panic!("Unknown opcode!");
}

pub(crate) enum InstArg {
    None,
    Byte(u8),
    Word(u8, u8)
}

impl InstArg {
    pub fn build_word(&self) -> u16 {
        match self {
            Self::Word(hi, lo) => ((*hi as u16) << 8) | (*lo as u16),
            Self::Byte(val) => *val as u16,
            Self::None => panic!("InstArg::build_word() called on a None value!")
        }
    }

    pub fn lo(&self) -> u8 {
        match self {
            Self::Word(_, lo) => *lo,
            Self::Byte(val) => {
                debug!("InstArg::lo called on a Byte value");
                *val
            },
            Self::None => panic!("InstArg::lo() called on a None value!")
        }
    }

    pub fn hi(&self) -> u8 {
        match self {
            Self::Word(hi, _) => *hi,
            Self::Byte(val) => {
                debug!("InstArg::hi called on a Byte value");
                *val
            },
            Self::None => panic!("InstArg::lo() called on a None value!")
        }
    }
}

struct Inst<F: FnMut(&mut Cpu)> {
    inst_type: InstructionType,
    transition_func: Box<F>
}

impl<F: FnMut(&mut Cpu)> Inst<F> {
    pub fn new(inst_type: InstructionType, transition_func: F) -> Self {
        Inst {
            inst_type,
            transition_func: Box::new(transition_func),
        }
    }

    pub fn advance(&mut self, cpu: &mut Cpu) {
        (*self.transition_func)(cpu);
    }
}

fn nop(_cpu: &mut Cpu) { _cpu.state = CpuState::FinishedExecution }

pub(super) static INSTRUCTIONS: [fn(&mut Cpu); 0x100] = [
    nop,
    |cpu| cpu.load_word_imm(BC),
    |cpu| cpu.store_indirect(BC),
    |cpu| cpu.inc16(BC),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(B)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(B)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.registers.set_reg8(B, arg)),
    |cpu| cpu.zero_latency(Cpu::rlca),
    Cpu::store_sp,
    |cpu| cpu.add_hl(BC),
    |cpu| cpu.load_indirect(A, BC),
    |cpu| cpu.dec16(BC),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(C)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(C)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.registers.set_reg8(C, arg)),
    |cpu| cpu.zero_latency(Cpu::rrca),

    |cpu| cpu.zero_latency(Cpu::stop),
    |cpu| cpu.load_word_imm(DE),
    |cpu| cpu.store_indirect(DE),
    |cpu| cpu.inc16(DE),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(D)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(D)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.registers.set_reg8(D, arg)),
    |cpu| cpu.zero_latency(Cpu::rla),
    |cpu| cpu.jump_rel(true),
    |cpu| cpu.add_hl(DE),
    |cpu| cpu.load_indirect(A, DE),
    |cpu| cpu.dec16(DE),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(E)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(E)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.registers.set_reg8(E, arg)),
    |cpu| cpu.zero_latency(Cpu::rra),

    |cpu| cpu.jump_rel_conditional(CpuFlag::Zero, true),
    |cpu| cpu.load_word_imm(HL),
    Cpu::store_hl_inc,
    |cpu| cpu.inc16(HL),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(H)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(H)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.registers.set_reg8(H, arg)),
    |cpu| cpu.zero_latency(Cpu::daa),
    |cpu| cpu.jump_rel_conditional(CpuFlag::Zero, false),
    |cpu| cpu.add_hl(HL),
    Cpu::load_hl_inc,
    |cpu| cpu.dec16(HL),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(L)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(L)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.registers.set_reg8(L, arg)),
    |cpu| cpu.zero_latency(Cpu::cpl),

    |cpu| cpu.jump_rel_conditional(CpuFlag::Carry, true),
    Cpu::load_word_sp,
    Cpu::store_hl_dec,
    Cpu::inc_sp,
    Cpu::inc_hl_indirect,
    Cpu::dec_hl_indirect,
    Cpu::store_hl_imm,
    |cpu| cpu.zero_latency(Cpu::scf),
    |cpu| cpu.jump_rel_conditional(CpuFlag::Carry, false),
    Cpu::add_hl_sp,
    Cpu::load_hl_dec,
    Cpu::dec_sp,
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(A)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(A)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.registers.set_reg8(A, arg)),
    |cpu| cpu.zero_latency(Cpu::ccf),

    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(B, B)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(B, C)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(B, D)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(B, E)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(B, H)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(B, L)),
    |cpu| cpu.hl_src_reg_op(|cpu, val| cpu.registers.set_reg8(B, val)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(B, A)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(C, B)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(C, C)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(C, D)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(C, E)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(C, H)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(C, L)),
    |cpu| cpu.hl_src_reg_op(|cpu, val| cpu.registers.set_reg8(C, val)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(C, A)),

    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(D, B)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(D, C)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(D, D)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(D, E)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(D, H)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(D, L)),
    |cpu| cpu.hl_src_reg_op(|cpu, val| cpu.registers.set_reg8(D, val)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(D, A)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(E, B)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(E, C)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(E, D)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(E, E)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(E, H)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(E, L)),
    |cpu| cpu.hl_src_reg_op(|cpu, val| cpu.registers.set_reg8(E, val)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(E, A)),

    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(H, B)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(H, C)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(H, D)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(H, E)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(H, H)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(H, L)),
    |cpu| cpu.hl_src_reg_op(|cpu, val| cpu.registers.set_reg8(H, val)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(H, A)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(L, B)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(L, C)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(L, D)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(L, E)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(L, H)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(L, L)),
    |cpu| cpu.hl_src_reg_op(|cpu, val| cpu.registers.set_reg8(L, val)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(L, A)),

    |cpu| cpu.hl_dst_reg_op(|cpu| cpu.registers.get_reg8(B)),
    |cpu| cpu.hl_dst_reg_op(|cpu| cpu.registers.get_reg8(C)),
    |cpu| cpu.hl_dst_reg_op(|cpu| cpu.registers.get_reg8(D)),
    |cpu| cpu.hl_dst_reg_op(|cpu| cpu.registers.get_reg8(E)),
    |cpu| cpu.hl_dst_reg_op(|cpu| cpu.registers.get_reg8(H)),
    |cpu| cpu.hl_dst_reg_op(|cpu| cpu.registers.get_reg8(L)),
    Cpu::halt,
    |cpu| cpu.hl_dst_reg_op(|cpu| cpu.registers.get_reg8(A)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(A, B)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(A, C)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(A, D)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(A, E)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(A, H)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(A, L)),
    |cpu| cpu.hl_src_reg_op(|cpu, val| cpu.registers.set_reg8(A, val)),
    |cpu| cpu.zero_latency(|cpu| cpu.registers.load(A, A)),

    |cpu| cpu.zero_latency(|cpu| cpu.add(cpu.registers.get_reg8(B))),
    |cpu| cpu.zero_latency(|cpu| cpu.add(cpu.registers.get_reg8(C))),
    |cpu| cpu.zero_latency(|cpu| cpu.add(cpu.registers.get_reg8(D))),
    |cpu| cpu.zero_latency(|cpu| cpu.add(cpu.registers.get_reg8(E))),
    |cpu| cpu.zero_latency(|cpu| cpu.add(cpu.registers.get_reg8(H))),
    |cpu| cpu.zero_latency(|cpu| cpu.add(cpu.registers.get_reg8(L))),
    |cpu| cpu.hl_src_reg_op(Cpu::add),
    |cpu| cpu.zero_latency(|cpu| cpu.add(cpu.registers.get_reg8(A))),
    |cpu| cpu.zero_latency(|cpu| cpu.adc(cpu.registers.get_reg8(B))),
    |cpu| cpu.zero_latency(|cpu| cpu.adc(cpu.registers.get_reg8(C))),
    |cpu| cpu.zero_latency(|cpu| cpu.adc(cpu.registers.get_reg8(D))),
    |cpu| cpu.zero_latency(|cpu| cpu.adc(cpu.registers.get_reg8(E))),
    |cpu| cpu.zero_latency(|cpu| cpu.adc(cpu.registers.get_reg8(H))),
    |cpu| cpu.zero_latency(|cpu| cpu.adc(cpu.registers.get_reg8(L))),
    |cpu| cpu.hl_src_reg_op(Cpu::adc),
    |cpu| cpu.zero_latency(|cpu| cpu.adc(cpu.registers.get_reg8(A))),
    |cpu| cpu.zero_latency(|cpu| cpu.sub(cpu.registers.get_reg8(B))),
    |cpu| cpu.zero_latency(|cpu| cpu.sub(cpu.registers.get_reg8(C))),
    |cpu| cpu.zero_latency(|cpu| cpu.sub(cpu.registers.get_reg8(D))),
    |cpu| cpu.zero_latency(|cpu| cpu.sub(cpu.registers.get_reg8(E))),
    |cpu| cpu.zero_latency(|cpu| cpu.sub(cpu.registers.get_reg8(H))),
    |cpu| cpu.zero_latency(|cpu| cpu.sub(cpu.registers.get_reg8(L))),
    |cpu| cpu.hl_src_reg_op(Cpu::sub),
    |cpu| cpu.zero_latency(|cpu| cpu.sub(cpu.registers.get_reg8(A))),
    |cpu| cpu.zero_latency(|cpu| cpu.sbc(cpu.registers.get_reg8(B))),
    |cpu| cpu.zero_latency(|cpu| cpu.sbc(cpu.registers.get_reg8(C))),
    |cpu| cpu.zero_latency(|cpu| cpu.sbc(cpu.registers.get_reg8(D))),
    |cpu| cpu.zero_latency(|cpu| cpu.sbc(cpu.registers.get_reg8(E))),
    |cpu| cpu.zero_latency(|cpu| cpu.sbc(cpu.registers.get_reg8(H))),
    |cpu| cpu.zero_latency(|cpu| cpu.sbc(cpu.registers.get_reg8(L))),
    |cpu| cpu.hl_src_reg_op(Cpu::sbc),
    |cpu| cpu.zero_latency(|cpu| cpu.sbc(cpu.registers.get_reg8(A))),

    |cpu| cpu.zero_latency(|cpu| cpu.and(cpu.registers.get_reg8(B))),
    |cpu| cpu.zero_latency(|cpu| cpu.and(cpu.registers.get_reg8(C))),
    |cpu| cpu.zero_latency(|cpu| cpu.and(cpu.registers.get_reg8(D))),
    |cpu| cpu.zero_latency(|cpu| cpu.and(cpu.registers.get_reg8(E))),
    |cpu| cpu.zero_latency(|cpu| cpu.and(cpu.registers.get_reg8(H))),
    |cpu| cpu.zero_latency(|cpu| cpu.and(cpu.registers.get_reg8(L))),
    |cpu| cpu.hl_src_reg_op(Cpu::and),
    |cpu| cpu.zero_latency(|cpu| cpu.and(cpu.registers.get_reg8(A))),

    |cpu| cpu.zero_latency(|cpu| cpu.xor(cpu.registers.get_reg8(B))),
    |cpu| cpu.zero_latency(|cpu| cpu.xor(cpu.registers.get_reg8(C))),
    |cpu| cpu.zero_latency(|cpu| cpu.xor(cpu.registers.get_reg8(D))),
    |cpu| cpu.zero_latency(|cpu| cpu.xor(cpu.registers.get_reg8(E))),
    |cpu| cpu.zero_latency(|cpu| cpu.xor(cpu.registers.get_reg8(H))),
    |cpu| cpu.zero_latency(|cpu| cpu.xor(cpu.registers.get_reg8(L))),
    |cpu| cpu.hl_src_reg_op(Cpu::xor),
    |cpu| cpu.zero_latency(|cpu| cpu.xor(cpu.registers.get_reg8(A))),

    |cpu| cpu.zero_latency(|cpu| cpu.or(cpu.registers.get_reg8(B))),
    |cpu| cpu.zero_latency(|cpu| cpu.or(cpu.registers.get_reg8(C))),
    |cpu| cpu.zero_latency(|cpu| cpu.or(cpu.registers.get_reg8(D))),
    |cpu| cpu.zero_latency(|cpu| cpu.or(cpu.registers.get_reg8(E))),
    |cpu| cpu.zero_latency(|cpu| cpu.or(cpu.registers.get_reg8(H))),
    |cpu| cpu.zero_latency(|cpu| cpu.or(cpu.registers.get_reg8(L))),
    |cpu| cpu.hl_src_reg_op(Cpu::or),
    |cpu| cpu.zero_latency(|cpu| cpu.or(cpu.registers.get_reg8(A))),

    |cpu| cpu.zero_latency(|cpu| cpu.cp(cpu.registers.get_reg8(B))),
    |cpu| cpu.zero_latency(|cpu| cpu.cp(cpu.registers.get_reg8(C))),
    |cpu| cpu.zero_latency(|cpu| cpu.cp(cpu.registers.get_reg8(D))),
    |cpu| cpu.zero_latency(|cpu| cpu.cp(cpu.registers.get_reg8(E))),
    |cpu| cpu.zero_latency(|cpu| cpu.cp(cpu.registers.get_reg8(H))),
    |cpu| cpu.zero_latency(|cpu| cpu.cp(cpu.registers.get_reg8(L))),
    |cpu| cpu.hl_src_reg_op(Cpu::cp),
    |cpu| cpu.zero_latency(|cpu| cpu.cp(cpu.registers.get_reg8(A))),

    |cpu| cpu.ret_conditional(CpuFlag::Zero, true),
    |cpu| cpu.pop(BC),
    |cpu| cpu.jump_conditional(CpuFlag::Zero, true),
    |cpu| cpu.jump(true),
    |cpu| cpu.call_conditional(CpuFlag::Zero, true),
    |cpu| cpu.push(BC),
    |cpu| cpu.one_arg(Cpu::add),
    |cpu| cpu.rst(0x0000),
    |cpu| cpu.ret_conditional(CpuFlag::Zero, false),
    |cpu| cpu.ret(true),
    |cpu| cpu.jump_conditional(CpuFlag::Zero, false),
    Cpu::cb, // TODO: 0xCB
    |cpu| cpu.call_conditional(CpuFlag::Zero, false),
    |cpu| cpu.call(true),
    |cpu| cpu.one_arg(Cpu::adc),
    |cpu| cpu.rst(0x0008),

    |cpu| cpu.ret_conditional(CpuFlag::Carry, true),
    |cpu| cpu.pop(DE),
    |cpu| cpu.jump_conditional(CpuFlag::Carry, true),
    unknown,
    |cpu| cpu.call_conditional(CpuFlag::Carry, true),
    |cpu| cpu.push(DE),
    |cpu| cpu.one_arg(Cpu::sub),
    |cpu| cpu.rst(0x0010),
    |cpu| cpu.ret_conditional(CpuFlag::Carry, false),
    Cpu::reti,
    |cpu| cpu.jump_conditional(CpuFlag::Carry, false),
    unknown,
    |cpu| cpu.call_conditional(CpuFlag::Carry, false),
    unknown,
    |cpu| cpu.one_arg(Cpu::sbc),
    |cpu| cpu.rst(0x0018),

    Cpu::store_highmem_immediate,
    |cpu| cpu.pop(HL),
    Cpu::store_highmem_reg,
    unknown,
    unknown,
    |cpu| cpu.push(HL),
    |cpu| cpu.one_arg(Cpu::and),
    |cpu| cpu.rst(0x0020),
    Cpu::add_sp,
    |cpu| cpu.zero_latency(|cpu| cpu.pc = cpu.registers.get_reg16(HL)),
    Cpu::store_accumulator,
    unknown,
    unknown,
    unknown,
    |cpu| cpu.one_arg(Cpu::xor),
    |cpu| cpu.rst(0x0028),

    Cpu::load_highmem_immediate,
    |cpu| cpu.pop(AF),
    Cpu::load_highmem_reg,
    |cpu| cpu.zero_latency(Cpu::di),
    unknown,
    |cpu| cpu.push(AF),
    |cpu| cpu.one_arg(Cpu::or),
    |cpu| cpu.rst(0x0030),
    Cpu::ldhl_sp_offset,
    Cpu::ld_sp_hl,
    Cpu::load_accumulator,
    Cpu::ei,
    unknown,
    unknown,
    |cpu| cpu.one_arg(Cpu::cp),
    |cpu| cpu.rst(0x0038),
];
