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

pub const PREFIXED_MNEMONICS: [&str; 0x100] = [
    "RLC B", "RLC C", "RLC D", "RLC E", "RLC H", "RLC L", "RLC (HL)", "RLC A",
    "RRC B", "RRC C", "RRC D", "RRC E", "RRC H", "RRC L", "RRC (HL)", "RRC A",
    "RL B", "RL C", "RL D", "RL E", "RL H", "RL L", "RL (HL)", "RL A",
    "RR B", "RR C", "RR D", "RR E", "RR H", "RR L", "RR (HL)", "RR A",
    "SLA B", "SLA C", "SLA D", "SLA E", "SLA H", "SLA L", "SLA (HL)", "SLA A",
    "SRA B", "SRA C", "SRA D", "SRA E", "SRA H", "SRA L", "SRA (HL)", "SRA A",
    "SWAP B", "SWAP C", "SWAP D", "SWAP E", "SWAP H", "SWAP L", "SWAP (HL)", "SWAP A",
    "SRL B", "SRL C", "SRL D", "SRL E", "SRL H", "SRL L", "SRL (HL)", "SRL A",
    "BIT 0,B", "BIT 0,C", "BIT 0,D", "BIT 0,E", "BIT 0,H", "BIT 0,L", "BIT 0,(HL)", "BIT 0,A",
    "BIT 1,B", "BIT 1,C", "BIT 1,D", "BIT 1,E", "BIT 1,H", "BIT 1,L", "BIT 1,(HL)", "BIT 1,A",
    "BIT 2,B", "BIT 2,C", "BIT 2,D", "BIT 2,E", "BIT 2,H", "BIT 2,L", "BIT 2,(HL)", "BIT 2,A",
    "BIT 3,B", "BIT 3,C", "BIT 3,D", "BIT 3,E", "BIT 3,H", "BIT 3,L", "BIT 3,(HL)", "BIT 3,A",
    "BIT 4,B", "BIT 4,C", "BIT 4,D", "BIT 4,E", "BIT 4,H", "BIT 4,L", "BIT 4,(HL)", "BIT 4,A",
    "BIT 5,B", "BIT 5,C", "BIT 5,D", "BIT 5,E", "BIT 5,H", "BIT 5,L", "BIT 5,(HL)", "BIT 5,A",
    "BIT 6,B", "BIT 6,C", "BIT 6,D", "BIT 6,E", "BIT 6,H", "BIT 6,L", "BIT 6,(HL)", "BIT 6,A",
    "BIT 7,B", "BIT 7,C", "BIT 7,D", "BIT 7,E", "BIT 7,H", "BIT 7,L", "BIT 7,(HL)", "BIT 7,A",
    "RES 0,B", "RES 0,C", "RES 0,D", "RES 0,E", "RES 0,H", "RES 0,L", "RES 0,(HL)", "RES 0,A",
    "RES 1,B", "RES 1,C", "RES 1,D", "RES 1,E", "RES 1,H", "RES 1,L", "RES 1,(HL)", "RES 1,A",
    "RES 2,B", "RES 2,C", "RES 2,D", "RES 2,E", "RES 2,H", "RES 2,L", "RES 2,(HL)", "RES 2,A",
    "RES 3,B", "RES 3,C", "RES 3,D", "RES 3,E", "RES 3,H", "RES 3,L", "RES 3,(HL)", "RES 3,A",
    "RES 4,B", "RES 4,C", "RES 4,D", "RES 4,E", "RES 4,H", "RES 4,L", "RES 4,(HL)", "RES 4,A",
    "RES 5,B", "RES 5,C", "RES 5,D", "RES 5,E", "RES 5,H", "RES 5,L", "RES 5,(HL)", "RES 5,A",
    "RES 6,B", "RES 6,C", "RES 6,D", "RES 6,E", "RES 6,H", "RES 6,L", "RES 6,(HL)", "RES 6,A",
    "RES 7,B", "RES 7,C", "RES 7,D", "RES 7,E", "RES 7,H", "RES 7,L", "RES 7,(HL)", "RES 7,A",
    "SET 0,B", "SET 0,C", "SET 0,D", "SET 0,E", "SET 0,H", "SET 0,L", "SET 0,(HL)", "SET 0,A",
    "SET 1,B", "SET 1,C", "SET 1,D", "SET 1,E", "SET 1,H", "SET 1,L", "SET 1,(HL)", "SET 1,A",
    "SET 2,B", "SET 2,C", "SET 2,D", "SET 2,E", "SET 2,H", "SET 2,L", "SET 2,(HL)", "SET 2,A",
    "SET 3,B", "SET 3,C", "SET 3,D", "SET 3,E", "SET 3,H", "SET 3,L", "SET 3,(HL)", "SET 3,A",
    "SET 4,B", "SET 4,C", "SET 4,D", "SET 4,E", "SET 4,H", "SET 4,L", "SET 4,(HL)", "SET 4,A",
    "SET 5,B", "SET 5,C", "SET 5,D", "SET 5,E", "SET 5,H", "SET 5,L", "SET 5,(HL)", "SET 5,A",
    "SET 6,B", "SET 6,C", "SET 6,D", "SET 6,E", "SET 6,H", "SET 6,L", "SET 6,(HL)", "SET 6,A",
    "SET 7,B", "SET 7,C", "SET 7,D", "SET 7,E", "SET 7,H", "SET 7,L", "SET 7,(HL)", "SET 7,A",
];