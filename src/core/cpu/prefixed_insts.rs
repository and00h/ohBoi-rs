use crate::core::cpu::{CpuFlag, CpuState, Register16, Register8};
use crate::core::cpu::Cpu;
use Register16::*;
use Register8::*;
use CpuFlag::*;

enum Register {
    ByteReg(Register8),
    WordReg(Register16)
}

type PrefixedInstruction = fn(&mut Cpu) -> CpuState;

#[inline]
fn op_reg<F>(mut f: F, cpu: &mut Cpu, reg: Register8) -> CpuState
    where
        F: FnMut(&mut Cpu, u8) -> u8
{
    match cpu.state {
        CpuState::ReadArg => {
            let val = cpu.registers.get_reg8(reg);
            let res = f(cpu, val);
            cpu.registers.set_reg8(reg, res);

            CpuState::FinishedExecution
        },
        _ => unreachable!()
    }
}

#[inline]
fn op_hl<F>(mut f: F, cpu: &mut Cpu) -> CpuState
    where
        F: FnMut(&mut Cpu, u8) -> u8
{
    match cpu.state {
        CpuState::ReadArg => CpuState::ReadMemory(cpu.registers.get_reg16(HL)),
        CpuState::ReadMemory(addr) => {
            let val = cpu.bus.read(addr);
            let res = f(cpu, val);
            CpuState::WriteMemory(addr, res)
        },
        CpuState::WriteMemory(addr, val) => {
            cpu.bus.write(addr, val);
            CpuState::FinishedExecution
        },
        _ => unreachable!()
    }
}

#[inline]
fn bit_op_reg<F>(mut f: F, cpu: &mut Cpu, bit: usize, reg: Register8) -> CpuState
    where
        F: FnMut(usize, u8) -> u8
{
    match cpu.state {
        CpuState::ReadArg => {
            let val = cpu.registers.get_reg8(reg);
            let res = f(bit, val);
            cpu.registers.set_reg8(reg, res);

            CpuState::FinishedExecution
        },
        _ => unreachable!()
    }
}

#[inline]
fn bit_op_hl<F>(mut f: F, cpu: &mut Cpu, bit: usize) -> CpuState
    where
        F: FnMut(usize, u8) -> u8
{
    match cpu.state {
        CpuState::ReadArg => CpuState::ReadMemory(cpu.registers.get_reg16(HL)),
        CpuState::ReadMemory(addr) => {
            let val = cpu.bus.read(addr);
            let res = f(bit, val);
            CpuState::WriteMemory(addr, res)
        },
        CpuState::WriteMemory(addr, val) => {
            cpu.bus.write(addr, val);
            CpuState::FinishedExecution
        },
        _ => unreachable!()
    }
}

