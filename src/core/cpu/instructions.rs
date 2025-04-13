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

pub static NARGS: [usize; 256] = [
    0,    // 0x00
    2,    // 0x01
    0,    // 0x02
    0,    // 0x03
    0,    // 0x04
    0,    // 0x05
    1,    // 0x06
    0,    // 0x07
    2,    // 0x08
    0,    // 0x09
    0,    // 0x0A
    0,    // 0x0B
    0,    // 0x0C
    0,    // 0x0D
    1,    // 0x0E
    0,    // 0x0F
    0,    // 0x10
    2,    // 0x11
    0,    // 0x12
    0,    // 0x13
    0,    // 0x14
    0,    // 0x15
    1,    // 0x16
    0,    // 0x17
    1,    // 0x18
    0,    // 0x19
    0,    // 0x1A
    0,    // 0x1B
    0,    // 0x1C
    0,    // 0x1D
    1,    // 0x1E
    0,    // 0x1F
    1,    // 0x20
    2,    // 0x21
    0,    // 0x22
    0,    // 0x23
    0,    // 0x24
    0,    // 0x25
    1,    // 0x26
    0,    // 0x27
    1,    // 0x28
    0,    // 0x29
    0,    // 0x2A
    0,    // 0x2B
    0,    // 0x2C
    0,    // 0x2D
    1,    // 0x2E
    0,    // 0x2F
    1,    // 0x30
    2,    // 0x31
    0,    // 0x32
    0,    // 0x33
    0,    // 0x34
    0,    // 0x35
    1,    // 0x36
    0,    // 0x37
    1,    // 0x38
    0,    // 0x39
    0,    // 0x3A
    0,    // 0x3B
    0,    // 0x3C
    0,    // 0x3D
    1,    // 0x3E
    0,    // 0x3F
    0,    // 0x40
    0,    // 0x41
    0,    // 0x42
    0,    // 0x43
    0,    // 0x44
    0,    // 0x45
    0,    // 0x46
    0,    // 0x47
    0,    // 0x48
    0,    // 0x49
    0,    // 0x4A
    0,    // 0x4B
    0,    // 0x4C
    0,    // 0x4D
    0,    // 0x4E
    0,    // 0x4F
    0,    // 0x50
    0,    // 0x51
    0,    // 0x52
    0,    // 0x53
    0,    // 0x54
    0,    // 0x55
    0,    // 0x56
    0,    // 0x57
    0,    // 0x58
    0,    // 0x59
    0,    // 0x5A
    0,    // 0x5B
    0,    // 0x5C
    0,    // 0x5D
    0,    // 0x5E
    0,    // 0x5F
    0,    // 0x60
    0,    // 0x61
    0,    // 0x62
    0,    // 0x63
    0,    // 0x64
    0,    // 0x65
    0,    // 0x66
    0,    // 0x67
    0,    // 0x68
    0,    // 0x69
    0,    // 0x6A
    0,    // 0x6B
    0,    // 0x6C
    0,    // 0x6D
    0,    // 0x6E
    0,    // 0x6F
    0,    // 0x70
    0,    // 0x71
    0,    // 0x72
    0,    // 0x73
    0,    // 0x74
    0,    // 0x75
    0,    // 0x76
    0,    // 0x77
    0,    // 0x78
    0,    // 0x79
    0,    // 0x7A
    0,    // 0x7B
    0,    // 0x7C
    0,    // 0x7D
    0,    // 0x7E
    0,    // 0x7F
    0,    // 0x80
    0,    // 0x81
    0,    // 0x82
    0,    // 0x83
    0,    // 0x84
    0,    // 0x85
    0,    // 0x86
    0,    // 0x87
    0,    // 0x88
    0,    // 0x89
    0,    // 0x8A
    0,    // 0x8B
    0,    // 0x8C
    0,    // 0x8D
    0,    // 0x8E
    0,    // 0x8F
    0,    // 0x90
    0,    // 0x91
    0,    // 0x92
    0,    // 0x93
    0,    // 0x94
    0,    // 0x95
    0,    // 0x96
    0,    // 0x97
    0,    // 0x98
    0,    // 0x99
    0,    // 0x9A
    0,    // 0x9B
    0,    // 0x9C
    0,    // 0x9D
    0,    // 0x9E
    0,    // 0x9F
    0,    // 0xA0
    0,    // 0xA1
    0,    // 0xA2
    0,    // 0xA3
    0,    // 0xA4
    0,    // 0xA5
    0,    // 0xA6
    0,    // 0xA7
    0,    // 0xA8
    0,    // 0xA9
    0,    // 0xAA
    0,    // 0xAB
    0,    // 0xAC
    0,    // 0xAD
    0,    // 0xAE
    0,    // 0xAF
    0,    // 0xB0
    0,    // 0xB1
    0,    // 0xB2
    0,    // 0xB3
    0,    // 0xB4
    0,    // 0xB5
    0,    // 0xB6
    0,    // 0xB7
    0,    // 0xB8
    0,    // 0xB9
    0,    // 0xBA
    0,    // 0xBB
    0,    // 0xBC
    0,    // 0xBD
    0,    // 0xBE
    0,    // 0xBF
    0,    // 0xC0
    0,    // 0xC1
    2,    // 0xC2
    2,    // 0xC3
    2,    // 0xC4
    0,    // 0xC5
    1,    // 0xC6
    0,    // 0xC7
    0,    // 0xC8
    0,    // 0xC9
    2,    // 0xCA
    1,    // 0xCB
    2,    // 0xCC
    2,    // 0xCD
    1,    // 0xCE
    0,    // 0xCF
    0,    // 0xD0
    0,    // 0xD1
    2,    // 0xD2
    0,    // 0xD3
    2,    // 0xD4
    0,    // 0xD5
    1,    // 0xD6
    0,    // 0xD7
    0,    // 0xD8
    0,    // 0xD9
    2,    // 0xDA
    0,    // 0xDB
    2,    // 0xDC
    0,    // 0xDD
    1,    // 0xDE
    0,    // 0xDF
    1,    // 0xE0
    0,    // 0xE1
    0,    // 0xE2
    0,    // 0xE3
    0,    // 0xE4
    0,    // 0xE5
    1,    // 0xE6
    0,    // 0xE7
    1,    // 0xE8
    0,    // 0xE9
    2,    // 0xEA
    0,    // 0xEB
    0,    // 0xEC
    0,    // 0xED
    1,    // 0xEE
    0,    // 0xEF
    1,    // 0xF0
    0,    // 0xF1
    0,    // 0xF2
    0,    // 0xF3
    0,    // 0xF4
    0,    // 0xF5
    1,    // 0xF6
    0,    // 0xF7
    1,    // 0xF8
    0,    // 0xF9
    2,    // 0xFA
    0,    // 0xFB
    0,    // 0xFC
    0,    // 0xFD
    1,    // 0xFE
    0    // 0xFF
];

