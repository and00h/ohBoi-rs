mod instructions;
mod prefixed_insts;

use std::cell::{Ref, RefCell};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use crate::core::bus::{Bus, BusController};
use crate::core::interrupts::{Interrupt, InterruptController};

#[derive(Clone, Copy, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum Register8 {
    B = 0, C = 1, D = 2, E = 3, H = 4, L = 5, F = 6, A = 7
}

#[derive(Clone, Copy, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) enum Register16 {
    AF = 0x76, BC = 0x1, DE = 0x23, HL = 0x45
}

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
use crate::core::cpu::instructions::{Instruction, OPS};
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

pub(crate) static INTERRUPT_VECTORS: [(Interrupt, u16); 5] = [
    (Interrupt::JPAD, 0x40),
    (Interrupt::SERIAL, 0x48),
    (Interrupt::TIMER, 0x50),
    (Interrupt::LCD, 0x58),
    (Interrupt::VBLANK, 0x60)
];

pub(crate) struct Cpu {
    interrupt_controller: Rc<RefCell<InterruptController>>,
    bus: BusController,

    registers: Registers,
    pc: u16,
    sp: u16,

    ei_last_instruction: bool,
    halted: bool,
    halt_bug_trigger: bool,
    double_speed: bool,

    cycles: u64,
    cgb: bool,
}

impl Cpu {
    pub fn new(interrupt_controller: Rc<RefCell<InterruptController>>, bus: BusController) -> Self {
        unimplemented!()
    }

    pub fn reset(&mut self) {
        self.sp = 0xFFFE;
        self.pc = 0x0100;

        self.ei_last_instruction = false;
        self.halted = false;
        self.halt_bug_trigger = false;
        self.double_speed = false;
        self.cycles = 0;

        (*self.interrupt_controller).borrow_mut().reset();
    }

    pub fn step(&mut self) {
        self.execute_ei();
//        self.io.lock().unwrap().check_timer_overflow();

        if self.halted {
            self.clock(4);
        } else {
            self.exec_next();
        }

//        self.io.lock().unwrap().update_buttons();

        for vec in self.service_interrupts() {
            self.call(vec);
            self.clock(8);
        }
    }

    fn exec_next(&mut self) {
        let opcode = self.read_memory(self.pc) as usize;
        let op = &OPS[opcode];
        if self.halt_bug_trigger {
            self.halt_bug_trigger = false;
        } else {
            self.pc += 1;
        }

        match op {
            Instruction::NoArgs(f) => {
                f(self);
            }
            Instruction::OneArg(f) => {
                let arg = self.read_memory(self.pc);
                self.pc += 1;
                f(self, arg);
            }
            Instruction::TwoArgs(f) => {
                let arg = self.read_word(self.pc);
                self.pc += 2;
                f(self, arg);
            }
        }
    }

    fn execute_ei(&mut self) {
        if self.ei_last_instruction {
            self.ei_last_instruction = false;
            (*self.interrupt_controller).borrow_mut().ime = true;
        }
    }

    fn clock(&mut self, cycles: u64) {
        self.cycles += cycles;
        self.bus.advance(cycles);
    }

    fn service_interrupts(&mut self) -> Vec<u16> {
        let mut interrupts = (*self.interrupt_controller).borrow_mut();
        let pending = interrupts.interrupts_pending();
        if interrupts.ime && pending {
            self.halted = false;
            // if self.halted {
            //     self.halted = false;
            // }
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
            Vec::default()
        }

    }

    fn read_memory(&mut self, addr: u16) -> u8 {
        self.clock(4);
        self.bus.read(addr)
    }

    fn write_memory(&mut self, addr: u16, data: u8) {
        self.clock(4);
        self.bus.write(addr, data);
    }

    #[inline]
    fn read_word(&mut self, addr: u16) -> u16 {
        (self.read_memory(addr) as u16) | ((self.read_memory(addr + 1) as u16) << 8)
    }