fn rlc(cpu: &mut Cpu, val: u8) -> u8 {
    let carry = val >> 7;
    cpu.registers.set_flag_cond(Carry, carry == 1);

    let res = (val << 1) | carry;
    cpu.registers.set_flag_cond(Zero, res == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.reset_flag(HalfCarry);

    res
}

fn rrc(cpu: &mut Cpu, val: u8) -> u8 {
    let carry = val & 1;
    let res = (val >> 1) | (carry << 7);

    cpu.registers.set_flag_cond(Carry, carry == 1);
    cpu.registers.set_flag_cond(Zero, res == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.reset_flag(HalfCarry);

    res
}

fn rl(cpu: &mut Cpu, val: u8) -> u8 {
    let carry = if cpu.registers.test_flag(Carry) { 1 } else { 0 };
    cpu.registers.set_flag_cond(Carry, (val & 0x80) == 0x80);
    let res = (val << 1) | carry;

    cpu.registers.set_flag_cond(Zero, res == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.reset_flag(HalfCarry);

    res
}

fn rr(cpu: &mut Cpu, val: u8) -> u8 {
    let carry = if cpu.registers.test_flag(Carry) { 0x80 } else { 0 };
    cpu.registers.set_flag_cond(Carry, (val & 1) == 1);

    let res = (val >> 1) | carry;
    cpu.registers.set_flag_cond(Zero, res == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.reset_flag(HalfCarry);

    res
}

fn sla(cpu: &mut Cpu, val: u8) -> u8 {
    cpu.registers.set_flag_cond(Carry, (val & 0x80) == 0x80);
    let res = val << 1;
    cpu.registers.set_flag_cond(Zero, res == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.reset_flag(HalfCarry);

    res
}

fn sra(cpu: &mut Cpu, val: u8) -> u8 {
    cpu.registers.set_flag_cond(Carry, (val & 1) == 1);
    let res = (val & 0x80) | (val >> 1);

    cpu.registers.set_flag_cond(Zero, res == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.reset_flag(HalfCarry);

    res
}

fn swap(cpu: &mut Cpu, val: u8) -> u8 {
    let res = ((val & 0xF) << 4) | ((val & 0xF0) >> 4);
    cpu.registers.set_flag_cond(Zero, res == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.reset_flag(HalfCarry);
    cpu.registers.reset_flag(Carry);

    res
}

fn srl(cpu: &mut Cpu, val: u8) -> u8 {
    cpu.registers.set_flag_cond(Carry, (val & 1) == 1);
    let res = val >> 1;

    cpu.registers.set_flag_cond(Zero, res == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.reset_flag(HalfCarry);

    res
}

fn res(bit: usize, val: u8) -> u8 {
    val & !(1 << bit)
}

fn set(bit: usize, val: u8) -> u8 {
    let res = val | (1 << bit);
    res
}

fn bit(cpu: &mut Cpu, bit: usize, val: u8) {
    cpu.registers.set_flag_cond(Zero, (val & (1 << bit)) == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.set_flag(HalfCarry);
}

fn bit_hl(cpu: &mut Cpu, n: usize) -> CpuState {
        match cpu.state {
            CpuState::ReadArg => CpuState::ReadMemory(cpu.registers.get_reg16(HL)),
            CpuState::ReadMemory(addr) => {
                let val = cpu.bus.read(addr);
                bit(cpu, n, val);

                CpuState::FinishedExecution
            },
            _ => unreachable!()
        }
}

fn bit_reg(cpu: &mut Cpu, n: usize, reg: Register8) -> CpuState {
        match cpu.state {
            CpuState::ReadArg => {
                let val = cpu.registers.get_reg8(reg);
                bit(cpu, n, val);

                CpuState::FinishedExecution
            },
            _ => unreachable!()
        }
}

use Register::*;

pub(super) static PREFIXED_INSTS: [PrefixedInstruction; 0x100] = [
    |cpu| op_reg(rlc, cpu, B),
    |cpu| op_reg(rlc, cpu, C),
    |cpu| op_reg(rlc, cpu, D),
    |cpu| op_reg(rlc, cpu, E),
    |cpu| op_reg(rlc, cpu, H),
    |cpu| op_reg(rlc, cpu, L),
    |cpu| op_hl(rlc, cpu),
    |cpu| op_reg(rlc, cpu, A),
    |cpu| op_reg(rrc, cpu, B),
    |cpu| op_reg(rrc, cpu, C),
    |cpu| op_reg(rrc, cpu, D),
    |cpu| op_reg(rrc, cpu, E),
    |cpu| op_reg(rrc, cpu, H),
    |cpu| op_reg(rrc, cpu, L),
    |cpu| op_hl(rrc, cpu),
    |cpu| op_reg(rrc, cpu, A),
    |cpu| op_reg(rl, cpu, B),
    |cpu| op_reg(rl, cpu, C),
    |cpu| op_reg(rl, cpu, D),
    |cpu| op_reg(rl, cpu, E),
    |cpu| op_reg(rl, cpu, H),
    |cpu| op_reg(rl, cpu, L),
    |cpu| op_hl(rl, cpu),
    |cpu| op_reg(rl, cpu, A),
    |cpu| op_reg(rr, cpu, B),
    |cpu| op_reg(rr, cpu, C),
    |cpu| op_reg(rr, cpu, D),
    |cpu| op_reg(rr, cpu, E),
    |cpu| op_reg(rr, cpu, H),
    |cpu| op_reg(rr, cpu, L),
    |cpu| op_hl(rr, cpu),
    |cpu| op_reg(rr, cpu, A),
    |cpu| op_reg(sla, cpu, B),
    |cpu| op_reg(sla, cpu, C),
    |cpu| op_reg(sla, cpu, D),
    |cpu| op_reg(sla, cpu, E),
    |cpu| op_reg(sla, cpu, H),
    |cpu| op_reg(sla, cpu, L),
    |cpu| op_hl(sla, cpu),
    |cpu| op_reg(sla, cpu, A),
    |cpu| op_reg(sra, cpu, B),
    |cpu| op_reg(sra, cpu, C),
    |cpu| op_reg(sra, cpu, D),
    |cpu| op_reg(sra, cpu, E),
    |cpu| op_reg(sra, cpu, H),
    |cpu| op_reg(sra, cpu, L),
    |cpu| op_hl(sra, cpu),
    |cpu| op_reg(sra, cpu, A),
    |cpu| op_reg(swap, cpu, B),
    |cpu| op_reg(swap, cpu, C),
    |cpu| op_reg(swap, cpu, D),
    |cpu| op_reg(swap, cpu, E),
    |cpu| op_reg(swap, cpu, H),
    |cpu| op_reg(swap, cpu, L),
    |cpu| op_hl(swap, cpu),
    |cpu| op_reg(swap, cpu, A),
    |cpu| op_reg(srl, cpu, B),
    |cpu| op_reg(srl, cpu, C),
    |cpu| op_reg(srl, cpu, D),
    |cpu| op_reg(srl, cpu, E),
    |cpu| op_reg(srl, cpu, H),
    |cpu| op_reg(srl, cpu, L),
    |cpu| op_hl(srl, cpu),
    |cpu| op_reg(srl, cpu, A),
    |cpu| bit_reg(cpu, 0, B),
    |cpu| bit_reg(cpu, 0, C),
    |cpu| bit_reg(cpu, 0, D),
    |cpu| bit_reg(cpu, 0, E),
    |cpu| bit_reg(cpu, 0, H),
    |cpu| bit_reg(cpu, 0, L),
    |cpu| bit_hl(cpu, 0),
    |cpu| bit_reg(cpu, 0, A),
    |cpu| bit_reg(cpu, 1, B),
    |cpu| bit_reg(cpu, 1, C),
    |cpu| bit_reg(cpu, 1, D),
    |cpu| bit_reg(cpu, 1, E),
    |cpu| bit_reg(cpu, 1, H),
    |cpu| bit_reg(cpu, 1, L),
    |cpu| bit_hl(cpu, 1),
    |cpu| bit_reg(cpu, 1, A),
    |cpu| bit_reg(cpu, 2, B),
    |cpu| bit_reg(cpu, 2, C),
    |cpu| bit_reg(cpu, 2, D),
    |cpu| bit_reg(cpu, 2, E),
    |cpu| bit_reg(cpu, 2, H),
    |cpu| bit_reg(cpu, 2, L),
    |cpu| bit_hl(cpu, 2),
    |cpu| bit_reg(cpu, 2, A),
    |cpu| bit_reg(cpu, 3, B),
    |cpu| bit_reg(cpu, 3, C),
    |cpu| bit_reg(cpu, 3, D),
    |cpu| bit_reg(cpu, 3, E),
    |cpu| bit_reg(cpu, 3, H),
    |cpu| bit_reg(cpu, 3, L),
    |cpu| bit_hl(cpu, 3),
    |cpu| bit_reg(cpu, 3, A),
    |cpu| bit_reg(cpu, 4, B),
    |cpu| bit_reg(cpu, 4, C),
    |cpu| bit_reg(cpu, 4, D),
    |cpu| bit_reg(cpu, 4, E),
    |cpu| bit_reg(cpu, 4, H),
    |cpu| bit_reg(cpu, 4, L),
    |cpu| bit_hl(cpu, 4),
    |cpu| bit_reg(cpu, 4, A),
    |cpu| bit_reg(cpu, 5, B),
    |cpu| bit_reg(cpu, 5, C),
    |cpu| bit_reg(cpu, 5, D),
    |cpu| bit_reg(cpu, 5, E),
    |cpu| bit_reg(cpu, 5, H),
    |cpu| bit_reg(cpu, 5, L),
    |cpu| bit_hl(cpu, 5),
    |cpu| bit_reg(cpu, 5, A),
    |cpu| bit_reg(cpu, 6, B),
    |cpu| bit_reg(cpu, 6, C),
    |cpu| bit_reg(cpu, 6, D),
    |cpu| bit_reg(cpu, 6, E),
    |cpu| bit_reg(cpu, 6, H),
    |cpu| bit_reg(cpu, 6, L),
    |cpu| bit_hl(cpu, 6),
    |cpu| bit_reg(cpu, 6, A),
    |cpu| bit_reg(cpu, 7, B),
    |cpu| bit_reg(cpu, 7, C),
    |cpu| bit_reg(cpu, 7, D),
    |cpu| bit_reg(cpu, 7, E),
    |cpu| bit_reg(cpu, 7, H),
    |cpu| bit_reg(cpu, 7, L),
    |cpu| bit_hl(cpu, 7),
    |cpu| bit_reg(cpu, 7, A),
    |cpu| bit_op_reg(res, cpu, 0, B),
    |cpu| bit_op_reg(res, cpu, 0, C),
    |cpu| bit_op_reg(res, cpu, 0, D),
    |cpu| bit_op_reg(res, cpu, 0, E),
    |cpu| bit_op_reg(res, cpu, 0, H),
    |cpu| bit_op_reg(res, cpu, 0, L),
    |cpu| bit_op_hl(res, cpu, 0),
    |cpu| bit_op_reg(res, cpu, 0, A),
    |cpu| bit_op_reg(res, cpu, 1, B),
    |cpu| bit_op_reg(res, cpu, 1, C),
    |cpu| bit_op_reg(res, cpu, 1, D),
    |cpu| bit_op_reg(res, cpu, 1, E),
    |cpu| bit_op_reg(res, cpu, 1, H),
    |cpu| bit_op_reg(res, cpu, 1, L),
    |cpu| bit_op_hl(res, cpu, 1),
    |cpu| bit_op_reg(res, cpu, 1, A),
    |cpu| bit_op_reg(res, cpu, 2, B),
    |cpu| bit_op_reg(res, cpu, 2, C),
    |cpu| bit_op_reg(res, cpu, 2, D),
    |cpu| bit_op_reg(res, cpu, 2, E),
    |cpu| bit_op_reg(res, cpu, 2, H),
    |cpu| bit_op_reg(res, cpu, 2, L),
    |cpu| bit_op_hl(res, cpu, 2),
    |cpu| bit_op_reg(res, cpu, 2, A),
    |cpu| bit_op_reg(res, cpu, 3, B),
    |cpu| bit_op_reg(res, cpu, 3, C),
    |cpu| bit_op_reg(res, cpu, 3, D),
    |cpu| bit_op_reg(res, cpu, 3, E),
    |cpu| bit_op_reg(res, cpu, 3, H),
    |cpu| bit_op_reg(res, cpu, 3, L),
    |cpu| bit_op_hl(res, cpu, 3),
    |cpu| bit_op_reg(res, cpu, 3, A),
    |cpu| bit_op_reg(res, cpu, 4, B),
    |cpu| bit_op_reg(res, cpu, 4, C),
    |cpu| bit_op_reg(res, cpu, 4, D),
    |cpu| bit_op_reg(res, cpu, 4, E),
    |cpu| bit_op_reg(res, cpu, 4, H),
    |cpu| bit_op_reg(res, cpu, 4, L),
    |cpu| bit_op_hl(res, cpu, 4),
    |cpu| bit_op_reg(res, cpu, 4, A),
    |cpu| bit_op_reg(res, cpu, 5, B),
    |cpu| bit_op_reg(res, cpu, 5, C),
    |cpu| bit_op_reg(res, cpu, 5, D),
    |cpu| bit_op_reg(res, cpu, 5, E),
    |cpu| bit_op_reg(res, cpu, 5, H),
    |cpu| bit_op_reg(res, cpu, 5, L),
    |cpu| bit_op_hl(res, cpu, 5),
    |cpu| bit_op_reg(res, cpu, 5, A),
    |cpu| bit_op_reg(res, cpu, 6, B),
    |cpu| bit_op_reg(res, cpu, 6, C),
    |cpu| bit_op_reg(res, cpu, 6, D),
    |cpu| bit_op_reg(res, cpu, 6, E),
    |cpu| bit_op_reg(res, cpu, 6, H),
    |cpu| bit_op_reg(res, cpu, 6, L),
    |cpu| bit_op_hl(res, cpu, 6),
    |cpu| bit_op_reg(res, cpu, 6, A),
    |cpu| bit_op_reg(res, cpu, 7, B),
    |cpu| bit_op_reg(res, cpu, 7, C),
    |cpu| bit_op_reg(res, cpu, 7, D),
    |cpu| bit_op_reg(res, cpu, 7, E),
    |cpu| bit_op_reg(res, cpu, 7, H),
    |cpu| bit_op_reg(res, cpu, 7, L),
    |cpu| bit_op_hl(res, cpu, 7),
    |cpu| bit_op_reg(res, cpu, 7, A),
    |cpu| bit_op_reg(set, cpu, 0, B),
    |cpu| bit_op_reg(set, cpu, 0, C),
    |cpu| bit_op_reg(set, cpu, 0, D),
    |cpu| bit_op_reg(set, cpu, 0, E),
    |cpu| bit_op_reg(set, cpu, 0, H),
    |cpu| bit_op_reg(set, cpu, 0, L),
    |cpu| bit_op_hl(set, cpu, 0),
    |cpu| bit_op_reg(set, cpu, 0, A),
    |cpu| bit_op_reg(set, cpu, 1, B),
    |cpu| bit_op_reg(set, cpu, 1, C),
    |cpu| bit_op_reg(set, cpu, 1, D),
    |cpu| bit_op_reg(set, cpu, 1, E),
    |cpu| bit_op_reg(set, cpu, 1, H),
    |cpu| bit_op_reg(set, cpu, 1, L),
    |cpu| bit_op_hl(set, cpu, 1),
    |cpu| bit_op_reg(set, cpu, 1, A),
    |cpu| bit_op_reg(set, cpu, 2, B),
    |cpu| bit_op_reg(set, cpu, 2, C),
    |cpu| bit_op_reg(set, cpu, 2, D),
    |cpu| bit_op_reg(set, cpu, 2, E),
    |cpu| bit_op_reg(set, cpu, 2, H),
    |cpu| bit_op_reg(set, cpu, 2, L),
    |cpu| bit_op_hl(set, cpu, 2),
    |cpu| bit_op_reg(set, cpu, 2, A),
    |cpu| bit_op_reg(set, cpu, 3, B),
    |cpu| bit_op_reg(set, cpu, 3, C),
    |cpu| bit_op_reg(set, cpu, 3, D),
    |cpu| bit_op_reg(set, cpu, 3, E),
    |cpu| bit_op_reg(set, cpu, 3, H),
    |cpu| bit_op_reg(set, cpu, 3, L),
    |cpu| bit_op_hl(set, cpu, 3),
    |cpu| bit_op_reg(set, cpu, 3, A),
    |cpu| bit_op_reg(set, cpu, 4, B),
    |cpu| bit_op_reg(set, cpu, 4, C),
    |cpu| bit_op_reg(set, cpu, 4, D),
    |cpu| bit_op_reg(set, cpu, 4, E),
    |cpu| bit_op_reg(set, cpu, 4, H),
    |cpu| bit_op_reg(set, cpu, 4, L),
    |cpu| bit_op_hl(set, cpu, 4),
    |cpu| bit_op_reg(set, cpu, 4, A),
    |cpu| bit_op_reg(set, cpu, 5, B),
    |cpu| bit_op_reg(set, cpu, 5, C),
    |cpu| bit_op_reg(set, cpu, 5, D),
    |cpu| bit_op_reg(set, cpu, 5, E),
    |cpu| bit_op_reg(set, cpu, 5, H),
    |cpu| bit_op_reg(set, cpu, 5, L),
    |cpu| bit_op_hl(set, cpu, 5),
    |cpu| bit_op_reg(set, cpu, 5, A),
    |cpu| bit_op_reg(set, cpu, 6, B),
    |cpu| bit_op_reg(set, cpu, 6, C),
    |cpu| bit_op_reg(set, cpu, 6, D),
    |cpu| bit_op_reg(set, cpu, 6, E),
    |cpu| bit_op_reg(set, cpu, 6, H),
    |cpu| bit_op_reg(set, cpu, 6, L),
    |cpu| bit_op_hl(set, cpu, 6),
    |cpu| bit_op_reg(set, cpu, 6, A),
    |cpu| bit_op_reg(set, cpu, 7, B),
    |cpu| bit_op_reg(set, cpu, 7, C),
    |cpu| bit_op_reg(set, cpu, 7, D),
    |cpu| bit_op_reg(set, cpu, 7, E),
    |cpu| bit_op_reg(set, cpu, 7, H),
    |cpu| bit_op_reg(set, cpu, 7, L),
    |cpu| bit_op_hl(set, cpu, 7),
    |cpu| bit_op_reg(set, cpu, 7, A)
];