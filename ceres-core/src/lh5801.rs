use core::panic;

use crate::Pc1500;

const DO_DEBUG_ROM: bool = true;

const CF: u8 = 0x01;
const IE: u8 = 0x02;
const ZF: u8 = 0x04;
const VF: u8 = 0x08;
const HF: u8 = 0x10;

#[derive(Debug, Default)]
pub struct Lh5801 {
    a: u8,
    t: u8,
    x: u16,
    y: u16,
    u: u16,
    s: u16,
    p: u16,

    is_halted: bool,

    pu: bool,
    pv: bool,
    bf: bool,
    disp: bool,
    tm: u16,

    ir0: bool,
    ir1: bool,
    ir2: bool,

    reset_flag: bool,
    is_timer_reached: bool,

    timer_state: usize,
    step_previous_state: usize,
    ticks: usize,

    // DEBUG UTILS
    debug_messages: usize,
    print_insts: bool,
}

impl Lh5801 {
    pub fn set_ir2(&mut self, ir2: bool) {
        self.ir2 = ir2;
    }

    #[must_use]
    pub const fn a(&self) -> u8 {
        self.a
    }

    #[must_use]
    pub const fn p(&self) -> u16 {
        self.p
    }

    #[must_use]
    pub const fn s(&self) -> u16 {
        self.s
    }

    #[must_use]
    pub const fn u(&self) -> u16 {
        self.u
    }

    #[must_use]
    pub const fn x(&self) -> u16 {
        self.x
    }

    #[must_use]
    pub const fn y(&self) -> u16 {
        self.y
    }

    #[must_use]
    pub const fn t(&self) -> u8 {
        self.t
    }

    #[must_use]
    pub const fn ie(&self) -> bool {
        self.t & IE != 0
    }

    #[must_use]
    pub const fn is_halted(&self) -> bool {
        self.is_halted
    }

    pub fn set_pc(&mut self, pc: u16) {
        self.p = pc;
    }

    pub fn set_a(&mut self, a: u8) {
        self.a = a;
    }

    #[must_use]
    pub const fn xl(&self) -> u8 {
        (self.x & 0xFF) as u8
    }
    #[must_use]
    pub const fn yl(&self) -> u8 {
        (self.y & 0xFF) as u8
    }
    #[must_use]
    pub const fn ul(&self) -> u8 {
        (self.u & 0xFF) as u8
    }
    #[must_use]
    pub const fn sl(&self) -> u8 {
        (self.s & 0xFF) as u8
    }
    #[must_use]
    pub const fn pl(&self) -> u8 {
        (self.p & 0xFF) as u8
    }

    #[must_use]
    pub const fn xh(&self) -> u8 {
        (self.x >> 8) as u8
    }

    #[must_use]
    pub const fn yh(&self) -> u8 {
        (self.y >> 8) as u8
    }
    #[must_use]
    pub const fn uh(&self) -> u8 {
        (self.u >> 8) as u8
    }
    #[must_use]
    pub const fn sh(&self) -> u8 {
        (self.s >> 8) as u8
    }
    #[must_use]
    pub const fn ph(&self) -> u8 {
        (self.p >> 8) as u8
    }

    pub fn set_xl(&mut self, val: u8) {
        self.x = (self.x & 0xFF00) | u16::from(val);
    }

    pub fn set_yl(&mut self, val: u8) {
        self.y = (self.y & 0xFF00) | u16::from(val);
    }

    pub fn set_ul(&mut self, val: u8) {
        self.u = (self.u & 0xFF00) | u16::from(val);
    }

    pub fn set_xh(&mut self, val: u8) {
        self.x = (self.x & 0x00FF) | (u16::from(val) << 8);
    }

    pub fn set_yh(&mut self, val: u8) {
        self.y = (self.y & 0x00FF) | (u16::from(val) << 8);
    }

    pub fn set_uh(&mut self, val: u8) {
        self.u = (self.u & 0x00FF) | (u16::from(val) << 8);
    }

    pub fn get_ticks(&self) -> usize {
        self.ticks
    }

    pub fn set_ticks(&mut self, ticks: usize) {
        self.ticks = ticks;
    }

    pub fn new() -> Self {
        let mut ret = Lh5801::default();
        ret.reset_flag = true;
        ret.debug_messages = 1200;
        ret
    }

    pub const fn timer_state(&self) -> usize {
        self.timer_state
    }

    pub const fn display_enabled(&self) -> bool {
        self.disp
    }

    pub const fn pu(&self) -> bool {
        self.pu
    }

    pub const fn pv(&self) -> bool {
        self.pv
    }
}

impl Pc1500 {
    fn get_mem16(&self, addr: u32) -> u16 {
        (self.read_byte(addr.wrapping_add(1)) as u16) | ((self.read_byte(addr) as u16) << 8)
    }

    fn reset(&mut self) {
        self.lh5801.reset_flag = true;
    }

    fn cpu_internal_reset(&mut self) {
        self.lh5801.reset_flag = true;
        self.set_p(self.get_mem16(0xFFFE));

        self.lh5801.a = 0;
        self.lh5801.t = 0;
        self.lh5801.x = 0;
        self.lh5801.y = 0;
        self.lh5801.u = 0;
        self.lh5801.s = 0x1000;
        self.lh5801.is_halted = false;
        self.lh5801.ir0 = false;
        self.lh5801.ir1 = false;
        self.lh5801.ir2 = false;
        self.lh5801.timer_state = 0;
        self.lh5801.bf = true;
        self.lh5801.tm = 0;
        self.lh5801.pu = false;
        self.lh5801.pv = false;

        self.lh5801.reset_flag = false;
    }