pub const MNEMONICS: &'static [&'static str; 256] = &[
    "NOP",                    // 0x00
    "LD BC, {arg16}",        // 0x01
    "LD (BC), A",            // 0x02
    "INC BC",                // 0x03
    "INC B",                // 0x04
    "DEC B",                // 0x05
    "LD B, {arg8}",            // 0x06
    "RLCA",                    // 0x07
    "LD ({arg16}), SP",        // 0x08
    "ADD HL, BC",            // 0x09
    "LD A, (BC)",            // 0x0a
    "DEC BC",                // 0x0b
    "INC C",                // 0x0c
    "DEC C",                // 0x0d
    "LD C, {arg8}",            // 0x0e
    "RRCA",                    // 0x0f
    "STOP",                    // 0x10
    "LD DE, {arg16}",        // 0x11
    "LD (DE), A",            // 0x12
    "INC DE",                // 0x13
    "INC D",                // 0x14
    "DEC D",                // 0x15
    "LD D, {arg8}",            // 0x16
    "RLA",                    // 0x17
    "JR {arg8}",            // 0x18
    "ADD HL, DE",            // 0x19
    "LD A, (DE)",            // 0x1a
    "DEC DE",                // 0x1b
    "INC E",                // 0x1c
    "DEC E",                // 0x1d
    "LD E, {arg8}",            // 0x1e
    "RRA",                    // 0x1f
    "JR NZ, {arg8}",        // 0x20
    "LD HL, {arg16}",        // 0x21
    "LDI (HL), A",            // 0x22
    "INC HL",                // 0x23
    "INC H",                // 0x24
    "DEC H",                // 0x25
    "LD H, {arg8}",            // 0x26
    "DAA",                    // 0x27
    "JR Z, {arg8}",            // 0x28
    "ADD HL, HL",            // 0x29
    "LDI A, (HL)",            // 0x2a
    "DEC HL",                // 0x2b
    "INC L",                // 0x2c
    "DEC L",                // 0x2d
    "LD L, {arg8}",            // 0x2e
    "CPL",                    // 0x2f
    "JR NC, {arg8}",        // 0x30
    "LD SP, {arg16}",        // 0x31
    "LDD (HL), A",            // 0x32
    "INC SP",                // 0x33
    "INC (HL)",                // 0x34
    "DEC (HL)",                // 0x35
    "LD (HL), {arg8}",        // 0x36
    "SCF",                    // 0x37
    "JR C, {arg8}",            // 0x38
    "ADD HL, SP",            // 0x39
    "LDD A, (HL)",            // 0x3a
    "DEC SP",                // 0x3b
    "INC A",                // 0x3c
    "DEC A",                // 0x3d
    "LD A, {arg8}",            // 0x3e
    "CCF",                    // 0x3f
    "LD B, B",                // 0x40
    "LD B, C",                // 0x41
    "LD B, D",                // 0x42
    "LD B, E",                // 0x43
    "LD B, H",                // 0x44
    "LD B, L",                // 0x45
    "LD B, (HL)",            // 0x46
    "LD B, A",                // 0x47
    "LD C, B",                // 0x48
    "LD C, C",                // 0x49
    "LD C, D",                // 0x4a
    "LD C, E",                // 0x4b
    "LD C, H",                // 0x4c
    "LD C, L",                // 0x4d
    "LD C, (HL)",            // 0x4e
    "LD C, A",                // 0x4f
    "LD D, B",                // 0x50
    "LD D, C",                // 0x51
    "LD D, D",                // 0x52
    "LD D, E",                // 0x53
    "LD D, H",                // 0x54
    "LD D, L",                // 0x55
    "LD D, (HL)",            // 0x56
    "LD D, A",                // 0x57
    "LD E, B",                // 0x58
    "LD E, C",                // 0x59
    "LD E, D",                // 0x5a
    "LD E, E",                // 0x5b
    "LD E, H",                // 0x5c
    "LD E, L",                // 0x5d
    "LD E, (HL)",            // 0x5e
    "LD E, A",                // 0x5f
    "LD H, B",                // 0x60
    "LD H, C",                // 0x61
    "LD H, D",                // 0x62
    "LD H, E",                // 0x63
    "LD H, H",                // 0x64
    "LD H, L",                // 0x65
    "LD H, (HL)",            // 0x66
    "LD H, A",                // 0x67
    "LD L, B",                // 0x68
    "LD L, C",                // 0x69
    "LD L, D",                // 0x6a
    "LD L, E",                // 0x6b
    "LD L, H",                // 0x6c
    "LD L, L",                // 0x6d
    "LD L, (HL)",            // 0x6e
    "LD L, A",                // 0x6f
    "LD (HL), B",            // 0x70
    "LD (HL), C",            // 0x71
    "LD (HL), D",            // 0x72
    "LD (HL), E",            // 0x73
    "LD (HL), H",            // 0x74
    "LD (HL), L",            // 0x75
    "HALT",                    // 0x76
    "LD (HL), A",            // 0x77
    "LD A, B",                // 0x78
    "LD A, C",                // 0x79
    "LD A, D",                // 0x7a
    "LD A, E",                // 0x7b
    "LD A, H",                // 0x7c
    "LD A, L",                // 0x7d
    "LD A, (HL)",            // 0x7e
    "LD A, A",                // 0x7f
    "ADD A, B",                // 0x80
    "ADD A, C",                // 0x81
    "ADD A, D",                // 0x82
    "ADD A, E",                // 0x83
    "ADD A, H",                // 0x84
    "ADD A, L",                // 0x85
    "ADD A, (HL)",            // 0x86
    "ADD A",                // 0x87
    "ADC B",                // 0x88
    "ADC C",                // 0x89
    "ADC D",                // 0x8a
    "ADC E",                // 0x8b
    "ADC H",                // 0x8c
    "ADC L",                // 0x8d
    "ADC (HL)",                // 0x8e
    "ADC A",                // 0x8f
    "SUB B",                // 0x90
    "SUB C",                // 0x91
    "SUB D",                // 0x92
    "SUB E",                // 0x93
    "SUB H",                // 0x94
    "SUB L",                // 0x95
    "SUB (HL)",                // 0x96
    "SUB A",                // 0x97
    "SBC B",                // 0x98
    "SBC C",                // 0x99
    "SBC D",                // 0x9a
    "SBC E",                // 0x9b
    "SBC H",                // 0x9c
    "SBC L",                // 0x9d
    "SBC (HL)",                // 0x9e
    "SBC A",                // 0x9f
    "AND B",                // 0xa0
    "AND C",                // 0xa1
    "AND D",                // 0xa2
    "AND E",                // 0xa3
    "AND H",                // 0xa4
    "AND L",                // 0xa5
    "AND (HL)",                // 0xa6
    "AND A",                // 0xa7
    "XOR B",                // 0xa8
    "XOR C",                // 0xa9
    "XOR D",                // 0xaa
    "XOR E",                // 0xab
    "XOR H",                // 0xac
    "XOR L",                // 0xad
    "XOR (HL)",                // 0xae
    "XOR A",                // 0xaf
    "OR B",                    // 0xb0
    "OR C",                    // 0xb1
    "OR D",                    // 0xb2
    "OR E",                    // 0xb3
    "OR H",                    // 0xb4
    "OR L",                    // 0xb5
    "OR (HL)",                // 0xb6
    "OR A",                    // 0xb7
    "CP B",                    // 0xb8
    "CP C",                    // 0xb9
    "CP D",                    // 0xba
    "CP E",                    // 0xbb
    "CP H",                    // 0xbc
    "CP L",                    // 0xbd
    "CP (HL)",                // 0xbe
    "CP A",                    // 0xbf
    "RET NZ",                // 0xc0
    "POP BC",                // 0xc1
    "JP NZ, {arg16}",        // 0xc2
    "JP {arg16}",            // 0xc3
    "CALL NZ, {arg16}",        // 0xc4
    "PUSH BC",                // 0xc5
    "ADD A, {arg8}",        // 0xc6
    "RST 0x00",                // 0xc7
    "RET Z",                // 0xc8
    "RET",                    // 0xc9
    "JP Z, {arg16}",            // 0xca
    "CB %02X",                // 0xcb
    "CALL Z, {arg16}",        // 0xcc
    "CALL {arg16}",            // 0xcd
    "ADC {arg8}",            // 0xce
    "RST 0x08",                // 0xcf
    "RET NC",                // 0xd0
    "POP DE",                // 0xd1
    "JP NC, {arg16}",        // 0xd2
    "UNKNOWN",                // 0xd3
    "CALL NC, {arg16}",        // 0xd4
    "PUSH DE",                // 0xd5
    "SUB {arg8}",            // 0xd6
    "RST 0x10",                // 0xd7
    "RET C",                // 0xd8
    "RETI",                    // 0xd9
    "JP C, {arg16}",            // 0xda
    "UNKNOWN",                // 0xdb
    "CALL C, {arg16}",        // 0xdc
    "UNKNOWN",                // 0xdd
    "SBC {arg8}",            // 0xde
    "RST 0x18",                // 0xdf
    "LD (0xFF00 + {arg8}), A",        // 0xe0
    "POP HL",                // 0xe1
    "LD (0xFF00 + C), A",    // 0xe2
    "UNKNOWN",                // 0xe3
    "UNKNOWN",                // 0xe4
    "PUSH HL",                // 0xe5
    "AND {arg8}",            // 0xe6
    "RST 0x20",                // 0xe7
    "ADD SP,{arg8}",        // 0xe8
    "JP HL",                // 0xe9
    "LD ({arg16}), A",        // 0xea
    "UNKNOWN",                // 0xeb
    "UNKNOWN",                // 0xec
    "UNKNOWN",                // 0xed
    "XOR {arg8}",            // 0xee
    "RST 0x28",                // 0xef
    "LD A, (0xFF00 + {arg8})",        // 0xf0
    "POP AF",                // 0xf1
    "LD A, (0xFF00 + C)",    // 0xf2
    "DI",                    // 0xf3
    "UNKNOWN",                // 0xf4
    "PUSH AF",                // 0xf5
    "OR {arg8}",            // 0xf6
    "RST 0x30",                // 0xf7
    "LD HL, SP+{arg8}",        // 0xf8
    "LD SP, HL",            // 0xf9
    "LD A, ({arg16})",        // 0xfa
    "EI",                    // 0xfb
    "UNKNOWN",                // 0xfc
    "UNKNOWN",                // 0xfd
    "CP {arg8}",            // 0xfe
    "RST 0x38"                // 0xff
];