    #[inline]
    fn store_word(&mut self, addr: u16, data: u16) {
        self.write_memory(addr, (data & 0xFF) as u8);
        self.write_memory(addr + 1, ((data & 0xFF00) >> 8) as u8);
    }

    fn stack_pop(&mut self) -> u16 {
        let res = self.read_word(self.sp);
        self.sp = self.sp.wrapping_add(2);
        res
    }

    fn stack_push(&mut self, data: u16) {
        self.sp = self.sp.wrapping_sub(2);
        self.store_word(self.sp, data);
    }

    // Instructions
    fn adc(&mut self, val: u8) {
        let a = self.registers.get_reg8(A);
        let carry = if self.registers.test_flag(CpuFlag::Carry) {
            1
        } else {
            0
        };
        let res = a as u16 + val as u16 + carry as u16;

        self.registers.set_flag_cond(CpuFlag::Carry, res > 0xFF);
        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (a & 0xF) + (val & 0xF) + carry > 0xF);
        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers.reset_flag(CpuFlag::Sub);

        self.registers.set_reg8(A, res as u8);
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

    fn add_hl(&mut self, data: u16) {
        let hl = self.registers.get_reg16(HL);

        self.registers.set_flag_cond(
            CpuFlag::HalfCarry,
            (hl & 0x0FFF) > (0x0FFFu16.wrapping_sub(data & 0x0FFF)),
        );
        self.registers
            .set_flag_cond(CpuFlag::Carry, hl > (0xFFFFu16.wrapping_sub(data)));
        self.registers.reset_flag(CpuFlag::Sub);

        self.registers.set_reg16(HL, hl + data);
        self.clock(4);
    }