    fn set_p(&mut self, addr: u16) {
        if DO_DEBUG_ROM {
            if self.lh5801.debug_messages < 1000 {
                self.lh5801.debug_messages += 1;
            }

            match addr {
                0xC8B4 => println!("BCMD_RUN"),
                0xC9E4 => println!("COLD_START"),
                0xCFCC => println!("INIT_SYS_ADDR"),
                0xD02B => println!("INBUF_CLR_1"),
                0xD030 => println!("INBUF_CLR_2"),
                0xD0B4 => println!("PRG_SEARCH"),
                0xDF63 => println!("IS_STRING"),
                0xE000 => println!("RESET"),
                0xE153 => println!("IO_INT"),
                0xE171 => println!("ISR_HANDLER"),
                0xE22C => println!("TIMER_ISR"),
                0xE234 => println!("PVBANK"),
                0xE243 => println!("WAIT_4_KB"),
                0xE246 => println!("WAIT_4_KB_1"),
                0xE24E => println!("WAIT_4_KB_2"),
                0xE25B => println!("WAIT_4_KB_3"),
                0xE269 => println!("WAIT_4_KB_4"),
                0xE29D => println!("WAIT_4_KB_5"),
                0xE29E => println!("WAIT_4_KB_6"),
                0xE2AC => println!("WAIT_4_KB_7"),
                0xE2B7 => println!("WAIT_4_KB_8"),
                0xE2C2 => println!("WAIT_4_KB_9"),
                0xE2C4 => println!("WAIT_4_KB_10"),
                0xE2D8 => println!("WAIT_4_KB_11"),
                0xE2DE => println!("WAIT_4_KB_12"),
                0xE2E4 => println!("WAIT_4_KB_13"),
                0xE2F2 => println!("WAIT_4_KB_14"),
                0xE2F6 => println!("WAIT_4_KB_15"),
                0xE2FF => println!("WAIT_4_KB_16"),
                0xE303 => println!("WAIT_4_KB_17"),
                0xE311 => println!("WAIT_4_KB_18"),
                0xE315 => println!("WAIT_4_KB_19"),
                0xE334 => println!("WAIT_4_KB_20"),
                0xE33A => println!("WAIT_4_KB_21"),
                0xE33F => println!("AUTO_OFF"),
                0xE347 => println!("AUTO_OFF_1"),
                0xE366 => println!("AUTO_OFF_2"),
                0xE37B => println!("AUTO_OFF_3"),
                0xE385 => println!("AUTO_OFF_4"),
                0xE38D => println!("AUTO_OFF_5"),
                0xE39D => println!("AUTO_OFF_6"),
                0xE39E => println!("AUTO_OFF_6_1"),
                0xE3A1 => println!("AUTO_OFF_7"),
                0xE3A7 => println!("AUTO_OFF_8"),
                0xE3AC => println!("AUTO_OFF_9"),
                0xE3B1 => println!("AUTO_OFF_10"),
                0xE3B3 => println!("AUTO_OFF_11"),
                0xE3BC => println!("AUTO_OFF_12"),
                0xE3C2 => println!("AUTO_OFF_13"),
                0xE3C8 => println!("AUTO_OFF_14"),
                0xE3E8 => println!("AUTO_OFF_15"),
                0xE3EF => println!("AUTO_OFF_16"),
                0xE3F6 => println!("AUTO_OFF_17"),
                0xE408 => println!("AUTO_OFF_18"),
                0xE40C => println!("AUTO_OFF_19"),
                0xE413 => println!("AUTO_OFF_20"),
                0xCA7A => println!("EDITOR"),
                0xCA7D => println!("EDITOR_1"),
                0xCAAE => {
                    println!("EDITOR_2");
                }
                0xCADA => println!("EDITOR_3"),
                0xCADF => {
                    println!("EDITOR_4");
                }
                0xCAE8 => {
                    println!("EDITOR_5");
                }
                0xCAF8 => println!("EDITOR_6"),
                0xCAFC => println!("EDITOR_7"),
                0xCB27 => println!("EDITOR_8"),
                0xCB2B => println!("EDITOR_9"),
                0xCB2D => println!("EDITOR_10"),
                0xCB3C => println!("EDITOR_11"),
                0xCB46 => println!("EDITOR_12"),
                0xE418 => println!("ISKEY"),
                0xE41A => {
                    println!("ISKEY_1");
                }
                0xE425 => println!("ISKEY_2"),
                0xE42C => {
                    println!("KEY_2_ASCII");
                    // self.lh5801.print_insts = true;
                }
                0xE430 => println!("KEY_2_ASCII_1"),
                0xE441 => println!("KEY_2_ASCII_2"),
                0xE444 => println!("KEY_2_ASCII_3"),
                0xE44C => {
                    println!("KEY_2_ASCII_4");
                }
                0xE451 => println!("CHK_BRK"),
                0xE4A8 => println!("TOK_TABL_SRCH"),
                0xE573 => println!("TIMER_MODE"),
                0xEDEF => println!("GPRINT_OUT, A = {:02X}", self.lh5801.a()),
                0xEDF6 => {
                    println!("GPRINT_OUT_1, A = {:02X}", self.lh5801.a());
                    // self.lh5801.print_insts = true;
                }
                0xF5B5 => println!("BCMD_PI"),
                0xF61B => println!("RAND_GEN_5"),
                0xF729 => println!("XFER_SM_ARX2ARY"),
                0xF733 => println!("XREG_2_YREG"),
                0xF73D => println!("XFER_ARY_2_ARX"),
                0xF763 => println!("CLR_N_XREG"),
                0xF79C => println!("ARX_SHL_4BITS"),
                0xF7B0 => println!("SET_HB_XYREGS"),
                0xF7CC => println!("ADD_SM_ARX_ARX"),
                0xED4D => println!("CHAR_OUT"),
                0xED57 => println!("CHAR_OUT_1"),
                0xED5B => println!("CHAR_OUT_2"),
                0xE9EB => {
                    println!("STATUS_CHK");
                }
                0xE9F8 => {
                    println!("STATUS_CHK_1");
                }
                0xE9F9 => println!("STATUS_CHK_2"),
                0xEA0E => println!("STATUS_CHK_3"),
                0xEA10 => println!("STATUS_CHK_4"),
                0xEA18 => println!("STATUS_CHK_5"),
                0xEA1E => println!("STATUS_CHK_6"),
                0xEA26 => println!("STATUS_CHK_7"),
                0xEA34 => println!("STATUS_CHK_8"),
                0xEA3D => println!("STATUS_CHK_9"),
                0xEA52 => println!("STATUS_CHK_10"),
                0xEA5C => println!("STATUS_CHK_11"),
                0xEA5D => println!("STATUS_CHK_12"),
                0xEA60 => println!("STATUS_CHK_13"),
                0xEA67 => println!("STATUS_CHK_14"),
                0xE8CA => println!("PRGM_DISP"),
                0xE8FF => println!("PRGM_DISP_4"),
                0xDDC8 => println!("LOAD_XREG"),
                0xCE9F => println!("RSRV_MEM_START"),
                0xCEAC => println!("RSRV_MEM_START_1"),
                0xE4EB => {
                    println!("BCMD_PRINT");
                    self.lh5801.debug_messages = 0
                }
                _ => {
                    if self.lh5801.debug_messages < 1000 {
                        self.lh5801.debug_messages -= 1;
                    }
                }
            }

            if self.lh5801.debug_messages < 1000 && self.lh5801.debug_messages > 20 {
                panic!("Too many debug messages");
            }
        }

        self.lh5801.p = addr;
    }

    pub fn step_cpu(&mut self) {
        if self.lh5801.reset_flag {
            self.cpu_internal_reset();
        }

        if self.lh5801.is_timer_reached {
            self.lh5801.ir1 = true;
            self.lh5801.is_timer_reached = false;
        }

        if self.lh5801.ir0 {
            // Connected to ground
        } else if self.lh5801.ir1 && self.lh5801.ie() {
            // println!("Timer interrupt, setting IR1 to false");
            self.push(self.lh5801.t);
            self.set_ie_flag(false);
            self.lh5801.ir1 = false;
            self.push_word(self.lh5801.p);
            self.set_p(self.get_mem16(0xFFFA));
            // println!("Jumped to vector address {:04X}", self.lh5801.p);
            self.lh5801.is_halted = false;
        } else if self.lh5801.ir2 && self.lh5801.ie() {
            // println!("Maskable interrupt, setting IR2 to false");

            self.push(self.lh5801.t);
            self.set_ie_flag(false);
            self.lh5801.ir2 = false;
            self.push_word(self.lh5801.p);
            self.set_p(self.get_mem16(0xFFF8));
            self.lh5801.is_halted = false;
        } else if self.lh5801.is_halted {
            self.add_state(2);
        } else {
            self.instruction();
        }

        let current_state = self.lh5801.timer_state;

        if current_state - self.lh5801.step_previous_state >= 42 {
            self.timer_inc();
            self.lh5801.step_previous_state += current_state - self.lh5801.step_previous_state;
        }
    }

    fn cpu_readmem<I: Into<u32> + Copy>(&mut self, addr: I) -> u8 {
        self.read_byte(addr.into())
    }

    fn cpu_writemem<I: Into<u32> + Copy>(&mut self, addr: I, val: u8) {
        self.write_byte(addr.into(), val);
    }

    fn cpu_readop(&mut self) -> u8 {
        let byte = self.read_byte(self.lh5801.p.into());
        self.lh5801.p = self.lh5801.p.wrapping_add(1);
        byte
    }

    fn readop_word(&mut self) -> u16 {
        let hi = self.cpu_readop() as u16;
        let lo = self.cpu_readop() as u16;
        (hi << 8) | lo
    }