fn unknown(_cpu: &mut Cpu, opcode: u8) {
    panic!("Unknown opcode! 0x{:#04X} PC: 0x{:#04X}", opcode, _cpu.pc);
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

    Cpu::stop,
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
    |cpu| unknown(cpu, 0xD3),
    |cpu| cpu.call_conditional(CpuFlag::Carry, true),
    |cpu| cpu.push(DE),
    |cpu| cpu.one_arg(Cpu::sub),
    |cpu| cpu.rst(0x0010),
    |cpu| cpu.ret_conditional(CpuFlag::Carry, false),
    Cpu::reti,
    |cpu| cpu.jump_conditional(CpuFlag::Carry, false),
    |cpu| unknown(cpu, 0xDB),
    |cpu| cpu.call_conditional(CpuFlag::Carry, false),
    |cpu| unknown(cpu, 0xDD),
    |cpu| cpu.one_arg(Cpu::sbc),
    |cpu| cpu.rst(0x0018),

    Cpu::store_highmem_immediate,
    |cpu| cpu.pop(HL),
    Cpu::store_highmem_reg,
    |cpu| unknown(cpu, 0xE3),
    |cpu| unknown(cpu, 0xE4),
    |cpu| cpu.push(HL),
    |cpu| cpu.one_arg(Cpu::and),
    |cpu| cpu.rst(0x0020),
    Cpu::add_sp,
    |cpu| cpu.zero_latency(|cpu| cpu.pc = cpu.registers.get_reg16(HL)),
    Cpu::store_accumulator,
    |cpu| unknown(cpu, 0xEB),
    |cpu| unknown(cpu, 0xEC),
    |cpu| unknown(cpu, 0xED),
    |cpu| cpu.one_arg(Cpu::xor),
    |cpu| cpu.rst(0x0028),
    Cpu::load_highmem_immediate,
    |cpu| cpu.pop(AF),
    Cpu::load_highmem_reg,
    |cpu| cpu.zero_latency(Cpu::di),
    |cpu| unknown(cpu, 0xF4),
    |cpu| cpu.push(AF),
    |cpu| cpu.one_arg(Cpu::or),
    |cpu| cpu.rst(0x0030),
    Cpu::ldhl_sp_offset,
    Cpu::ld_sp_hl,
    Cpu::load_accumulator,
    Cpu::ei,
    |cpu| unknown(cpu, 0xFC),
    |cpu| unknown(cpu, 0xFD),
    |cpu| cpu.one_arg(Cpu::cp),
    |cpu| cpu.rst(0x0038),
];
