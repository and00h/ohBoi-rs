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

fn op_indirect<F>(cpu: &mut Cpu, mut op: F)
    where
        F: FnMut(&mut Cpu, u8)
{
    let addr = cpu.registers.get_reg16(HL);
    let data = cpu.read_memory(addr);
    op(cpu, data);
}

fn op_reg<F>(cpu: &mut Cpu, mut op: F, reg: Register8)
    where
        F: FnMut(&mut Cpu, u8)
{
    op(cpu, cpu.registers.get_reg8(reg));
}

fn unknown(_cpu: &mut Cpu) {
    panic!("Unknown opcode!");
}

pub(crate) enum InstArg {
    None,
    Byte(u8),
    Word(u16)
}
enum InstState {
    Fetching,
    ReadArg,
    ReadArgLo,
    ReadArgHi,
    ALU16ReadHi,
    ALU16WriteHi,
    UpdatePC,
    ReadMemory,
    ReadMemoryLo,
    ReadMemoryHi,
    WriteMemory,
    WriteMemoryLo,
    WriteMemoryHi,
    Executing(InstArg, usize),
    Executed
}

pub struct Inst<F: FnMut(&mut Cpu)> {
    inst_type: InstructionType,
    latency: usize,
    arg: Option<InstArg>,
    transition_func: Box<F>
}

impl<F: FnMut(&mut Cpu)> Inst<F> {
    pub fn new(inst_type: InstructionType, transition_func: F, latency: usize) -> Self {
        Self::with_initial_state(inst_type, transition_func, latency, InstState::Fetching)
    }

    pub fn with_initial_state(inst_type: InstructionType, transition_func: F, latency: usize, state: InstState) -> Self {
        Inst {
            inst_type,
            transition_func: Box::new(transition_func),
            latency,
            arg: None
        }
    }

    pub fn advance(&mut self, cpu: &mut Cpu) {
        (*self.transition_func)(cpu);
    }

    pub fn executes_immediately(&self) -> bool {
        self.latency == 0
    }
}

fn nop(cpu: &mut Cpu) -> CpuState {
    CpuState::FinishedExecution
}

pub(super) static INSTRUCTIONS: [fn(&mut Cpu); 0x100] = [
    nop,
    |cpu| cpu.load_word_imm(Register16::BC),
    |cpu| cpu.store_indirect(Register16::BC),
    |cpu| cpu.inc16(Register16::BC),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(Register8::B)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(Register8::B)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.load_immediate(Register8::B, arg)),
    |cpu| cpu.zero_latency(Cpu::rlca),
    Cpu::store_sp,
    |cpu| cpu.add_hl(Register16::BC),
    |cpu| cpu.load_indirect(Register8::A, Register16::BC),
    |cpu| cpu.dec16(Register16::BC),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(Register8::C)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(Register8::C)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.load_immediate(Register8::C, arg)),
    |cpu| cpu.zero_latency(Cpu::rrca),
    |cpu| cpu.stop(),
    |cpu| cpu.load_word_imm(Register16::DE),
    |cpu| cpu.store_indirect(Register16::DE),
    |cpu| cpu.inc16(Register16::DE),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(Register8::D)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(Register8::D)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.load_immediate(Register8::D, arg)),
    |cpu| cpu.zero_latency(Cpu::rla),
    |cpu| cpu.jump_rel_conditional(true),
    |cpu| cpu.add_hl(Register16::DE),
    |cpu| cpu.load_indirect(Register8::A, Register16::DE),
    |cpu| cpu.dec16(Register16::DE),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(Register8::E)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(Register8::E)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.load_immediate(Register8::E, arg)),
    |cpu| cpu.zero_latency(Cpu::rra),
    |cpu| cpu.jump_rel_conditional(!cpu.registers.test_flag(CpuFlag::Zero)),
    |cpu| cpu.load_word_imm(Register16::HL),
    Cpu::store_hl_inc,
    |cpu| cpu.inc16(Register16::HL),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(Register8::H)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(Register8::H)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.load_immediate(Register8::H, arg)),
    |cpu| cpu.zero_latency(Cpu::daa),
    |cpu| cpu.jump_rel_conditional(cpu.registers.test_flag(CpuFlag::Zero)),
    |cpu| cpu.add_hl(Register16::HL),
    Cpu::load_hl_inc,
    |cpu| cpu.dec16(Register16::HL),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(Register8::L)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(Register8::L)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.load_immediate(Register8::L, arg)),
    |cpu| cpu.zero_latency(Cpu::cpl),
    |cpu| cpu.jump_rel_conditional(!cpu.registers.test_flag(CpuFlag::Carry)),
    Cpu::load_word_sp,
    Cpu::store_hl_dec,
    Cpu::inc_sp,
    Cpu::inc_hl_indirect,
    Cpu::dec_hl_indirect
    |cpu| cpu.one_arg(|cpu, arg| cpu.load_immediate(Register8::H, arg)),
    |cpu| cpu.zero_latency(Cpu::daa),
    |cpu| cpu.jump_rel_conditional(cpu.registers.test_flag(CpuFlag::Zero)),
    |cpu| cpu.add_hl(Register16::HL),
    Cpu::load_hl_inc,
    |cpu| cpu.dec16(Register16::HL),
    |cpu| cpu.zero_latency(|cpu| cpu.inc8(Register8::L)),
    |cpu| cpu.zero_latency(|cpu| cpu.dec8(Register8::L)),
    |cpu| cpu.one_arg(|cpu, arg| cpu.load_immediate(Register8::L, arg)),
    |cpu| cpu.zero_latency(Cpu::cpl),

];