    fn me1<I: Into<u32> + Copy>(addr: I) -> u32 {
        addr.into() | 0x10000
    }

    const fn get_flag(&self, flag: u8) -> bool {
        self.lh5801.t & flag != 0
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.lh5801.t |= flag;
        } else {
            self.lh5801.t &= !flag;
        }
    }

    pub fn set_ie_flag(&mut self, value: bool) {
        self.set_flag(IE, value);
    }

    pub fn set_zero_flag(&mut self, value: bool) {
        self.set_flag(ZF, value);
    }
    pub fn set_carry_flag(&mut self, value: bool) {
        self.set_flag(CF, value);
    }
    fn set_overflow_flag(&mut self, value: bool) {
        self.set_flag(VF, value);
    }

    fn set_half_carry_flag(&mut self, value: bool) {
        self.set_flag(HF, value);
    }

    fn get_zero_flag(&self) -> bool {
        self.get_flag(ZF)
    }
    fn get_carry_flag(&self) -> bool {
        self.get_flag(CF)
    }
    fn get_overflow_flag(&self) -> bool {
        self.get_flag(VF)
    }

    fn get_half_carry_flag(&self) -> bool {
        self.get_flag(HF)
    }

    fn check_z(&mut self, val: u8) {
        self.set_zero_flag(val == 0);
    }

    fn check_c(&mut self, val: u8) {
        self.set_carry_flag(val != 0);
    }

    fn check_h(&mut self, val: u8) {
        self.set_half_carry_flag(val != 0);
    }

    fn timer_inc(&mut self) {
        self.lh5801.tm = (self.lh5801.tm >> 1)
            | (((self.lh5801.tm & 0x01) ^ ((self.lh5801.tm & 0x10) >> 4)) << 8);
        self.lh5801.is_timer_reached = self.lh5801.tm == 0x1FF;
    }

    fn push(&mut self, value: u8) {
        self.cpu_writemem(self.lh5801.s, value);
        self.lh5801.s = self.lh5801.s.wrapping_sub(1);
    }

    fn push_word(&mut self, value: u16) {
        self.push((value & 0xFF) as u8);
        self.push((value >> 8) as u8);
    }

    fn pop(&mut self) {
        self.lh5801.s = self.lh5801.s.wrapping_add(1);
        self.lh5801.a = self.cpu_readmem(self.lh5801.s);
        self.check_z(self.lh5801.a);
    }

    fn pop_word(&mut self) -> u16 {
        self.lh5801.s = self.lh5801.s.wrapping_add(1);
        let hi = u16::from(self.cpu_readmem(self.lh5801.s));
        self.lh5801.s = self.lh5801.s.wrapping_add(1);
        let lo = u16::from(self.cpu_readmem(self.lh5801.s));
        (hi << 8) | lo
    }

    fn add_generic<I: Into<i16>>(&mut self, left: I, right: I, carry: bool) -> u8 {
        let left = left.into();
        let right = right.into();
        let res = left + right + if carry { 1 } else { 0 };

        self.lh5801.t &= !(HF | VF | ZF | CF);

        self.check_z((res & 0xff) as u8);

        let c = res & 0x100;
        self.set_carry_flag(c != 0);

        if ((left & 0x0f) + (right & 0x0f) + if carry { 1 } else { 0 }) & 0x10 != 0 {
            self.set_half_carry_flag(true);
        }

        let v = ((left & 0x7f) + (right & 0x7f) + if carry { 1 } else { 0 }) & 0x80;
        if (c != 0 && v == 0) || (c == 0 && v != 0) {
            self.set_overflow_flag(true);
        }

        res as u8
    }

    fn adc(&mut self, data: u8) {
        self.lh5801.a = self.add_generic(self.lh5801.a, data, self.get_carry_flag());
    }

    fn add_mem<I: Into<u32> + Copy>(&mut self, addr: I, data: u8) {
        let mem_read = self.cpu_readmem(addr);
        let v = self.add_generic(mem_read, data, false);
        self.cpu_writemem(addr, v);
    }

    fn adr(&mut self, reg: u16) -> u16 {
        let loc_t = self.lh5801.t;
        let rl = (reg & 0xFF) as u8;
        let rl = self.add_generic(rl, self.lh5801.a, false);
        let rh = (reg >> 8) as u8;
        let rh = rh.wrapping_add(if self.get_carry_flag() { 1 } else { 0 });
        let ret = ((rh as u16) << 8) | rl as u16;

        self.lh5801.t = loc_t;
        ret
    }

    fn sbc(&mut self, data: u8) {
        self.lh5801.a = self.add_generic(
            self.lh5801.a as i16,
            (data ^ 0xff) as i16,
            self.get_carry_flag(),
        );
    }

    fn cpa(&mut self, a: u8, b: u8) {
        // We only care about flags
        let _ = self.add_generic(a as i16, (b ^ 0xff) as i16, true);
    }

    fn decimaladd_generic<I: Into<i16>>(&mut self, left: I, right: I, carry: bool) -> u8 {
        let a = self.add_generic(left, right, carry);

        let adjustment = if !self.get_carry_flag() && !self.get_half_carry_flag() {
            0x9a
        } else if !self.get_carry_flag() && self.get_half_carry_flag() {
            0xa0
        } else if self.get_carry_flag() && !self.get_half_carry_flag() {
            0xfa
        } else {
            0x00
        };

        a.wrapping_add(adjustment)
    }

    fn dca(&mut self, data: u8) {
        self.lh5801.a = self.decimaladd_generic(self.lh5801.a + 0x66, data, self.get_carry_flag());
    }

    fn dcs(&mut self, data: u8) {
        self.lh5801.a = self.decimaladd_generic(
            self.lh5801.a as i16,
            (data ^ 0xff) as i16,
            self.get_carry_flag(),
        );
    }

    fn and(&mut self, data: u8) {
        self.lh5801.a &= data;
        self.check_z(self.lh5801.a);
    }

    fn and_mem<I: Into<u32> + Copy>(&mut self, addr: I, data: u8) {
        let data = data & self.cpu_readmem(addr);
        self.check_z(data);
        self.cpu_writemem(addr, data);
    }

    fn bit(&mut self, a: u8, b: u8) {
        self.check_z(a & b);
    }

    fn eor(&mut self, data: u8) {
        self.lh5801.a ^= data;
        self.check_z(self.lh5801.a);
    }

    fn ora(&mut self, data: u8) {
        self.lh5801.a |= data;
        self.check_z(self.lh5801.a);
    }

    fn ora_mem<I: Into<u32> + Copy>(&mut self, addr: I, mut data: u8) {
        data |= self.cpu_readmem(addr);
        self.check_z(data);
        self.cpu_writemem(addr, data);
    }

    fn lda(&mut self, data: u8) {
        self.lh5801.a = data;
        self.check_z(data);
    }

    fn lde(&mut self, reg: u16) -> u16 {
        self.lh5801.a = self.cpu_readmem(reg);
        let ret = reg.wrapping_sub(1);
        self.check_z(self.lh5801.a);
        ret
    }

    fn sde(&mut self, reg: u16) -> u16 {
        self.cpu_writemem(reg, self.lh5801.a);
        reg.wrapping_sub(1)
    }

    fn lin(&mut self, reg: u16) -> u16 {
        self.lh5801.a = self.cpu_readmem(reg);
        let ret = reg.wrapping_add(1);
        self.check_z(self.lh5801.a);
        ret
    }

    fn sin(&mut self, reg: u16) -> u16 {
        self.cpu_writemem(reg, self.lh5801.a);
        reg.wrapping_add(1)
    }

    fn dec(&mut self, adr: u8) -> u8 {
        self.add_generic(adr, 0xff, false)
    }

    fn inc(&mut self, adr: u8) -> u8 {
        self.add_generic(adr, 1, false)
    }

    fn rtn(&mut self) {
        let addr = self.pop_word();
        self.set_p(addr);

        // println!(
        //     "RTN to {:04X} with Z: {} and C: {}",
        //     addr,
        //     self.get_zero_flag(),
        //     self.get_carry_flag()
        // );

        // if addr == 0xe270 && !self.get_zero_flag() {
        //     // self.lh5801.debug_messages = 0;
        // }
        // self.lh5801.print_insts = false;
    }

    fn rti(&mut self) {
        let addr = self.pop_word();
        self.set_p(addr);
        self.lh5801.s = self.lh5801.s.wrapping_add(1);
        self.lh5801.t = self.cpu_readmem(self.lh5801.s);
    }

    fn jmp(&mut self, addr: u16) {
        self.set_p(addr);
    }

    fn add_state(&mut self, n: u8) {
        self.lh5801.timer_state += n as usize;
        self.lh5801.ticks += n as usize;
    }

    fn branch_plus(&mut self, doit: bool) {
        let t = self.cpu_readop();
        if doit {
            self.add_state(2);
            self.set_p(self.lh5801.p.wrapping_add(u16::from(t)));
        }
    }

    fn branch_minus(&mut self, doit: bool) {
        let t = self.cpu_readop();
        if doit {
            self.add_state(3);
            self.set_p(self.lh5801.p.wrapping_sub(u16::from(t)));
        }
    }

    fn lop(&mut self) {
        let t = self.cpu_readop();
        self.add_state(8);
        if self.lh5801.ul() != 0 {
            self.add_state(3);
            // self.set_p(self.lh5801.p.wrapping_sub(u16::from(t)));
            self.lh5801.p = self.lh5801.p.wrapping_sub(u16::from(t));
        }
        self.lh5801.set_ul(self.lh5801.ul().wrapping_sub(1));
    }

    fn sjp(&mut self) {
        let t = self.readop_word();
        self.push_word(self.lh5801.p);
        self.set_p(t);
        // println!("SJP to {:04X}", t);
    }

    fn vector(&mut self, doit: bool, nr: u8) {
        if doit {
            self.push_word(self.lh5801.p);
            let addr = self.get_mem16(0xFF00 | u32::from(nr));
            self.set_p(addr);

            // println!("VEC to {:04X} for vector {}", addr, nr);

            self.add_state(21 - 8);
        }
        self.set_zero_flag(false);
    }

    fn aex(&mut self) {
        let l = self.lh5801.a;
        self.lh5801.a = (l << 4) | (l >> 4);
    }

    fn drl<I: Into<u32> + Copy>(&mut self, addr: I) {
        let l = u16::from(self.lh5801.a) | (u16::from(self.cpu_readmem(addr)) << 8);
        self.lh5801.a = (l >> 8) as u8;
        self.cpu_writemem(addr, (l >> 4) as u8);
    }

    fn drr<I: Into<u32> + Copy>(&mut self, addr: I) {
        let l = u16::from(self.cpu_readmem(addr)) | (u16::from(self.lh5801.a) << 8);
        self.lh5801.a = (l & 0xFF) as u8;
        self.cpu_writemem(addr, (l >> 4) as u8);
    }

    fn rol(&mut self) {
        let l = self.lh5801.a;
        self.lh5801.a = (self.lh5801.a << 1) | if self.get_carry_flag() { 1 } else { 0 };

        self.check_c(l & 0x80);
        self.check_z(self.lh5801.a);
        self.check_h(self.lh5801.a & 0x10);
        self.set_overflow_flag((l >= 0x40) && (l < 0xc0));
    }

    fn ror(&mut self) {
        let l = self.lh5801.a;
        self.lh5801.a = (self.lh5801.a >> 1) | (if self.get_carry_flag() { 0x80 } else { 0 });

        self.check_c(l & 0x01);
        self.check_z(self.lh5801.a);
        self.check_h(self.lh5801.a & 0x08);
        self.set_overflow_flag(
            ((l & 0x01 != 0) && (self.lh5801.a & 0x02 != 0))
                || ((l & 0x02 != 0) && (self.lh5801.a & 0x01 != 0)),
        );
    }

    fn shl(&mut self) {
        let l = self.lh5801.a;
        self.lh5801.a <<= 1;

        self.check_c(l & 0x80);
        self.check_z(self.lh5801.a);
        self.check_h(l & 0x08);
        self.set_overflow_flag((l >= 0x40) && (l < 0xc0));
    }

    fn shr(&mut self) {
        let l = self.lh5801.a;
        self.lh5801.a >>= 1;

        self.check_c(l & 0x01);
        self.check_z(self.lh5801.a);
        self.check_h(self.lh5801.a & 0x08);
        self.set_overflow_flag(
            ((l & 0x01 != 0) && (self.lh5801.a & 0x02 != 0))
                || ((l & 0x02 != 0) && (self.lh5801.a & 0x01 != 0)),
        );
    }

    fn am(&mut self, value: u16) {
        // println!("AM called with value: {}", value);
        self.lh5801.tm = value;
    }

    fn ita(&mut self) {
        self.lh5801.a = self.keyboard.input();
    }

    fn instruction_fd(&mut self) {
        let oper = self.cpu_readop();

        if self.lh5801.print_insts {
            println!("fd instruction: {:02X}", oper);
        }

        match oper {
            0x01 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.sbc(read);
                self.add_state(11);
            }
            0x03 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.adc(read);
                self.add_state(11);
            }
            0x05 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.lda(read);
                self.add_state(10);
            }
            0x07 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.cpa(self.lh5801.a, read);
                self.add_state(11);
            }
            0x08 => {
                self.add_state(11);
            }
            0x09 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.and(read);
                self.add_state(11);
            }
            0x0a => {
                self.lh5801.x = self.pop_word();
                self.add_state(15);
            }
            0x0b => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.ora(read);
                self.add_state(11);
            }
            0x0c => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.dcs(read);
                self.add_state(17);
            }
            0x0d => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.eor(read);
                self.add_state(11);
            }
            0x0e => {
                self.cpu_writemem(Self::me1(self.lh5801.x()), self.lh5801.a);
                self.add_state(10);
            }
            0x0f => {
                let data = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.bit(data, self.lh5801.a);
                self.add_state(11);
            }
            0x11 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.sbc(read);
                self.add_state(11);
            }
            0x13 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.adc(read);
                self.add_state(11);
            }
            0x15 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.lda(read);
                self.add_state(10);
            }
            0x17 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.cpa(self.lh5801.a, read);
                self.add_state(11);
            }
            0x18 => {
                self.lh5801.x = self.lh5801.y;
                self.add_state(11);
            }
            0x19 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.and(read);
                self.add_state(11);
            }
            0x1a => {
                self.lh5801.y = self.pop_word();
                self.add_state(15);
            }
            0x1b => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.ora(read);
                self.add_state(11);
            }
            0x1c => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.dcs(read);
                self.add_state(17);
            }
            0x1d => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.eor(read);
                self.add_state(11);
            }
            0x1e => {
                self.cpu_writemem(Self::me1(self.lh5801.y()), self.lh5801.a);
                self.add_state(10);
            }
            0x1f => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.bit(read, self.lh5801.a);
                self.add_state(11);
            }
            0x21 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.sbc(read);
                self.add_state(11);
            }
            0x23 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.adc(read);
                self.add_state(11);
            }
            0x25 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.lda(read);
                self.add_state(10);
            }
            0x27 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.cpa(self.lh5801.a, read);
                self.add_state(11);
            }
            0x28 => {
                self.lh5801.x = self.lh5801.u;
                self.add_state(11);
            }
            0x29 => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.and(read);
                self.add_state(11);
            }
            0x2a => {
                self.lh5801.u = self.pop_word();
                self.add_state(15);
            }
            0x2b => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.ora(read);
                self.add_state(11);
            }
            0x2c => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.dcs(read);
                self.add_state(17);
            }
            0x2d => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.eor(read);
                self.add_state(11);
            }
            0x2e => {
                self.cpu_writemem(Self::me1(self.lh5801.u()), self.lh5801.a);
                self.add_state(10);
            }
            0x2f => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                self.bit(read, self.lh5801.a);
                self.add_state(11);
            }
            0x3a => {
                self.lh5801.s = self.lh5801.s.wrapping_add(2);
                self.add_state(15);
            }
            0x40 => {
                let inc = self.inc(self.lh5801.xh());
                self.lh5801.set_xh(inc);
                self.add_state(9);
            }
            0x42 => {
                let dec = self.dec(self.lh5801.xh());
                self.lh5801.set_xh(dec);
                self.add_state(9);
            }
            0x48 => {
                self.lh5801.x = self.lh5801.s;
                self.add_state(11);
            }
            0x49 => {
                let op = self.cpu_readop();
                self.and_mem(Self::me1(self.lh5801.x()), op);
                self.add_state(17);
            }
            0x4a => {
                self.add_state(11);
            }
            0x4b => {
                let op = self.cpu_readop();
                self.ora_mem(Self::me1(self.lh5801.x()), op);
                self.add_state(14);
            }
            0x4c => {
                self.lh5801.bf = false;
                self.add_state(8);
            }
            0x4d => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                let op = self.cpu_readop();
                self.bit(read, op);
                self.add_state(14);
            }
            0x4e => {
                self.lh5801.s = self.lh5801.x;
                self.add_state(11);
            }
            0x4f => {
                let op = self.cpu_readop();
                self.add_mem(Self::me1(self.lh5801.x()), op);
                self.add_state(14);
            }
            0x50 => {
                let inc = self.inc(self.lh5801.yh());
                self.lh5801.set_yh(inc);
                self.add_state(9);
            }
            0x52 => {
                let dec = self.dec(self.lh5801.yh());
                self.lh5801.set_yh(dec);
                self.add_state(9);
            }
            0x58 => {
                self.lh5801.x = self.lh5801.p;
                self.add_state(11);
            }
            0x59 => {
                let op = self.cpu_readop();
                self.and_mem(Self::me1(self.lh5801.y()), op);
                self.add_state(17);
            }
            0x5a => {
                self.lh5801.y = self.lh5801.x;
                self.add_state(11);
            }
            0x5b => {
                let op = self.cpu_readop();
                self.ora_mem(Self::me1(self.lh5801.y()), op);
                self.add_state(14);
            }
            0x5d => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                let op = self.cpu_readop();
                self.bit(read, op);
                self.add_state(14);
            }
            0x5e => {
                self.jmp(self.lh5801.x);
                self.add_state(11);
            }
            0x5f => {
                let op = self.cpu_readop();
                self.add_mem(Self::me1(self.lh5801.y()), op);
                self.add_state(14);
            }
            0x60 => {
                let inc = self.inc(self.lh5801.uh());
                self.lh5801.set_uh(inc);
                self.add_state(9);
            }
            0x62 => {
                let dec = self.dec(self.lh5801.uh());
                self.lh5801.set_uh(dec);
                self.add_state(9);
            }
            0x69 => {
                let op = self.cpu_readop();
                self.and_mem(Self::me1(self.lh5801.u()), op);
                self.add_state(17);
            }
            0x6a => {
                self.lh5801.u = self.lh5801.x;
                self.add_state(11);
            }
            0x6b => {
                let op = self.cpu_readop();
                self.ora_mem(Self::me1(self.lh5801.u()), op);
                self.add_state(14);
            }
            0x6d => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u()));
                let op = self.cpu_readop();
                self.bit(read, op);
                self.add_state(14);
            }
            0x6f => {
                let op = self.cpu_readop();
                self.add_mem(Self::me1(self.lh5801.u()), op);
                self.add_state(17);
            }
            0x81 => {
                self.set_ie_flag(true);
                self.add_state(8);
            }
            0x88 => {
                self.push_word(self.lh5801.x);
                self.add_state(14);
            }
            0x8a => {
                self.pop();
                self.add_state(12);
            }
            0x8c => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.x()));
                self.dca(read);
                self.add_state(19);
            }

            0x98 => {
                self.push_word(self.lh5801.y);
                self.add_state(14);
            }
            0x9c => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.y()));
                self.dca(read);
                self.add_state(19);
            }
            0xa1 => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                self.sbc(read);
                self.add_state(17);
            }
            0xa3 => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                self.adc(read);
                self.add_state(17);
            }
            0xa5 => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                self.lda(read);
                self.add_state(16);
            }
            0xa7 => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                self.cpa(self.lh5801.a, read);
                self.add_state(17);
            }
            0xa8 => {
                self.push_word(self.lh5801.u);
                self.add_state(14);
            }
            0xa9 => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                self.and(read);
                self.add_state(17);
            }
            0xaa => {
                self.lda(self.lh5801.t);
                self.check_z(self.lh5801.t);
                self.add_state(9);
            }
            0xab => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                self.ora(read);
                self.add_state(17);
            }
            0xac => {
                let read = self.cpu_readmem(Self::me1(self.lh5801.u));
                self.dca(read);
                self.add_state(19);
            }
            0xad => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                self.eor(read);
                self.add_state(17);
            }
            0xae => {
                let op = self.readop_word();
                self.cpu_writemem(Self::me1(op), self.lh5801.a);
                self.add_state(16);
            }
            0xaf => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                self.bit(read, self.lh5801.a);
                self.add_state(17);
            }
            0xb1 => {
                self.lh5801.is_halted = true;
                self.add_state(8);
            }
            // Not in the official documentation, nor used in rom
            // 0xb8 => {
            //     self.push_word((self.lh5801.sh() as u16) << 8);
            //     self.add_state(14);
            // }
            0xba => {
                self.ita();
                self.add_state(9);
            }
            0xbe => {
                self.set_ie_flag(false);
                self.add_state(8);
            }
            0xc0 => {
                self.lh5801.disp = false;
                self.add_state(8);
            }
            0xc1 => {
                self.lh5801.disp = true;
                self.add_state(8);
            }
            0xc8 => {
                self.push(self.lh5801.a);
                self.add_state(11);
            }
            0xca => {
                self.lh5801.x = self.adr(self.lh5801.x);
                self.add_state(11);
            }
            0xcc => {
                self.add_state(9);
            }
            0xce => {
                self.am(u16::from(self.lh5801.a));
                self.add_state(9);
            }
            0xd3 => {
                self.drr(Self::me1(self.lh5801.x()));
                self.add_state(16);
            }
            0xd7 => {
                self.drl(Self::me1(self.lh5801.x()));
                self.add_state(16);
            }
            0xda => {
                self.lh5801.y = self.adr(self.lh5801.y);
                self.add_state(11);
            }
            0xde => {
                self.am(u16::from(self.lh5801.a) | 0x100);
                self.add_state(9);
            }
            0xea => {
                self.lh5801.u = self.adr(self.lh5801.u);
                self.add_state(11);
            }
            0xe9 => {
                let adr = Self::me1(self.readop_word());
                let read = self.cpu_readop();
                self.and_mem(adr, read);
                self.add_state(23);
            }
            0xeb => {
                let adr = Self::me1(self.readop_word());
                let read = self.cpu_readop();
                self.ora_mem(adr, read);
                self.add_state(23);
            }
            0xec => {
                self.lh5801.t = self.lh5801.a & 0x1F;
                self.add_state(9);
            }
            0xed => {
                let op = self.readop_word();
                let read = self.cpu_readmem(Self::me1(op));
                let op2 = self.cpu_readop();
                self.bit(read, op2);
                self.add_state(20);
            }
            0xef => {
                let op = self.readop_word();
                let adr = Self::me1(op);
                let op2 = self.cpu_readop();
                self.add_mem(adr, op2);
                self.add_state(23);
            }
            _ => {
                panic!("Illegal fd opcode: {:02X} at {:04X}", oper, self.lh5801.p);
            }
        }
    }

    fn instruction(&mut self) {
        let oper = self.cpu_readop();

        if self.lh5801.print_insts {
            println!(
                "instruction: {:02X} at addr: {:04X}. A = {:02X}",
                oper,
                self.lh5801.p - 1,
                self.lh5801.a
            );
        }

        match oper {
            0x00 => {
                self.sbc(self.lh5801.xl());
                self.add_state(6);
            }
            0x01 => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.sbc(val);
                self.add_state(7);
            }
            0x02 => {
                self.adc(self.lh5801.xl());
                self.add_state(6);
            }
            0x03 => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.adc(val);
                self.add_state(7);
            }
            0x04 => {
                self.lda(self.lh5801.xl());
                self.add_state(5);
            }
            0x05 => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.lda(val);
                self.add_state(6);
            }
            0x06 => {
                self.cpa(self.lh5801.a, self.lh5801.xl());
                self.add_state(6);
            }
            0x07 => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.cpa(self.lh5801.a, val);
                self.add_state(7);
            }
            0x08 => {
                self.lh5801.set_xh(self.lh5801.a);
                self.add_state(5);
            }
            0x09 => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.and(val);
                self.add_state(7);
            }
            0x0a => {
                self.lh5801.set_xl(self.lh5801.a);
                self.add_state(5);
            }
            0x0b => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.ora(val);
                self.add_state(7);
            }
            0x0c => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.dcs(val);
                self.add_state(13);
            }
            0x0d => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.eor(val);
                self.add_state(7);
            }
            0x0e => {
                self.cpu_writemem(self.lh5801.x(), self.lh5801.a);
                self.add_state(6);
            }
            0x0f => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.bit(val, self.lh5801.a);
                self.add_state(7);
            }

            0x10 => {
                self.sbc(self.lh5801.yl());
                self.add_state(6);
            }
            0x11 => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.sbc(val);
                self.add_state(7);
            }
            0x12 => {
                self.adc(self.lh5801.yl());
                self.add_state(6);
            }
            0x13 => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.adc(val);
                self.add_state(7);
            }
            0x14 => {
                self.lda(self.lh5801.yl());
                self.add_state(5);
            }
            0x15 => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.lda(val);
                self.add_state(6);
            }
            0x16 => {
                self.cpa(self.lh5801.a, self.lh5801.yl());
                self.add_state(6);
            }
            0x17 => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.cpa(self.lh5801.a, val);
                self.add_state(7);
            }
            0x18 => {
                self.lh5801.set_yh(self.lh5801.a);
                self.add_state(5);
            }
            0x19 => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.and(val);
                self.add_state(7);
            }
            0x1a => {
                self.lh5801.set_yl(self.lh5801.a);
                self.add_state(5);
            }
            0x1b => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.ora(val);
                self.add_state(7);
            }
            0x1c => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.dcs(val);
                self.add_state(13);
            }
            0x1d => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.eor(val);
                self.add_state(7);
            }
            0x1e => {
                self.cpu_writemem(self.lh5801.y(), self.lh5801.a);
                self.add_state(6);
            }
            0x1f => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.bit(val, self.lh5801.a);
                self.add_state(7);
            }

            0x20 => {
                self.sbc(self.lh5801.ul());
                self.add_state(6);
            }
            0x21 => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.sbc(val);
                self.add_state(7);
            }
            0x22 => {
                self.adc(self.lh5801.ul());
                self.add_state(6);
            }
            0x23 => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.adc(val);
                self.add_state(7);
            }
            0x24 => {
                self.lda(self.lh5801.ul());
                self.add_state(5);
            }
            0x25 => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.lda(val);
                self.add_state(6);
            }
            0x26 => {
                self.cpa(self.lh5801.a, self.lh5801.ul());
                self.add_state(6);
            }
            0x27 => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.cpa(self.lh5801.a, val);
                self.add_state(7);
            }
            0x28 => {
                self.lh5801.set_uh(self.lh5801.a);
                self.add_state(5);
            }
            0x29 => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.and(val);
                self.add_state(7);
            }
            0x2a => {
                self.lh5801.set_ul(self.lh5801.a);
                self.add_state(5);
            }
            0x2b => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.ora(val);
                self.add_state(7);
            }
            0x2c => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.dcs(val);
                self.add_state(13);
            }
            0x2d => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.eor(val);
                self.add_state(7);
            }
            0x2e => {
                self.cpu_writemem(self.lh5801.u(), self.lh5801.a);
                self.add_state(6);
            }
            0x2f => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.bit(val, self.lh5801.a);
                self.add_state(7);
            }

            0x30 => {
                self.sbc(0);
                self.add_state(6);
            }
            0x32 => {
                self.adc(0);
                self.add_state(6);
            }
            0x34 => {
                self.lda(0);
                self.add_state(5);
            }
            0x36 => {
                self.cpa(self.lh5801.a, 0);
                self.add_state(6);
            }
            0x38 => {
                self.add_state(5);
            }

            0x40 => {
                let inc = self.inc(self.lh5801.xl());
                self.lh5801.set_xl(inc);
                self.add_state(5);
            }
            0x41 => {
                self.lh5801.x = self.sin(self.lh5801.x);
                self.add_state(6);
            }
            0x42 => {
                let dec = self.dec(self.lh5801.xl());
                self.lh5801.set_xl(dec);
                self.add_state(5);
            }
            0x43 => {
                self.lh5801.x = self.sde(self.lh5801.x);
                self.add_state(6);
            }
            0x44 => {
                self.lh5801.x = self.lh5801.x.wrapping_add(1);
                self.add_state(5);
            }
            0x45 => {
                self.lh5801.x = self.lin(self.lh5801.x);
                self.add_state(6);
            }
            0x46 => {
                self.lh5801.x = self.lh5801.x.wrapping_sub(1);
                self.add_state(5);
            }
            0x47 => {
                self.lh5801.x = self.lde(self.lh5801.x);
                self.add_state(6);
            }
            0x48 => {
                let val = self.cpu_readop();
                self.lh5801.set_xh(val);
                self.add_state(6);
            }
            0x49 => {
                let val = self.cpu_readop();
                self.and_mem(self.lh5801.x(), val);
                self.add_state(13);
            }
            0x4a => {
                let val = self.cpu_readop();
                self.lh5801.set_xl(val);
                self.add_state(6);
            }
            0x4b => {
                let val = self.cpu_readop();
                self.ora_mem(self.lh5801.x(), val);
                self.add_state(13);
            }
            0x4c => {
                let val = self.cpu_readop();
                self.cpa(self.lh5801.xh(), val);
                self.add_state(7);
            }
            0x4d => {
                let mem = self.cpu_readmem(self.lh5801.x());
                let val = self.cpu_readop();
                self.bit(mem, val);
                self.add_state(10);
            }
            0x4e => {
                let val = self.cpu_readop();
                self.cpa(self.lh5801.xl(), val);
                self.add_state(7);
            }
            0x4f => {
                let val = self.cpu_readop();
                self.add_mem(self.lh5801.x(), val);
                self.add_state(13);
            }

            0x50 => {
                let inc = self.inc(self.lh5801.yl());
                self.lh5801.set_yl(inc);
                self.add_state(5);
            }
            0x51 => {
                self.lh5801.y = self.sin(self.lh5801.y);
                self.add_state(6);
            }
            0x52 => {
                let dec = self.dec(self.lh5801.yl());
                self.lh5801.set_yl(dec);
                self.add_state(5);
            }
            0x53 => {
                self.lh5801.y = self.sde(self.lh5801.y);
                self.add_state(6);
            }
            0x54 => {
                self.lh5801.y = self.lh5801.y.wrapping_add(1);
                self.add_state(5);
            }
            0x55 => {
                self.lh5801.y = self.lin(self.lh5801.y);
                self.add_state(6);
            }
            0x56 => {
                self.lh5801.y = self.lh5801.y.wrapping_sub(1);
                self.add_state(5);
            }
            0x57 => {
                self.lh5801.y = self.lde(self.lh5801.y);
                self.add_state(6);
            }
            0x58 => {
                let val = self.cpu_readop();
                self.lh5801.set_yh(val);
                self.add_state(6);
            }
            0x59 => {
                let val = self.cpu_readop();
                self.and_mem(self.lh5801.y(), val);
                self.add_state(13);
            }
            0x5a => {
                let val = self.cpu_readop();
                self.lh5801.set_yl(val);
                self.add_state(6);
            }
            0x5b => {
                let val = self.cpu_readop();
                self.ora_mem(self.lh5801.y(), val);
                self.add_state(13);
            }
            0x5c => {
                let val = self.cpu_readop();
                self.cpa(self.lh5801.yh(), val);
                self.add_state(7);
            }
            0x5d => {
                let mem = self.cpu_readmem(self.lh5801.y());
                let val = self.cpu_readop();
                self.bit(mem, val);
                self.add_state(10);
            }
            0x5e => {
                let val = self.cpu_readop();
                self.cpa(self.lh5801.yl(), val);
                self.add_state(7);
            }
            0x5f => {
                let val = self.cpu_readop();
                self.add_mem(self.lh5801.y(), val);
                self.add_state(13);
            }

            0x60 => {
                let inc = self.inc(self.lh5801.ul());
                self.lh5801.set_ul(inc);
                self.add_state(5);
            }
            0x61 => {
                self.lh5801.u = self.sin(self.lh5801.u);
                self.add_state(6);
            }
            0x62 => {
                let dec = self.dec(self.lh5801.ul());
                self.lh5801.set_ul(dec);
                self.add_state(5);
            }
            0x63 => {
                self.lh5801.u = self.sde(self.lh5801.u);
                self.add_state(6);
            }
            0x64 => {
                self.lh5801.u = self.lh5801.u.wrapping_add(1);
                self.add_state(5);
            }
            0x65 => {
                self.lh5801.u = self.lin(self.lh5801.u);
                self.add_state(6);
            }
            0x66 => {
                self.lh5801.u = self.lh5801.u.wrapping_sub(1);
                self.add_state(5);
            }
            0x67 => {
                self.lh5801.u = self.lde(self.lh5801.u);
                self.add_state(6);
            }
            0x68 => {
                let val = self.cpu_readop();
                self.lh5801.set_uh(val);
                self.add_state(6);
            }
            0x69 => {
                let val = self.cpu_readop();
                self.and_mem(self.lh5801.u(), val);
                self.add_state(13);
            }
            0x6a => {
                let val = self.cpu_readop();
                self.lh5801.set_ul(val);
                self.add_state(6);
            }
            0x6b => {
                let val = self.cpu_readop();
                self.ora_mem(self.lh5801.u(), val);
                self.add_state(13);
            }
            0x6c => {
                let val = self.cpu_readop();
                self.cpa(self.lh5801.uh(), val);
                self.add_state(7);
            }
            0x6d => {
                let mem = self.cpu_readmem(self.lh5801.u());
                let val = self.cpu_readop();
                self.bit(mem, val);
                self.add_state(10);
            }
            0x6e => {
                let val = self.cpu_readop();
                self.cpa(self.lh5801.ul(), val);
                self.add_state(7);
            }
            0x6f => {
                let val = self.cpu_readop();
                self.add_mem(self.lh5801.u(), val);
                self.add_state(13);
            }

            0x80 => {
                self.sbc(self.lh5801.xh());
                self.add_state(6);
            }
            0x81 => {
                self.branch_plus(!self.get_carry_flag());
                self.add_state(8);
            }
            0x82 => {
                self.adc(self.lh5801.xh());
                self.add_state(6);
            }
            0x83 => {
                self.branch_plus(self.get_carry_flag());
                self.add_state(8);
            }
            0x84 => {
                self.lda(self.lh5801.xh());
                self.add_state(5);
            }
            0x85 => {
                self.branch_plus(!self.get_half_carry_flag());
                self.add_state(8);
            }
            0x86 => {
                self.cpa(self.lh5801.a, self.lh5801.xh());
                self.add_state(6);
            }
            0x87 => {
                self.branch_plus(self.get_half_carry_flag());
                self.add_state(8);
            }
            0x88 => {
                self.lop();
            }
            0x89 => {
                self.branch_plus(!self.get_zero_flag());
                self.add_state(8);
            }
            0x8a => {
                self.rti();
                self.add_state(14);
            }
            0x8b => {
                self.branch_plus(self.get_zero_flag());
                self.add_state(8);
            }
            0x8c => {
                let val = self.cpu_readmem(self.lh5801.x());
                self.dca(val);
                self.add_state(15);
            }
            0x8d => {
                self.branch_plus(!self.get_overflow_flag());
                self.add_state(8);
            }
            0x8e => {
                self.branch_plus(true);
                self.add_state(6);
            }
            0x8f => {
                self.branch_plus(self.get_overflow_flag());
                self.add_state(8);
            }

            0x90 => {
                self.sbc(self.lh5801.yh());
                self.add_state(6);
            }
            0x91 => {
                self.branch_minus(!self.get_carry_flag());
                self.add_state(8);
            }
            0x92 => {
                self.adc(self.lh5801.yh());
                self.add_state(6);
            }
            0x93 => {
                self.branch_minus(self.get_carry_flag());
                self.add_state(8);
            }
            0x94 => {
                self.lda(self.lh5801.yh());
                self.add_state(5);
            }
            0x95 => {
                self.branch_minus(!self.get_half_carry_flag());
                self.add_state(8);
            }
            0x96 => {
                self.cpa(self.lh5801.a, self.lh5801.yh());
                self.add_state(6);
            }
            0x97 => {
                self.branch_minus(self.get_half_carry_flag());
                self.add_state(8);
            }
            0x99 => {
                self.branch_minus(!self.get_zero_flag());
                self.add_state(8);
            }
            0x9a => {
                self.rtn();
                self.add_state(11);
            }
            0x9b => {
                self.branch_minus(self.get_zero_flag());
                self.add_state(8);
            }
            0x9c => {
                let val = self.cpu_readmem(self.lh5801.y());
                self.dca(val);
                self.add_state(15);
            }
            0x9d => {
                self.branch_minus(!self.get_overflow_flag());
                self.add_state(8);
            }
            0x9e => {
                self.branch_minus(true);
                self.add_state(6);
            }
            0x9f => {
                self.branch_minus(self.get_overflow_flag());
                self.add_state(8);
            }

            0xa0 => {
                self.sbc(self.lh5801.uh());
                self.add_state(6);
            }
            0xa1 => {
                let addr = self.readop_word();
                let val = self.cpu_readmem(addr);
                self.sbc(val);
                self.add_state(13);
            }
            0xa2 => {
                self.adc(self.lh5801.uh());
                self.add_state(6);
            }
            0xa3 => {
                let addr = self.readop_word();
                let val = self.cpu_readmem(addr);
                self.adc(val);
                self.add_state(13);
            }
            0xa4 => {
                self.lda(self.lh5801.uh());
                self.add_state(5);
            }
            0xa5 => {
                let addr = self.readop_word();
                let val = self.cpu_readmem(addr);
                self.lda(val);
                self.add_state(12);
            }
            0xa6 => {
                self.cpa(self.lh5801.a, self.lh5801.uh());
                self.add_state(6);
            }
            0xa7 => {
                let addr = self.readop_word();
                let val = self.cpu_readmem(addr);
                self.cpa(self.lh5801.a, val);
                self.add_state(13);
            }
            0xa8 => {
                self.lh5801.pv = true;
                self.add_state(4);
            }
            0xa9 => {
                let addr = self.readop_word();
                let val = self.cpu_readmem(addr);
                self.and(val);
                self.add_state(13);
            }
            0xaa => {
                let addr = self.readop_word();
                self.lh5801.s = addr;
                self.add_state(12);
            }
            0xab => {
                let addr = self.readop_word();
                let val = self.cpu_readmem(addr);
                self.ora(val);
                self.add_state(13);
            }
            0xac => {
                let val = self.cpu_readmem(self.lh5801.u());
                self.dca(val);
                self.add_state(15);
            }
            0xad => {
                let addr = self.readop_word();
                let val = self.cpu_readmem(addr);
                self.eor(val);
                self.add_state(13);
            }
            0xae => {
                let addr = self.readop_word();
                self.cpu_writemem(addr, self.lh5801.a);
                self.add_state(12);
            }
            0xaf => {
                let addr = self.readop_word();
                let val = self.cpu_readmem(addr);
                self.bit(val, self.lh5801.a);
                self.add_state(13);
            }

            0xb1 => {
                let val = self.cpu_readop();
                self.sbc(val);
                self.add_state(7);
            }
            0xb3 => {
                let val = self.cpu_readop();
                self.adc(val);
                self.add_state(7);
            }
            0xb5 => {
                let val = self.cpu_readop();
                self.lda(val);
                self.add_state(6);
            }
            0xb7 => {
                let val = self.cpu_readop();
                self.cpa(self.lh5801.a, val);
                self.add_state(7);
            }
            0xb8 => {
                self.lh5801.pv = false;
                self.add_state(4);
            }
            0xb9 => {
                let val = self.cpu_readop();
                self.and(val);
                self.add_state(7);
            }
            0xba => {
                let addr = self.readop_word();
                self.jmp(addr);
                self.add_state(12);
            }
            0xbb => {
                let val = self.cpu_readop();
                self.ora(val);
                self.add_state(7);
            }
            0xbd => {
                let val = self.cpu_readop();
                self.eor(val);
                self.add_state(7);
            }
            0xbe => {
                self.sjp();
                self.add_state(19);
            }
            0xbf => {
                let val = self.cpu_readop();
                self.bit(self.lh5801.a, val);
                self.add_state(7);
            }

            0xc1 => {
                let nr = self.cpu_readop();
                self.vector(!self.get_carry_flag(), nr);
                self.add_state(8);
            }
            0xc3 => {
                let nr = self.cpu_readop();
                self.vector(self.get_carry_flag(), nr);
                self.add_state(8);
            }
            0xc5 => {
                let nr = self.cpu_readop();
                self.vector(!self.get_half_carry_flag(), nr);
                self.add_state(8);
            }
            0xc7 => {
                let nr = self.cpu_readop();
                self.vector(self.get_half_carry_flag(), nr);
                self.add_state(8);
            }
            0xc9 => {
                let nr = self.cpu_readop();
                self.vector(!self.get_zero_flag(), nr);
                self.add_state(8);
            }
            0xcb => {
                let nr = self.cpu_readop();
                self.vector(self.get_zero_flag(), nr);
                self.add_state(8);
            }
            0xcd => {
                let nr = self.cpu_readop();
                self.vector(true, nr);
                self.add_state(7);
            }
            0xcf => {
                let nr = self.cpu_readop();
                self.vector(self.get_overflow_flag(), nr);
                self.add_state(8);
            }

            0xd1 => {
                self.ror();
                self.add_state(9);
            }
            0xd3 => {
                self.drr(self.lh5801.x());
                self.add_state(12);
            }
            0xd5 => {
                self.shr();
                self.add_state(9);
            }
            0xd7 => {
                self.drl(self.lh5801.x());
                self.add_state(12);
            }
            0xd9 => {
                self.shl();
                self.add_state(6);
            }
            0xdb => {
                self.rol();
                self.add_state(8);
            }
            0xdd => {
                self.lh5801.a = self.inc(self.lh5801.a);
                self.add_state(5);
            }
            0xdf => {
                self.lh5801.a = self.dec(self.lh5801.a);
                self.add_state(5);
            }

            0xe1 => {
                self.lh5801.pu = true;
                self.add_state(4);
            }
            0xe3 => {
                self.lh5801.pu = false;
                self.add_state(4);
            }
            0xe9 => {
                let addr = self.readop_word();
                let val = self.cpu_readop();
                self.and_mem(addr, val);
                self.add_state(19);
            }
            0xeb => {
                let addr = self.readop_word();
                let val = self.cpu_readop();
                self.ora_mem(addr, val);
                self.add_state(19);
            }
            0xed => {
                let addr = self.readop_word();
                let mem = self.cpu_readmem(addr);
                let val = self.cpu_readop();
                self.bit(mem, val);
                self.add_state(16);
            }
            0xef => {
                let addr = self.readop_word();
                let val = self.cpu_readop();
                self.add_mem(addr, val);
                self.add_state(19);
            }

            0xf1 => {
                self.aex();
                self.add_state(6);
            }
            0xf5 => {
                let val = self.cpu_readmem(self.lh5801.x);
                self.lh5801.x = self.lh5801.x.wrapping_add(1);
                self.cpu_writemem(self.lh5801.y, val);
                self.lh5801.y = self.lh5801.y.wrapping_add(1);
                self.add_state(7);
            }
            0xf7 => {
                let val = self.cpu_readmem(self.lh5801.x);
                self.lh5801.x = self.lh5801.x.wrapping_add(1);
                self.cpa(self.lh5801.a, val);
                self.add_state(7);
            }
            0xf9 => {
                self.set_carry_flag(false);
                self.add_state(4);
            }
            0xfb => {
                self.set_carry_flag(true);
                self.add_state(4);
            }
            0xfd => {
                self.instruction_fd();
            }

            0xc0 | 0xc2 | 0xc4 | 0xc6 | 0xc8 | 0xca | 0xcc | 0xce | 0xd0 | 0xd2 | 0xd4 | 0xd6
            | 0xd8 | 0xda | 0xdc | 0xde | 0xe0 | 0xe2 | 0xe4 | 0xe6 | 0xe8 | 0xea | 0xec | 0xee
            | 0xf0 | 0xf2 | 0xf4 | 0xf6 => {
                self.vector(true, oper);
                self.add_state(4);
            }

            _ => {
                panic!(
                    "Illegal opcode: 0x{:02x} at PC: 0x{:04x}",
                    oper,
                    self.lh5801.p.wrapping_sub(1)
                );
            }
        }
    }
}
