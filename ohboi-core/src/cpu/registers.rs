// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use super::CpuFlag;
use std::fmt::{Debug, Formatter};
use std::ops::{Index, IndexMut};

use Register8::*;
use Register16::*;
#[derive(Clone, Copy, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(usize)]
pub enum Register8 {
    B = 0, C = 1, D = 2, E = 3, H = 4, L = 5, F = 6, A = 7
}

impl Register8 {
    pub fn from_word_reg(reg: Register16) -> (Self, Self) {
        match reg {
            AF => (A, F),
            BC => (B, C),
            DE => (D, E),
            HL => (H, L)
        }
    }
}

#[repr(usize)]
#[derive(Clone, Copy, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Register16 {
    AF = 0x76, BC = 0x1, DE = 0x23, HL = 0x45
}

#[derive(Clone)]
pub struct Registers {
    regs: Vec<u8>,
    cgb: bool
}

impl Registers {
    pub fn new(cgb: bool) -> Self {
        Registers {
            regs: if cgb {
                vec![0x00, 0x00, 0xFF, 0x56, 0x00, 0x0D, 0xB0, 0x11]
            } else {
                vec![0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D, 0xB0, 0x01]
            },
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
    pub(crate) fn set_flag_cond(&mut self, flag: CpuFlag, condition: bool) {
        if condition {
            self.regs[F as usize] |= flag as u8;
        } else {
            self.regs[F as usize] &= !(flag as u8);
        }
    }

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: CpuFlag) {
        self.regs[F as usize] |= flag as u8;
    }

    #[inline]
    pub(crate) fn reset_flag(&mut self, flag: CpuFlag) {
        self.regs[F as usize] &= !(flag as u8);
    }

    #[inline]
    pub(crate) fn test_flag(&mut self, flag: CpuFlag) -> bool {
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

impl Index<Register8> for Registers {
    type Output = u8;

    fn index(&self, index: Register8) -> &Self::Output {
        &self.regs[index as usize]
    }
}

impl IndexMut<Register8> for Registers {
    fn index_mut(&mut self, index: Register8) -> &mut Self::Output {
        &mut self.regs[index as usize]
    }
}