    fn add_sp(&mut self, imm: u8) {
        let sign_ext_imm = (imm as i8) as u16;
        self.registers.set_flag_cond(
            CpuFlag::HalfCarry,
            (self.sp & 0xF) as u8 > (0xF - (imm & 0xF)),
        );
        self.registers
            .set_flag_cond(CpuFlag::Carry, self.sp as u8 > 0xFF - imm);
        self.sp = self.sp.wrapping_add(sign_ext_imm);
        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);
        self.clock(8);
    }

    fn and(&mut self, val: u8) {
        let res = self.registers.get_reg8(A) & val;

        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.set_flag(CpuFlag::HalfCarry);
        self.registers.reset_flag(CpuFlag::Carry);

        self.registers.set_reg8(A, res);
    }

    fn call(&mut self, addr: u16) {
        self.stack_push(self.pc);
        self.pc = addr;
        self.clock(4);
    }

    fn callc(&mut self, addr: u16, cond: CpuFlag, negate: bool) {
        if self.registers.test_flag(cond) ^ negate {
            self.call(addr);
        }
    }

    fn cb(&mut self, data: u8) {
        PREFIXED_INSTS[data as usize](self);
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
                val = (val - 6) & 0xFF;
            }
            if self.registers.test_flag(CpuFlag::Carry) {
                val = (val - 0x60) & 0xFF;
            }
        } else {
            if self.registers.test_flag(CpuFlag::HalfCarry) || (val & 0xF) > 9 {
                val += 0x06;
            }
            if self.registers.test_flag(CpuFlag::Carry) || val > 0x9F {
                val += 0x60;
            }
        }

        self.registers.reset_flag(CpuFlag::HalfCarry);
        self.registers.set_flag_cond(CpuFlag::Carry, val > 0xFF);
        val &= 0xFF;
        self.registers.set_flag_cond(CpuFlag::Zero, val == 0);
        self.registers.set_reg8(A, val as u8);
    }

    fn dec16(&mut self, reg: Register16) {
        self.clock(4);
        let data = self.registers.get_reg16(reg).wrapping_sub(1);
        self.registers.set_reg16(reg, data);
    }

    fn dec8(&mut self, reg: Register8) {
        let mut data = self.registers.get_reg8(reg);

        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (data & 0xF) == 0);
        data = data.wrapping_sub(1);

        self.registers.set_flag_cond(CpuFlag::Zero, data == 0);
        self.registers.set_flag(CpuFlag::Sub);

        self.registers.set_reg8(reg, data);
    }

    fn dec_hl(&mut self) {
        let addr = self.registers.get_reg16(HL);
        let data = self.read_memory(addr);

        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (data & 0xF) != 0);
        let decremented = data.wrapping_sub(1);
        self.registers.set_flag_cond(CpuFlag::Zero, decremented == 0);
        self.registers.set_flag(CpuFlag::Sub);

        self.write_memory(addr, decremented);
    }

    fn dec_sp(&mut self) {
        self.clock(4);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn di(&mut self) {
        (*self.interrupt_controller).borrow_mut().ime = false;
    }

    fn ei(&mut self) {
        self.ei_last_instruction = true;
    }

    fn halt(&mut self) {
        let interrupts = (*self.interrupt_controller).borrow();
        if interrupts.ime {
            self.halted = true;
        } else {
            if interrupts.interrupts_pending() {
                self.halt_bug_trigger = true;
            } else {
                self.halted = true;
            }
        }
    }

    fn inc16(&mut self, reg: Register16) {
        self.clock(4);
        let data = self.registers.get_reg16(reg).wrapping_add(1);
        self.registers.set_reg16(reg, data);
    }

    fn inc8(&mut self, reg: Register8) {
        let mut data = self.registers.get_reg8(reg);

        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (data & 0xF) == 0);
        data = data.wrapping_add(1);

        self.registers.set_flag_cond(CpuFlag::Zero, data == 0);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.set_reg8(reg, data);
    }

    fn inc_hl(&mut self) {
        let addr = self.registers.get_reg16(HL);
        let data = self.read_memory(addr);

        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (data & 0xF) == 0xF);
        let incremented = data.wrapping_add(1);

        self.registers.set_flag_cond(CpuFlag::Zero, incremented == 0);
        self.registers.reset_flag(CpuFlag::Sub);

        self.write_memory(addr, incremented);
    }

    fn inc_sp(&mut self) {
        self.clock(4);
        self.sp = self.sp.wrapping_add(1);
    }

    fn jp(&mut self, addr: u16) {
        self.pc = addr;
        self.clock(4);
    }

    fn jpc(&mut self, addr: u16, cond: CpuFlag, negate: bool) {
        if self.registers.test_flag(cond) ^ negate {
            self.jp(addr);
        }
    }

    fn jr(&mut self, offset: u8) {
        let offset = offset as i8;
        // This works, since Rust sign-extends when converting from a smaller signed integer
        // to a larger unsigned integer
        self.pc = self.pc.wrapping_add(offset as u16);
        self.clock(4);
    }

    fn jrc(&mut self, addr: u8, flag: CpuFlag, negate: bool) {
        if self.registers.test_flag(flag) ^ negate {
            self.jr(addr);
        }
    }

    fn load(&mut self, reg: Register8, addr: u16) {
        let data = self.read_memory(addr);
        self.registers.set_reg8(reg, data);
    }

    fn load_immediate(&mut self, reg: Register8, data: u8) {
        self.registers.set_reg8(reg, data);
    }

    fn load_indirect(&mut self, reg: Register8, src: Register16) {
        let data = self.read_memory(self.registers.get_reg16(src));
        self.registers.set_reg8(reg, data);
    }

    fn ldc_out(&mut self) {
        self.write_memory(
            0xFF00 + self.registers.get_reg8(C) as u16,
            self.registers.get_reg8(A),
        );
    }

    fn ldhl(&mut self, offset: i8) {
        self.clock(4);
        let res = self.sp.wrapping_add(offset as u16);
        if offset >= 0 {
            self.registers.set_flag_cond(
                CpuFlag::Carry,
                (self.sp & 0xFF).wrapping_add(offset as u16) > 0xFF,
            );
            self.registers.set_flag_cond(
                CpuFlag::HalfCarry,
                (self.sp & 0xF).wrapping_add((offset & 0xF) as u16) > 0xF,
            );
        } else {
            self.registers
                .set_flag_cond(CpuFlag::Carry, (res & 0xFF) <= (self.sp & 0xFF));
            self.registers
                .set_flag_cond(CpuFlag::HalfCarry, (res & 0xF) <= (self.sp & 0xF));
        }

        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);

        self.registers.set_reg16(HL, res);
    }

    fn ldh_in(&mut self, offset: u8) {
        let data = self.read_memory(0xFF00 + offset as u16);
        self.registers.set_reg8(A, data);
    }

    fn ldc_in(&mut self) {
        let data = self.read_memory(0xFF00 + self.registers.get_reg8(C) as u16);
        self.registers.set_reg8(A, data);
    }

    fn ldh_out(&mut self, offset: u8) {
        self.write_memory(0xFF00 + offset as u16, self.registers.get_reg8(A));
    }

    fn ldsphl(&mut self) {
        self.clock(4);
        self.sp = self.registers.get_reg16(HL);
    }

    fn load_word(&mut self, reg: Register16, data: u16) {
        self.registers.set_reg16(reg, data);
    }

    fn or(&mut self, val: u8) {
        let res = self.registers.get_reg8(A) | val;

        self.registers.set_flag_cond(CpuFlag::Zero, res == 0);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
        self.registers.reset_flag(CpuFlag::Carry);

        self.registers.set_reg8(A, res);
    }

    fn poop(&mut self, reg: Register16) {
        let data = self.stack_pop();
        self.registers.set_reg16(reg, data);
    }

    fn poosh(&mut self, reg: Register16) {
        self.clock(4);
        let data = self.registers.get_reg16(reg);
        self.stack_push(data);
    }

    fn ret(&mut self) {
        self.pc = self.stack_pop();
        self.clock(4);
    }

    fn retc(&mut self, cond: CpuFlag, negate: bool) {
        if self.registers.test_flag(cond) ^ negate {
            self.ret();
        }
    }

    fn reti(&mut self) {
        self.ret();
        (*self.interrupt_controller).borrow_mut().ime = true;
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
        let a = self.registers.get_reg8(A);
        self.registers.set_reg8(A, a.rotate_left(1));

        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
        self.registers.set_flag_cond(CpuFlag::Carry, (a & 1) == 1);
    }

    fn rra(&mut self) {
        let c = if self.registers.test_flag(CpuFlag::Carry) { 1 << 7 } else { 0 };
        let mut a = self.registers.get_reg8(A);

        self.registers.set_flag_cond(CpuFlag::Carry, (a & 1) == 1);
        a >>= 1;
        a |= c;
        self.registers.set_reg8(A, a);

        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
    }

    fn rrca(&mut self) {
        let a = self.registers.get_reg8(A);
        self.registers.set_reg8(A, a.rotate_right(1));

        self.registers.reset_flag(CpuFlag::Zero);
        self.registers.reset_flag(CpuFlag::Sub);
        self.registers.reset_flag(CpuFlag::HalfCarry);
        self.registers.set_flag_cond(CpuFlag::Carry, (a & 0x80) == 0x80);
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
        todo!()
    }

    fn store(&mut self, reg: Register8, addr: u16) {
        self.write_memory(addr, self.registers.get_reg8(reg));
    }

    fn store_indirect(&mut self, reg: Register8, dst: Register16) {
        let addr = self.registers.get_reg16(dst);
        self.store(reg, addr);
    }

    fn sub(&mut self, val: u8) {
        let a = self.registers.get_reg8(A);

        self.registers.set_flag_cond(CpuFlag::Zero, a == val);
        self.registers
            .set_flag_cond(CpuFlag::HalfCarry, (val & 0xF) > (a & 0xF));
        self.registers.set_flag_cond(CpuFlag::Carry, val > a);
        self.registers.set_flag(CpuFlag::Sub);

        self.registers.set_reg8(A, a - val);
    }

    fn sw(&mut self, reg: Register16, addr: u16) {
        self.store_word(addr, self.registers.get_reg16(reg))
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