pub(super) static OPS: [InstructionType; 0x100] = [
    NoArgs(|_| {}),
    TwoArgs(|cpu, data| cpu.load_word(BC, data)),
    NoArgs(|cpu| cpu.store_indirect(A, BC)),
    NoArgs(|cpu| cpu.inc16(BC)),
    NoArgs(|cpu| cpu.inc8(B)),
    NoArgs(|cpu| cpu.dec8(B)),
    OneArg(|cpu, data| cpu.load_immediate(B, data)),
    NoArgs(Cpu::rlca),
    TwoArgs(|cpu, data| cpu.store_word(cpu.sp, data)),
    NoArgs(|cpu| cpu.add_hl(cpu.registers.get_reg16(BC))),
    NoArgs(|cpu| cpu.load_indirect(A, BC)),
    NoArgs(|cpu| cpu.dec16(BC)),
    NoArgs(|cpu| cpu.inc8(C)),
    NoArgs(|cpu| cpu.dec8(C)),
    OneArg(|cpu, data| cpu.registers.set_reg8(C, data)),
    NoArgs(Cpu::rrca),
    NoArgs(Cpu::stop),
    TwoArgs(|cpu, data| cpu.registers.set_reg16(DE, data)),
    NoArgs(|cpu| cpu.store_indirect(A, DE)),
    NoArgs(|cpu| cpu.inc16(DE)),
    NoArgs(|cpu| cpu.inc8(D)),
    NoArgs(|cpu| cpu.dec8(D)),
    OneArg(|cpu, data| cpu.load_immediate(D, data)),
    NoArgs(Cpu::rla),
    OneArg(Cpu::jr),
    NoArgs(|cpu| cpu.add_hl(cpu.registers.get_reg16(DE))),
    NoArgs(|cpu| cpu.load_indirect(A, DE)),
    NoArgs(|cpu| cpu.dec16(DE)),
    NoArgs(|cpu| cpu.inc8(E)),
    NoArgs(|cpu| cpu.dec8(E)),
    OneArg(|cpu, data| cpu.load_immediate(E, data)),
    NoArgs(Cpu::rra),
    OneArg(|cpu, data| cpu.jrc(data, CpuFlag::Zero, true)),
    TwoArgs(|cpu, data| cpu.load_word(HL, data)),
    NoArgs(|cpu| {
        cpu.store_indirect(A, HL);
        cpu.registers.set_reg16(HL, cpu.registers.get_reg16(HL) + 1);
    }),
    NoArgs(|cpu| cpu.inc16(HL)),
    NoArgs(|cpu| cpu.inc8(H)),
    NoArgs(|cpu| cpu.dec8(H)),
    OneArg(|cpu, data| cpu.load_immediate(H, data)),
    NoArgs(Cpu::daa),
    OneArg(|cpu, data| cpu.jrc(data, CpuFlag::Zero, false)),
    NoArgs(|cpu| cpu.add_hl(cpu.registers.get_reg16(HL))),
    NoArgs(|cpu| {
        cpu.load_indirect(A, HL);
        cpu.registers.set_reg16(HL, cpu.registers.get_reg16(HL) + 1);
    }),
    NoArgs(|cpu| cpu.dec16(HL)),
    NoArgs(|cpu| cpu.inc8(L)),
    NoArgs(|cpu| cpu.dec8(L)),
    OneArg(|cpu, data| cpu.load_immediate(L, data)),
    NoArgs(Cpu::cpl),
    OneArg(|cpu, data| cpu.jrc(data, CpuFlag::Carry, true)),
    TwoArgs(|cpu, data| {
        cpu.sp = data;
    }),
    NoArgs(|cpu| {
        cpu.store_indirect(A, HL);
        cpu.registers.set_reg16(HL, cpu.registers.get_reg16(HL) - 1);
    }),
    NoArgs(Cpu::inc_sp),
    NoArgs(Cpu::inc_hl),
    NoArgs(Cpu::dec_hl),
    OneArg(|cpu, data| cpu.write_memory(cpu.registers.get_reg16(HL), data)),
    NoArgs(Cpu::scf),
    OneArg(|cpu, data| cpu.jrc(data, CpuFlag::Carry, false)),
    NoArgs(|cpu| cpu.add_hl(cpu.sp)),
    NoArgs(|cpu| {
        let addr = cpu.registers.get_reg16(HL);
        let data = cpu.read_memory(addr);
        cpu.registers.set_reg8(A, data);
        cpu.registers.set_reg16(HL, addr - 1);
    }),
    NoArgs(Cpu::dec_sp),
    NoArgs(|cpu| cpu.inc8(A)),
    NoArgs(|cpu| cpu.dec8(A)),
    OneArg(|cpu, data| cpu.load_immediate(A, data)),
    NoArgs(Cpu::ccf),
    NoArgs(|cpu| cpu.registers.load(B, B)),
    NoArgs(|cpu| cpu.registers.load(B, C)),
    NoArgs(|cpu| cpu.registers.load(B, D)),
    NoArgs(|cpu| cpu.registers.load(B, E)),
    NoArgs(|cpu| cpu.registers.load(B, H)),
    NoArgs(|cpu| cpu.registers.load(B, L)),
    NoArgs(|cpu| cpu.load_indirect(B, HL)),
    NoArgs(|cpu| cpu.registers.load(B, A)),
    NoArgs(|cpu| cpu.registers.load(C, B)),
    NoArgs(|cpu| cpu.registers.load(C, C)),
    NoArgs(|cpu| cpu.registers.load(C, D)),
    NoArgs(|cpu| cpu.registers.load(C, E)),
    NoArgs(|cpu| cpu.registers.load(C, H)),
    NoArgs(|cpu| cpu.registers.load(C, L)),
    NoArgs(|cpu| cpu.load_indirect(C, HL)),
    NoArgs(|cpu| cpu.registers.load(C, A)),
    NoArgs(|cpu| cpu.registers.load(B, B)),
    NoArgs(|cpu| cpu.registers.load(D, C)),
    NoArgs(|cpu| cpu.registers.load(D, D)),
    NoArgs(|cpu| cpu.registers.load(D, E)),
    NoArgs(|cpu| cpu.registers.load(D, H)),
    NoArgs(|cpu| cpu.registers.load(D, L)),
    NoArgs(|cpu| cpu.load_indirect(D, HL)),
    NoArgs(|cpu| cpu.registers.load(D, A)),
    NoArgs(|cpu| cpu.registers.load(E, B)),
    NoArgs(|cpu| cpu.registers.load(E, C)),
    NoArgs(|cpu| cpu.registers.load(E, D)),
    NoArgs(|cpu| cpu.registers.load(E, E)),
    NoArgs(|cpu| cpu.registers.load(E, H)),
    NoArgs(|cpu| cpu.registers.load(E, L)),
    NoArgs(|cpu| cpu.load_indirect(E, HL)),
    NoArgs(|cpu| cpu.registers.load(E, A)),
    NoArgs(|cpu| cpu.registers.load(H, B)),
    NoArgs(|cpu| cpu.registers.load(H, C)),
    NoArgs(|cpu| cpu.registers.load(H, D)),
    NoArgs(|cpu| cpu.registers.load(H, E)),
    NoArgs(|cpu| cpu.registers.load(H, H)),
    NoArgs(|cpu| cpu.registers.load(H, L)),
    NoArgs(|cpu| cpu.load_indirect(H, HL)),
    NoArgs(|cpu| cpu.registers.load(H, A)),
    NoArgs(|cpu| cpu.registers.load(L, B)),
    NoArgs(|cpu| cpu.registers.load(L, C)),
    NoArgs(|cpu| cpu.registers.load(L, D)),
    NoArgs(|cpu| cpu.registers.load(L, E)),
    NoArgs(|cpu| cpu.registers.load(L, H)),
    NoArgs(|cpu| cpu.registers.load(L, L)),
    NoArgs(|cpu| cpu.load_indirect(L, HL)),
    NoArgs(|cpu| cpu.registers.load(L, A)),
    NoArgs(|cpu| cpu.store_indirect(B, HL)),
    NoArgs(|cpu| cpu.store_indirect(C, HL)),
    NoArgs(|cpu| cpu.store_indirect(D, HL)),
    NoArgs(|cpu| cpu.store_indirect(E, HL)),
    NoArgs(|cpu| cpu.store_indirect(H, HL)),
    NoArgs(|cpu| cpu.store_indirect(L, HL)),
    NoArgs(Cpu::halt),
    NoArgs(|cpu| cpu.store_indirect(A, HL)),
    NoArgs(|cpu| cpu.registers.load(A, B)),
    NoArgs(|cpu| cpu.registers.load(A, C)),
    NoArgs(|cpu| cpu.registers.load(A, D)),
    NoArgs(|cpu| cpu.registers.load(A, E)),
    NoArgs(|cpu| cpu.registers.load(A, H)),
    NoArgs(|cpu| cpu.registers.load(A, L)),
    NoArgs(|cpu| cpu.load_indirect(A, HL)),
    NoArgs(|cpu| cpu.registers.load(A, A)),
    NoArgs(|cpu| op_reg(cpu, Cpu::add, B)),
    NoArgs(|cpu| op_reg(cpu, Cpu::add, C)),
    NoArgs(|cpu| op_reg(cpu, Cpu::add, D)),
    NoArgs(|cpu| op_reg(cpu, Cpu::add, E)),
    NoArgs(|cpu| op_reg(cpu, Cpu::add, H)),
    NoArgs(|cpu| op_reg(cpu, Cpu::add, L)),
    NoArgs(|cpu| op_indirect(cpu, Cpu::add)),
    NoArgs(|cpu| op_reg(cpu, Cpu::add, A)),
    NoArgs(|cpu| op_reg(cpu, Cpu::adc, B)),
    NoArgs(|cpu| op_reg(cpu, Cpu::adc, C)),
    NoArgs(|cpu| op_reg(cpu, Cpu::adc, D)),
    NoArgs(|cpu| op_reg(cpu, Cpu::adc, E)),
    NoArgs(|cpu| op_reg(cpu, Cpu::adc, H)),
    NoArgs(|cpu| op_reg(cpu, Cpu::adc, L)),
    NoArgs(|cpu| op_indirect(cpu, Cpu::adc)),
    NoArgs(|cpu| op_reg(cpu, Cpu::adc, A)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sub, B)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sub, C)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sub, D)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sub, E)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sub, H)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sub, L)),
    NoArgs(|cpu| op_indirect(cpu, Cpu::sub)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sub, A)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sbc, B)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sbc, C)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sbc, D)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sbc, E)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sbc, H)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sbc, L)),
    NoArgs(|cpu| op_indirect(cpu, Cpu::sbc)),
    NoArgs(|cpu| op_reg(cpu, Cpu::sbc, A)),
    NoArgs(|cpu| op_reg(cpu, Cpu::and, B)),
    NoArgs(|cpu| op_reg(cpu, Cpu::and, C)),
    NoArgs(|cpu| op_reg(cpu, Cpu::and, D)),
    NoArgs(|cpu| op_reg(cpu, Cpu::and, E)),
    NoArgs(|cpu| op_reg(cpu, Cpu::and, H)),
    NoArgs(|cpu| op_reg(cpu, Cpu::and, L)),
    NoArgs(|cpu| op_indirect(cpu, Cpu::and)),
    NoArgs(|cpu| op_reg(cpu, Cpu::and, A)),
    NoArgs(|cpu| op_reg(cpu, Cpu::xor, B)),
    NoArgs(|cpu| op_reg(cpu, Cpu::xor, C)),
    NoArgs(|cpu| op_reg(cpu, Cpu::xor, D)),
    NoArgs(|cpu| op_reg(cpu, Cpu::xor, E)),
    NoArgs(|cpu| op_reg(cpu, Cpu::xor, H)),
    NoArgs(|cpu| op_reg(cpu, Cpu::xor, L)),
    NoArgs(|cpu| op_indirect(cpu, Cpu::xor)),
    NoArgs(|cpu| op_reg(cpu, Cpu::xor, A)),
    NoArgs(|cpu| op_reg(cpu, Cpu::or, B)),
    NoArgs(|cpu| op_reg(cpu, Cpu::or, C)),
    NoArgs(|cpu| op_reg(cpu, Cpu::or, D)),
    NoArgs(|cpu| op_reg(cpu, Cpu::or, E)),
    NoArgs(|cpu| op_reg(cpu, Cpu::or, H)),
    NoArgs(|cpu| op_reg(cpu, Cpu::or, L)),
    NoArgs(|cpu| op_indirect(cpu, Cpu::or)),
    NoArgs(|cpu| op_reg(cpu, Cpu::or, A)),
    NoArgs(|cpu| op_reg(cpu, Cpu::cp, B)),
    NoArgs(|cpu| op_reg(cpu, Cpu::cp, C)),
    NoArgs(|cpu| op_reg(cpu, Cpu::cp, D)),
    NoArgs(|cpu| op_reg(cpu, Cpu::cp, E)),
    NoArgs(|cpu| op_reg(cpu, Cpu::cp, H)),
    NoArgs(|cpu| op_reg(cpu, Cpu::cp, L)),
    NoArgs(|cpu| op_indirect(cpu, Cpu::cp)),
    NoArgs(|cpu| op_reg(cpu, Cpu::cp, A)),
    NoArgs(|cpu| cpu.retc(CpuFlag::Zero, true)),
    NoArgs(|cpu| cpu.poop(BC)),
    TwoArgs(|cpu, data| cpu.jpc(data, CpuFlag::Zero, true)),
    TwoArgs(Cpu::jp),
    TwoArgs(|cpu, data| cpu.callc(data, CpuFlag::Zero, true)),
    NoArgs(|cpu| cpu.poosh(BC)),
    OneArg(Cpu::add),
    NoArgs(|cpu| cpu.call(0x0000)),
    NoArgs(|cpu| cpu.retc(CpuFlag::Zero, false)),
    NoArgs(Cpu::ret),
    TwoArgs(|cpu, data| cpu.jpc(data, CpuFlag::Zero, false)),
    OneArg(Cpu::cb),
    TwoArgs(|cpu, data| cpu.callc(data, CpuFlag::Zero, false)),
    TwoArgs(Cpu::call),
    OneArg(Cpu::adc),
    NoArgs(|cpu| cpu.call(0x0008)),
    NoArgs(|cpu| cpu.retc(CpuFlag::Carry, true)),
    NoArgs(|cpu| cpu.poop(DE)),
    TwoArgs(|cpu, data| cpu.jpc(data, CpuFlag::Carry, false)),
    NoArgs(unknown),
    TwoArgs(|cpu, data| cpu.callc(data, CpuFlag::Carry, true)),
    NoArgs(|cpu| cpu.poosh(DE)),
    OneArg(Cpu::sub),
    NoArgs(|cpu| cpu.call(0x0010)),
    NoArgs(|cpu| cpu.retc(CpuFlag::Carry, false)),
    NoArgs(Cpu::reti),
    TwoArgs(|cpu, data| cpu.jpc(data, CpuFlag::Carry, false)),
    NoArgs(unknown),
    TwoArgs(|cpu, data| cpu.callc(data, CpuFlag::Carry, false)),
    NoArgs(unknown),
    OneArg(Cpu::sbc),
    NoArgs(|cpu| cpu.call(0x0018)),
    OneArg(Cpu::ldh_out),
    NoArgs(|cpu| cpu.poop(HL)),
    NoArgs(Cpu::ldc_out),
    NoArgs(unknown),
    NoArgs(unknown),
    NoArgs(|cpu| cpu.poosh(HL)),
    OneArg(Cpu::and),
    NoArgs(|cpu| cpu.call(0x0020)),
    OneArg(Cpu::add_sp),
    NoArgs(|cpu| cpu.jp(cpu.registers.get_reg16(HL))),
    TwoArgs(|cpu, data| cpu.store(A, data)),
    NoArgs(unknown),
    NoArgs(unknown),
    NoArgs(unknown),
    OneArg(Cpu::xor),
    NoArgs(|cpu| cpu.call(0x0028)),
    OneArg(Cpu::ldh_in),
    NoArgs(|cpu| cpu.poop(AF)),
    NoArgs(Cpu::ldc_in),
    NoArgs(Cpu::di),
    NoArgs(unknown),
    NoArgs(|cpu| cpu.poosh(AF)),
    OneArg(Cpu::or),
    NoArgs(|cpu| cpu.call(0x0030)),
    OneArg(|cpu, data| cpu.ldhl(data as i8)),
    NoArgs(Cpu::ldsphl),
    TwoArgs(|cpu, data| cpu.load(A, data)),
    NoArgs(Cpu::ei),
    NoArgs(unknown),
    NoArgs(unknown),
    OneArg(Cpu::cp),
    NoArgs(|cpu| cpu.call(0x0038)),
];
