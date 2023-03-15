use crate::core::cpu::{CpuFlag, Register16, Register8};
use crate::core::cpu::Cpu;
use Register16::*;
use Register8::*;
use CpuFlag::*;

enum Register {
    ByteReg(Register8),
    WordReg(Register16)
}

type PrefixedInstruction = fn(&mut Cpu);

#[inline]
fn op_reg<F>(mut f: F, cpu: &mut Cpu, reg: Register8)
    where
        F: FnMut(&mut Cpu, u8) -> u8
{
    let val = cpu.registers.get_reg8(reg);
    let res = f(cpu, val);
    cpu.registers.set_reg8(reg, res);
}

#[inline]
fn op_hl<F>(mut f: F, cpu: &mut Cpu)
    where
        F: FnMut(&mut Cpu, u8) -> u8
{
    let addr = cpu.registers.get_reg16(HL);
    let val = cpu.read_memory(addr);
    let res = f(cpu, val);
    cpu.write_memory(addr, res);
}

#[inline]
fn bit_op_reg<F>(mut f: F, cpu: &mut Cpu, bit: usize, reg: Register8)
    where
        F: FnMut(usize, u8) -> u8
{
    let val = cpu.registers.get_reg8(reg);
    let res = f(bit, val);
    cpu.registers.set_reg8(reg, res);
}

#[inline]
fn bit_op_hl<F>(mut f: F, cpu: &mut Cpu, bit: usize)
    where
        F: FnMut(usize, u8) -> u8
{
    let addr = cpu.registers.get_reg16(HL);
    let val = cpu.read_memory(addr);
    let res = f(bit, val);
    cpu.write_memory(addr, res);
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

#[inline]
fn res(bit: usize, val: u8) -> u8 {
    val & !(1 << bit)
}

#[inline]
fn set(bit: usize, val: u8) -> u8 {
    val | (1 << bit)
}

fn bit(cpu: &mut Cpu, bit: usize, reg: Register) {
    let val =
        if let ByteReg(r) = reg {
            cpu.registers.get_reg8(r)
        } else {
            let addr = cpu.registers.get_reg16(HL);
            cpu.read_memory(addr)
        };
    cpu.registers.set_flag_cond(Zero, (val & (1 << bit)) == 0);
    cpu.registers.reset_flag(Sub);
    cpu.registers.set_flag(HalfCarry);
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
    |cpu| bit(cpu, 0, ByteReg(B)),
    |cpu| bit(cpu, 0, ByteReg(C)),
    |cpu| bit(cpu, 0, ByteReg(D)),
    |cpu| bit(cpu, 0, ByteReg(E)),
    |cpu| bit(cpu, 0, ByteReg(H)),
    |cpu| bit(cpu, 0, ByteReg(L)),
    |cpu| bit(cpu, 0, WordReg(HL)),
    |cpu| bit(cpu, 0, ByteReg(A)),
    |cpu| bit(cpu, 1, ByteReg(B)),
    |cpu| bit(cpu, 1, ByteReg(C)),
    |cpu| bit(cpu, 1, ByteReg(D)),
    |cpu| bit(cpu, 1, ByteReg(E)),
    |cpu| bit(cpu, 1, ByteReg(H)),
    |cpu| bit(cpu, 1, ByteReg(L)),
    |cpu| bit(cpu, 1, WordReg(HL)),
    |cpu| bit(cpu, 1, ByteReg(A)),
    |cpu| bit(cpu, 2, ByteReg(B)),
    |cpu| bit(cpu, 2, ByteReg(C)),
    |cpu| bit(cpu, 2, ByteReg(D)),
    |cpu| bit(cpu, 2, ByteReg(E)),
    |cpu| bit(cpu, 2, ByteReg(H)),
    |cpu| bit(cpu, 2, ByteReg(L)),
    |cpu| bit(cpu, 2, WordReg(HL)),
    |cpu| bit(cpu, 2, ByteReg(A)),
    |cpu| bit(cpu, 3, ByteReg(B)),
    |cpu| bit(cpu, 3, ByteReg(C)),
    |cpu| bit(cpu, 3, ByteReg(D)),
    |cpu| bit(cpu, 3, ByteReg(E)),
    |cpu| bit(cpu, 3, ByteReg(H)),
    |cpu| bit(cpu, 3, ByteReg(L)),
    |cpu| bit(cpu, 3, WordReg(HL)),
    |cpu| bit(cpu, 3, ByteReg(A)),
    |cpu| bit(cpu, 4, ByteReg(B)),
    |cpu| bit(cpu, 4, ByteReg(C)),
    |cpu| bit(cpu, 4, ByteReg(D)),
    |cpu| bit(cpu, 4, ByteReg(E)),
    |cpu| bit(cpu, 4, ByteReg(H)),
    |cpu| bit(cpu, 4, ByteReg(L)),
    |cpu| bit(cpu, 4, WordReg(HL)),
    |cpu| bit(cpu, 4, ByteReg(A)),
    |cpu| bit(cpu, 5, ByteReg(B)),
    |cpu| bit(cpu, 5, ByteReg(C)),
    |cpu| bit(cpu, 5, ByteReg(D)),
    |cpu| bit(cpu, 5, ByteReg(E)),
    |cpu| bit(cpu, 5, ByteReg(H)),
    |cpu| bit(cpu, 5, ByteReg(L)),
    |cpu| bit(cpu, 5, WordReg(HL)),
    |cpu| bit(cpu, 5, ByteReg(A)),
    |cpu| bit(cpu, 6, ByteReg(B)),
    |cpu| bit(cpu, 6, ByteReg(C)),
    |cpu| bit(cpu, 6, ByteReg(D)),
    |cpu| bit(cpu, 6, ByteReg(E)),
    |cpu| bit(cpu, 6, ByteReg(H)),
    |cpu| bit(cpu, 6, ByteReg(L)),
    |cpu| bit(cpu, 6, WordReg(HL)),
    |cpu| bit(cpu, 6, ByteReg(A)),
    |cpu| bit(cpu, 7, ByteReg(B)),
    |cpu| bit(cpu, 7, ByteReg(C)),
    |cpu| bit(cpu, 7, ByteReg(D)),
    |cpu| bit(cpu, 7, ByteReg(E)),
    |cpu| bit(cpu, 7, ByteReg(H)),
    |cpu| bit(cpu, 7, ByteReg(L)),
    |cpu| bit(cpu, 7, WordReg(HL)),
    |cpu| bit(cpu, 7, ByteReg(A)),
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