use crate::memory::MemoryBus;

const CF: u8 = 0x01; // C: Carry flag
const IE: u8 = 0x02; // IE: Interrupt enable flag  
const ZF: u8 = 0x04; // Z: Zero flag
const VF: u8 = 0x08; // V: Overflow flag
const HF: u8 = 0x10; // H: Half carry flag

#[derive(Debug, Default)]
pub struct Lh5801Cpu {
    a: u8,  // Accumulator
    t: u8,  // T register (contains flags: C, IE, Z, V, H)
    x: u16, // X index register
    y: u16, // Y index register
    u: u16, // U pointer register
    s: u16, // Stack pointer
    p: u16, // Program counter

    // CPU state
    is_halted: bool,

    pu: bool,
    pv: bool,
    bf: bool,
    disp: bool,
    tm: u16, // 9-bit timer register

    ir0: bool, // Non-maskable interrupt (connected to ground in PC-1500)
    ir1: bool, // Timer interrupt
    ir2: bool, // Maskable interrupt

    reset_flag: bool,
    is_timer_reached: bool,

    timer_state: usize, // Timer tick counter
    step_previous_state: usize,
}

impl Lh5801Cpu {
    pub fn new(memory: &mut MemoryBus) -> Self {
        let mut ret = Self::default();
        ret.internal_reset(memory);
        ret
    }

    // UINT32	CLH5801::get_mem(UINT32 adr,int size)
    // {
    // 	switch(size)
    // 	{
    //     case 8:
    // 	case SIZE_8 :return( cpu_readmem(adr));
    //     case 16:
    //     case SIZE_16:return( cpu_readmem(adr+1)+(cpu_readmem(adr)<<8));
    //     case 20:
    //     case SIZE_20:return((cpu_readmem(adr+2)+(cpu_readmem(adr+1)<<8)+(cpu_readmem(adr)<<16))&MASK_20);
    //     case 24:
    //     case SIZE_24:return((cpu_readmem(adr+2)+(cpu_readmem(adr+1)<<8)+(cpu_readmem(adr)<<16))&MASK_24);
    // 	}
    // 	return(0);
    // }
    fn get_mem16(memory: &mut MemoryBus, addr: u16) -> u16 {
        (memory.read_byte(addr.wrapping_add(1)) as u16) | ((memory.read_byte(addr) as u16) << 8)
    }

    // void CLH5801::Reset(void)
    // {
    //     resetFlag = true;
    // }
    fn reset(&mut self) {
        self.reset_flag = true;
    }

    // void CLH5801::internalReset(void)
    // {
    //     resetFlag = true;
    //     memset(imem,0,imemsize);
    //     P	= (UINT16) get_mem(0xFFFE,SIZE_16);
    //     lh5801.HLT=lh5801.IR0=lh5801.IR1=lh5801.IR2=0;
    //     S	= 0;
    //     U	= 0;
    //     UL	= 0;
    //     UH	= 0;
    //     X	= 0;
    //     XL	= 0;
    //     XH	= 0;
    //     Y	= 0;
    //     YL	= 0;
    //     YH	= 0;
    //     lh5801.tm=0; //9 bit
    //     lh5801.t=lh5801.a=lh5801.dp=lh5801.pu=lh5801.pv=0;
    //     lh5801.bf=1;
    //     CallSubLevel = 0;

    //     resetFlag = false;
    // }
    // FIXME: wrong
    fn internal_reset(&mut self, memory: &mut MemoryBus) {
        self.reset_flag = true;
        self.p = Self::get_mem16(memory, 0xFFFE);
        self.a = 0;
        self.t = 0;
        self.x = 0;
        self.y = 0;
        self.u = 0;
        self.s = 0;
        self.p = 0;
        self.is_halted = false;
        self.ir0 = false;
        self.ir1 = false;
        self.ir2 = false;
        self.timer_state = 0;
        self.bf = true;

        self.reset_flag = false;
    }

    // void CLH5801::step(void)
    // {

    //     quint64	Current_State;

    //     if (resetFlag) internalReset();

    //     if (Is_Timer_Reached) { lh5801.IR1=1; Is_Timer_Reached = false; }

    // 	if (lh5801.IR0)
    // 	{
    // 		// Non-maskable Interrupt processing
    // 		// NOT USED - Connected to Ground
    // 	}
    // 	else
    // 	if ( (lh5801.IR1) && F_IE )
    // 	{
    // 		// Timer Interrupt Routine
    // 		PUSH(lh5801.t);
    // 		UNSET_IE;
    // 		lh5801.IR1=0;
    // 		PUSH_WORD(P);
    // 		P = (UINT16) get_mem(0xFFFA,SIZE_16);
    //         CallSubLevel++;

    // 	}
    // 	else
    // 	if ( (lh5801.IR2) && F_IE )
    // 	{

    // 		// Maskable Interrupt processing
    // 		PUSH(lh5801.t);
    // 		UNSET_IE;
    //         lh5801.HLT = false;
    // 		lh5801.IR2=0;
    // 		PUSH_WORD(P);
    // 		P = (UINT16) get_mem(0xFFF8,SIZE_16);
    //         CallSubLevel++;

    // 	}
    // 	else
    // 	if (lh5801.HLT)
    // 	{
    // 		// Do nothing
    //         AddState(2);
    // 	}
    // 	else
    // 	{
    // 		instruction();
    // 	}

    // #define TIMER_FREQUENCY 31250
    // #define NB_STATE_PER_TIMER	42

    // 	// INCREMENT TIMER
    // 	Current_State = pPC->pTIMER->state;

    // 	if ((Current_State - step_Previous_State) >= 42)
    // 	{
    // 		TIMER_INC();
    // 		step_Previous_State += (Current_State - step_Previous_State);
    // 	}

    // }
    pub fn step(&mut self, memory: &mut MemoryBus) {
        if self.reset_flag {
            self.internal_reset(memory);
        }

        if self.is_timer_reached {
            self.ir1 = true; // Timer interrupt
            self.is_timer_reached = false;
        }

        if self.ir0 {
            // Non-maskable Interrupt processing
            // NOT USED - Connected to Ground
        } else if self.ir1 && self.interrupt_enabled() {
            self.push(memory, self.t);
            self.set_flag(IE, false); // Unset interrupt enable flag
            self.ir1 = false;
            self.push_word(memory, self.p);
            self.p = Self::get_mem16(memory, 0xFFFA);
        } else if self.ir2 && self.interrupt_enabled() {
            // Maskable Interrupt processing
            self.push(memory, self.t);
            self.set_flag(IE, false); // Unset interrupt enable flag
            self.ir2 = false;
            self.push_word(memory, self.p);
            self.p = Self::get_mem16(memory, 0xFFF8);
        } else if self.is_halted {
            // Do nothing
            self.add_state(2);
        } else {
            self.instruction(memory);
        }

        let current_state = self.timer_state;

        if current_state - self.step_previous_state >= 42 {
            self.timer_inc();
            self.step_previous_state += current_state - self.step_previous_state;
        }
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
    pub const fn interrupt_enabled(&self) -> bool {
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

    fn cpu_readmem(&mut self, memory: &mut MemoryBus, addr: u16) -> u8 {
        memory.read_byte(addr)
    }

    fn cpu_writemem(&mut self, memory: &mut MemoryBus, addr: u16, val: u8) {
        memory.write_byte(addr, val);
    }

    // INLINE UINT8 CLH5801::cpu_readop(UINT32 adr)
    // {
    //     return (pPC->Get_8(adr));
    // }
    fn cpu_readop(&mut self, memory: &mut MemoryBus, addr: u16) -> u8 {
        memory.read_byte(addr)
    }

    // INLINE UINT16 CLH5801::readop_word(void)
    // {
    // 	return (UINT16) ((cpu_readop(P++) << 8) | cpu_readop(P++));
    // }
    // FIXME: is this truly big-endian?
    fn readop_word(&mut self, memory: &mut MemoryBus) -> u16 {
        let hi = self.cpu_readop(memory, self.p) as u16;
        self.p = self.p.wrapping_add(1);
        let lo = self.cpu_readop(memory, self.p) as u16;
        self.p = self.p.wrapping_add(1);
        (hi << 8) | lo
    }

    // #define ME1(a)		((a)|0x10000)

    const fn me1(&self, addr: u16) -> u16 {
        addr | 0x10000
    }

    // === FLAG OPERATIONS ===

    const fn get_flag(&self, flag: u8) -> bool {
        self.t & flag != 0
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.t |= flag;
        } else {
            self.t &= !flag;
        }
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

    // #define CHECK_Z(a)	{ ( !(a)? SET_Z : UNSET_Z);	}

    fn check_z<I: Into<i16>>(&mut self, val: I) {
        self.set_zero_flag(val.into() == 0);
    }

    // #define CHECK_C(a)	{ ( (a) ? SET_C : UNSET_C);	}

    fn check_c<I: Into<i16>>(&mut self, val: I) {
        self.set_carry_flag(val.into() != 0);
    }

    // #define CHECK_H(a)	{ ( (a) ? SET_H : UNSET_H);	}

    fn check_h<I: Into<i16>>(&mut self, val: I) {
        self.set_half_carry_flag(val.into() != 0);
    }

    // #define CHECK_V(a)	{ ( (a) ? SET_V : UNSET_V);	}

    fn check_v<I: Into<i16>>(&mut self, val: I) {
        self.set_overflow_flag(val.into() != 0);
    }

    // INLINE void CLH5801::TIMER_INC(void)
    // {
    // 	// Shift right , b9=(b0 xor b4)
    // 	lh5801.tm = (lh5801.tm >> 1) | (( (lh5801.tm & 0x01) ^ ((lh5801.tm & 0x10)>>4) )<<8 );

    //     Is_Timer_Reached=(lh5801.tm == 0x1FF ? true : false);
    // }
    fn timer_inc(&mut self) {
        // Shift right , b9=(b0 xor b4)
        self.tm = (self.tm >> 1) | (((self.tm & 0x01) ^ ((self.tm & 0x10) >> 4)) << 8);
        self.is_timer_reached = self.tm == 0x1FF;
    }

    // INLINE void CLH5801::PUSH(UINT8 data)
    // {
    // 	cpu_writemem(S--, data);
    // }

    fn push(&mut self, memory: &mut MemoryBus, value: u8) {
        self.cpu_writemem(memory, self.s, value);
        self.s = self.s.wrapping_sub(1);
    }

    // INLINE void CLH5801::PUSH_WORD(UINT16 data)
    // {
    // 	PUSH( (UINT8) (data & 0xff));
    // 	PUSH( (UINT8) (data >> 8));
    // }

    fn push_word(&mut self, memory: &mut MemoryBus, value: u16) {
        self.push(memory, (value & 0xFF) as u8);
        self.push(memory, (value >> 8) as u8);
    }

    // INLINE void CLH5801::POP(void)
    // {
    // 	lh5801.a = cpu_readmem(++S);
    // 	CHECK_Z(lh5801.a);
    // }

    fn pop(&mut self, memory: &mut MemoryBus) {
        self.s = self.s.wrapping_add(1);
        self.a = self.cpu_readmem(memory, self.s);
        self.check_z(self.a);
    }

    // INLINE void CLH5801::POP_WORD(PAIR *reg)
    // {
    // 	reg->b.h = cpu_readmem(++S);
    // 	reg->b.l = cpu_readmem(++S);
    // }

    fn pop_word(&mut self, memory: &mut MemoryBus) -> u16 {
        self.s = self.s.wrapping_add(1);
        let hi = u16::from(self.cpu_readmem(memory, self.s));
        self.s = self.s.wrapping_add(1);
        let lo = u16::from(self.cpu_readmem(memory, self.s));
        (hi << 8) | lo
    }

    // INLINE UINT8 CLH5801::add_generic(int left, int right, int carry)
    // {
    // 	int res = left + right + carry;
    // 	int v,c;

    // 	lh5801.t&=~(H|V|Z|C);

    // 	CHECK_Z(res & 0xff);

    // 	c = res & 0x100;
    // 	CHECK_C(c);

    // 	if (((left & 0x0f)+(right & 0x0f) + carry) & 0x10) SET_H;
    // 	v = ((left & 0x7f)+(right & 0x7f) + carry) & 0x80;
    // 	if ( (c && !v)||(!c && v) ) SET_V;

    // 	return (UINT8) (res);
    // }

    fn add_generic<I: Into<i16>>(&mut self, left: I, right: I, carry: bool) -> u8 {
        let left = left.into();
        let right = right.into();
        let carry_i16 = if carry { 1 } else { 0 };
        let res = left + right + carry_i16;

        // Clear affected flags first (H|V|Z|C in C++ code)
        self.t &= !(HF | VF | ZF | CF);

        self.check_z(res & 0xff);

        // Check carry flag
        let c = res & 0x100;
        self.check_c(c);

        // Check half carry flag
        if ((left & 0x0f) + (right & 0x0f) + carry_i16) & 0x10 != 0 {
            self.set_half_carry_flag(true);
        }

        // Check overflow flag
        let v = ((left & 0x7f) + (right & 0x7f) + carry_i16) & 0x80;
        if (c != 0 && v == 0) || (c == 0 && v != 0) {
            self.set_overflow_flag(true);
        }

        res as u8
    }

    // INLINE void CLH5801::ADC(UINT8 data)
    // {
    // 	lh5801.a = add_generic(lh5801.a,data,bool(F_C));
    // }

    fn adc(&mut self, data: u8) {
        self.a = self.add_generic(self.a, data, self.get_carry_flag());
    }

    // INLINE void CLH5801::ADD_MEM(UINT32 addr, UINT8 data)
    // {
    // 	UINT8 v = add_generic(cpu_readmem(addr),data,0);
    // 	cpu_writemem(addr,v);
    // }

    fn add_mem(&mut self, memory: &mut MemoryBus, addr: u16, data: u8) {
        let mem_read = self.cpu_readmem(memory, addr);
        let v = self.add_generic(mem_read, data, false);
        self.cpu_writemem(memory, addr, v);
    }

    // INLINE void CLH5801::ADR(PAIR *reg)
    // {
    // 	UINT8 loc_t = lh5801.t;		// Record Flags

    // 	reg->b.l = add_generic(reg->b.l,lh5801.a,0);
    // 	if (F_C) {
    // 		reg->b.h++;
    // 	}
    // 	lh5801.t = loc_t;		// Restore Flags : OFFICIAL DOCUMENTATION IS WRONG Flags are not impacted
    // }

    fn adr(&mut self, reg: u16) -> u16 {
        let loc_t = self.t; // Record Flags
        let rl = (reg & 0xFF) as u8;
        let ret = (reg & 0xFF00) | (self.add_generic(rl, self.a, false)) as u16;

        self.t = loc_t; // Restore Flags: OFFICIAL DOCUMENTATION IS WRONG, flags are not impacted
        ret
    }

    // INLINE void CLH5801::SBC(UINT8 data)
    // {
    // 	lh5801.a = add_generic(lh5801.a,data ^ 0xff,bool(F_C));
    // }

    fn sbc(&mut self, data: u8) {
        self.a = self.add_generic(self.a, data ^ 0xff, self.get_carry_flag());
    }

    // INLINE void CLH5801::CPA(UINT8 a, UINT8 b)
    // {
    // 	add_generic(a, b ^ 0xff, 1);
    // }

    fn cpa(&mut self, a: u8, b: u8) {
        self.add_generic(a, b ^ 0xff, true);
    }

    // INLINE UINT8 CLH5801::decimaladd_generic(int left, int right, int carry)
    // {

    // 	UINT8 a = add_generic(left,right,carry);
    // 	if (!F_C && !F_H) a += 0x9a;
    // 	else
    // 	if (!F_C &&  F_H) a += 0xa0;
    // 	else
    // 	if ( F_C && !F_H) a += 0xfa;

    // 	return(a);
    // }

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

    // INLINE void CLH5801::DCA(UINT8 data)
    // {
    // 	lh5801.a = decimaladd_generic(lh5801.a + 0x66, data, bool(F_C));
    // }
    fn dca(&mut self, data: u8) {
        self.a = self.decimaladd_generic(self.a + 0x66, data, self.get_carry_flag());
    }

    // INLINE void CLH5801::DCS(UINT8 data)
    // {
    // 	lh5801.a = decimaladd_generic(lh5801.a, data^0xff, bool(F_C));
    // }
    fn dcs(&mut self, data: u8) {
        self.a = self.decimaladd_generic(self.a, data ^ 0xff, self.get_carry_flag());
    }

    // INLINE void CLH5801::AND(UINT8 data)
    // {
    // 	lh5801.a &= data;
    // 	CHECK_Z(lh5801.a);
    // }
    fn and(&mut self, data: u8) {
        self.a &= data;
        self.check_z(self.a);
    }

    // INLINE void CLH5801::AND_MEM(UINT32 addr, UINT8 data)
    // {
    // 	data &= cpu_readmem(addr);
    // 	CHECK_Z(data);
    // 	cpu_writemem(addr,data);
    // }

    fn and_mem(&mut self, memory: &mut MemoryBus, addr: u16, data: u8) {
        let data = data & self.cpu_readmem(memory, addr);
        self.check_z(data);
        self.cpu_writemem(memory, addr, data);
    }

    // INLINE void CLH5801::BIT(UINT8 a, UINT8 b)
    // {
    // 	CHECK_Z(a & b);
    // }
    fn bit(&mut self, a: u8, b: u8) {
        self.check_z(a & b);
    }

    // INLINE void CLH5801::EOR(UINT8 data)		// EXCLUSIVE OR
    // {
    // 	lh5801.a ^= data;
    // 	CHECK_Z(lh5801.a);
    // }
    fn eor(&mut self, data: u8) {
        self.a ^= data;
        self.check_z(self.a);
    }

    // INLINE void CLH5801::ORA(UINT8 data)
    // {
    // 	lh5801.a |= data;
    // 	CHECK_Z(lh5801.a);
    // }
    fn ora(&mut self, data: u8) {
        self.a |= data;
        self.check_z(self.a);
    }

    // INLINE void CLH5801::ORA_MEM(UINT32 addr, UINT8 data)
    // {
    // 	data |= cpu_readmem(addr);
    // 	CHECK_Z(data);
    // 	cpu_writemem(addr,data);
    // }
    fn ora_mem(&mut self, memory: &mut MemoryBus, addr: u16, mut data: u8) {
        data |= self.cpu_readmem(memory, addr);
        self.check_z(data);
        self.cpu_writemem(memory, addr, data);
    }

    // INLINE void CLH5801::LDA(UINT8 data)
    // {
    // 	lh5801.a = data;
    // 	CHECK_Z(data);
    // }
    fn lda(&mut self, data: u8) {
        self.a = data;
        self.check_z(data);
    }

    // INLINE void CLH5801::LDE(PAIR *reg)
    // {
    // 	// or z flag depends on reg
    // 	lh5801.a = cpu_readmem(reg->w--);
    // 	CHECK_Z(lh5801.a);
    // }
    fn lde(&mut self, memory: &mut MemoryBus, reg: u16) -> u16 {
        self.a = self.cpu_readmem(memory, reg);
        let ret = reg.wrapping_sub(1);
        self.check_z(self.a);
        ret
    }

    // INLINE void CLH5801::SDE(PAIR *reg)
    // {
    // 	cpu_writemem(reg->w--, lh5801.a);
    // }
    fn sde(&mut self, memory: &mut MemoryBus, reg: u16) -> u16 {
        self.cpu_writemem(memory, reg, self.a);
        reg.wrapping_sub(1)
    }

    // INLINE void CLH5801::LIN(PAIR *reg)
    // {
    // 	// or z flag depends on reg
    // 	lh5801.a = cpu_readmem(reg->w++);
    // 	CHECK_Z(lh5801.a);
    // }
    fn lin(&mut self, memory: &mut MemoryBus, reg: u16) -> u16 {
        self.a = self.cpu_readmem(memory, reg);
        let ret = reg.wrapping_add(1);
        self.check_z(self.a);
        ret
    }

    // INLINE void CLH5801::SIN(PAIR *reg)
    // {
    // 	cpu_writemem(reg->w++, lh5801.a);
    // }
    fn sin(&mut self, memory: &mut MemoryBus, reg: u16) -> u16 {
        self.cpu_writemem(memory, reg, self.a);
        reg.wrapping_add(1)
    }

    // INLINE void CLH5801::DEC(UINT8 *adr)
    // {
    // 	*adr=add_generic(*adr,0xff,0);
    // }
    fn dec(&mut self, adr: u8) -> u8 {
        self.add_generic(adr, 0xff, false)
    }

    // INLINE void CLH5801::INC(UINT8 *adr)
    // {
    // 	*adr=add_generic(*adr,1,0);
    // }
    fn inc(&mut self, adr: u8) -> u8 {
        self.add_generic(adr, 1, false)
    }

    // TODO: this is a NOP
    // INLINE void CLH5801::change_pc(UINT16 addr)
    // {
    // 	addr=0;
    // }

    // void CLH5801::RTN(void)
    // {
    // 	P =  cpu_readmem(++S)<<8;
    // 	P |= cpu_readmem(++S);
    // 	change_pc(P);
    //     CallSubLevel--;
    // }
    fn rtn(&mut self, memory: &mut MemoryBus) {
        self.s = self.s.wrapping_add(1);
        let hi = self.cpu_readmem(memory, self.s) as u16;
        self.s = self.s.wrapping_add(1);
        let lo = self.cpu_readmem(memory, self.s) as u16;
        self.p = (hi << 8) | lo;
        // change_pc(self.p); // Assuming this is handled elsewhere
        // CallSubLevel--; // Assuming this is handled elsewhere
    }

    // INLINE void CLH5801::RTI(void)
    // {
    // 	RTN();
    // 	// flags
    // 	T = cpu_readmem(++S);
    // }
    fn rti(&mut self, memory: &mut MemoryBus) {
        self.rtn(memory);
        // flags
        self.s = self.s.wrapping_add(1);
        self.t = self.cpu_readmem(memory, self.s);
    }

    // INLINE void CLH5801::JMP(UINT32 adr)
    // {
    // 	P = (UINT16) adr;
    // 	change_pc(P);
    // }
    fn jmp(&mut self, addr: u16) {
        self.p = addr;
    }

    // INLINE void CLH5801::AddState(UINT8 n)
    // {
    // 	pPC->pTIMER->state+=(n);
    //     ticks+=(n);
    // }
    fn add_state(&mut self, n: u8) {
        self.timer_state += n as usize;
    }

    // INLINE void CLH5801::BRANCH_PLUS(int doit)
    // {
    // 	UINT16 t = cpu_readop(P++);
    // 	if (doit) {
    // 		AddState(2);
    // 		P += t;
    // 		change_pc(P);
    // 	}
    // }
    fn branch_plus(&mut self, memory: &mut MemoryBus, doit: bool) {
        let t = self.cpu_readop(memory, self.p);
        self.p = self.p.wrapping_add(1);
        if doit {
            self.add_state(2);
            self.p = self.p.wrapping_add(u16::from(t));
        }
    }

    // INLINE void CLH5801::BRANCH_MINUS(int doit)
    // {
    // 	UINT8 t=cpu_readop(P++);
    // 	if (doit) {
    // 		AddState(3);
    // 		P -= t;
    // 		change_pc(P);
    // 	}
    // }
    fn branch_minus(&mut self, memory: &mut MemoryBus, doit: bool) {
        let t = self.cpu_readop(memory, self.p);
        self.p = self.p.wrapping_add(1);
        if doit {
            self.add_state(3);
            self.p = self.p.wrapping_sub(u16::from(t));
        }
    }

    // INLINE void CLH5801::LOP(void)
    // {
    // 	UINT8 t = cpu_readop(P++);

    // 	AddState(8);

    // 	if (UL--) {
    // 		AddState(3);
    // 		P -= t;
    // 		change_pc(P);
    // 	}
    // }
    fn lop(&mut self, memory: &mut MemoryBus) {
        let t = self.cpu_readop(memory, self.p);
        self.p = self.p.wrapping_add(1);
        self.add_state(8);
        if self.ul().wrapping_sub(1) != 0 {
            self.add_state(3);
            self.p = self.p.wrapping_sub(u16::from(t));
        }
        self.set_ul(self.ul().wrapping_sub(1));
    }

    // INLINE void CLH5801::SJP(void)
    // {
    // 	UINT16 t=readop_word();
    // 	PUSH_WORD(P);
    // 	P = t;
    // 	change_pc(t);
    //     CallSubLevel++;
    // }
    fn sjp(&mut self, memory: &mut MemoryBus) {
        let t = self.readop_word(memory);
        self.push_word(memory, self.p);
        self.p = t;
        // change_pc(t); // Assuming this is handled elsewhere
        // CallSubLevel++; // Assuming this is handled elsewhere
    }

    // INLINE void CLH5801::VECTOR(int doit, int nr)
    // {
    // 	if (doit) {
    // 		PUSH_WORD(P);
    // 		P =  (cpu_readmem(0xff00+nr) << 8) | cpu_readmem(0xff00+nr+1);
    // 		change_pc(P);
    // 		AddState(21-8);
    //         CallSubLevel++;
    // 	}
    // 	UNSET_Z;
    // }
    fn vector(&mut self, memory: &mut MemoryBus, doit: bool, nr: u8) {
        if doit {
            self.push_word(memory, self.p);
            let hi = self.cpu_readmem(memory, 0xFF00 + u16::from(nr)) as u16;
            let lo = self.cpu_readmem(memory, 0xFF00 + u16::from(nr) + 1) as u16;
            self.p = (hi << 8) | lo;
            // change_pc(self.p); // Assuming this is handled elsewhere
            self.add_state(21 - 8);
            // CallSubLevel++; // Assuming this is handled elsewhere
        }
        self.set_zero_flag(false);
    }

    // INLINE void CLH5801::AEX(void)
    // {
    // 	UINT8 l = lh5801.a;
    // 	lh5801.a = (l<<4) | (l>>4);
    // }
    fn aex(&mut self) {
        let l = self.a;
        self.a = (l << 4) | (l >> 4);
    }

    // INLINE void CLH5801::DRL(UINT32 adr)
    // {
    // 	UINT16 l = lh5801.a | (cpu_readmem(adr)<<8);

    // 	lh5801.a = l>>8;
    // 	cpu_writemem( adr , l>>4 );
    // }
    fn drl(&mut self, memory: &mut MemoryBus, addr: u16) {
        let l = u16::from(self.a) | (u16::from(self.cpu_readmem(memory, addr)) << 8);
        self.a = (l >> 8) as u8;
        self.cpu_writemem(memory, addr, (l >> 4) as u8);
    }

    // INLINE void CLH5801::DRR(UINT32 adr)
    // {
    // 	UINT16 l = cpu_readmem(adr) | (lh5801.a<<8);

    // 	lh5801.a = (UINT8) l;
    // 	cpu_writemem(adr,l>>4);
    // }
    fn drr(&mut self, memory: &mut MemoryBus, addr: u16) {
        let l = u16::from(self.cpu_readmem(memory, addr)) | (u16::from(self.a) << 8);
        self.a = (l & 0xFF) as u8;
        self.cpu_writemem(memory, addr, (l >> 4) as u8);
    }

    // INLINE void CLH5801::ROL(void)
    // {
    // 	// maybe use of the adder
    // 	int l = lh5801.a;
    // 	lh5801.a=(lh5801.a << 1) | F_C;

    // 	CHECK_C( l & 0x80 );				// OK
    // 	CHECK_Z( lh5801.a );				// OK
    // 	CHECK_H( lh5801.a & 0x10 );			// OK
    // 	CHECK_V( (l >= 0x40) && (l<0xc0) );	// OK

    // }
    fn rol(&mut self) {
        let l = self.a;
        self.a = (self.a << 1) | if self.get_carry_flag() { 1 } else { 0 };

        self.check_c(l & 0x80);
        self.check_z(self.a);
        self.check_h(self.a & 0x10);
        self.check_v((l >= 0x40) && (l < 0xc0));
    }

    // INLINE void CLH5801::ROR(void)
    // {
    // 	int l = lh5801.a;
    // 	lh5801.a = ((lh5801.a | (F_C << 8)) >> 1);

    // 	// flags cvhz
    // 	CHECK_C(l & 0x01);					// OK
    // 	CHECK_Z(lh5801.a);					// OK
    // 	CHECK_H(lh5801.a & 0x08);			// OK
    // 	CHECK_V( ( (l&0x01)&&(lh5801.a&0x02) ) || ((l&0x02)&&(lh5801.a&0x01)));	// OK
    // }
    fn ror(&mut self) {
        let l = self.a;
        self.a = ((self.a as u16 | (if self.get_carry_flag() { 1 } else { 0 } << 8)) >> 1) as u8;

        // flags cvhz
        self.check_c(l & 0x01);
        self.check_z(self.a);
        self.check_h(self.a & 0x08);
        self.check_v(
            ((l & 0x01 != 0) && (self.a & 0x02 != 0)) || ((l & 0x02 != 0) && (self.a & 0x01 != 0)),
        );
    }

    // INLINE void CLH5801::SHL(void)		// FLAGS OK
    // {
    // 	int l = lh5801.a;
    // 	lh5801.a<<=1;

    // 	CHECK_C(l & 0x80);					// OK
    // 	CHECK_Z(lh5801.a);					// OK
    // 	CHECK_H(l & 0x08);					// OK
    // 	CHECK_V((l>=0x40)&&(l<0xc0));		// OK
    // }
    fn shl(&mut self) {
        let l = self.a;
        self.a <<= 1;

        self.check_c(l & 0x80);
        self.check_z(self.a);
        self.check_h(l & 0x08);
        self.check_v((l >= 0x40) && (l < 0xc0));
    }

    // INLINE void CLH5801::SHR(void)		// FLAGS OK
    // {
    // 	int l=lh5801.a;
    // 	lh5801.a>>=1;

    // 	CHECK_C(l & 0x01);										// OK
    // 	CHECK_Z(lh5801.a);										// OK
    // 	CHECK_H(lh5801.a & 0x08);								// OK
    // 	CHECK_V( ( (l&0x01)&&(lh5801.a&0x02) ) || ((l&0x02)&&(lh5801.a&0x01)));	// OK
    // }
    fn shr(&mut self) {
        let l = self.a;
        self.a >>= 1;

        self.check_c(l & 0x01);
        self.check_z(self.a);
        self.check_h(self.a & 0x08);
        self.check_v(
            ((l & 0x01 != 0) && (self.a & 0x02 != 0)) || ((l & 0x02 != 0) && (self.a & 0x01 != 0)),
        );
    }

    // INLINE void CLH5801::AM(int value)
    // {
    // 	lh5801.tm=value;
    // }
    fn am(&mut self, value: u16) {
        self.tm = value;
    }

    // INLINE void CLH5801::ITA(void)
    // {
    //     lh5801.a=pPC->in(0);
    // 	CHECK_Z(lh5801.a);
    // }
    // FIXME: stub
    fn ita(&mut self, memory: &mut MemoryBus) {}

    // INLINE void CLH5801::instruction_fd(void)
    // {
    // 	int oper;
    // 	int adr;

    // 	oper = cpu_readop(P++);

    // //	Log_Oper(1,oper);

    // 	switch (oper) {
    // 	case 0x01:	SBC(cpu_readmem(ME1(X)));						AddState(11);	break;
    // 	case 0x03:	ADC(cpu_readmem(ME1(X)));						AddState(11);	break;
    // 	case 0x05:	LDA(cpu_readmem(ME1(X)));						AddState(10);/**/	break;
    // 	case 0x07:	CPA(lh5801.a, cpu_readmem(ME1(X))); 			AddState(11);	break;
    // 	case 0x08:	X=X;	AddLog(LOG_MASTER,"X=X op08");				AddState(11);	break;
    // 	case 0x09:	AND(cpu_readmem(ME1(X)));						AddState(11);	break;
    // 	case 0x0a:	POP_WORD(&lh5801.x);							AddState(15);	break;
    // 	case 0x0b:	ORA(cpu_readmem(ME1(X)));						AddState(11);	break;
    // 	case 0x0c:	DCS(cpu_readmem(ME1(X)));						AddState(17);/**/ 	break;
    // 	case 0x0d:	EOR(cpu_readmem(ME1(X)));						AddState(11);	break;
    // 	case 0x0e:	cpu_writemem(ME1(X),lh5801.a);					AddState(10);	break;
    // 	case 0x0f:	BIT(cpu_readmem(ME1(X)),lh5801.a); 				AddState(11);	break;
    // 	case 0x11:	SBC(cpu_readmem(ME1(Y)));						AddState(11);	break;
    // 	case 0x13:	ADC(cpu_readmem(ME1(Y)));						AddState(11);	break;
    // 	case 0x15:	LDA(cpu_readmem(ME1(Y)));						AddState(10);	break;
    // 	case 0x17:	CPA(lh5801.a, cpu_readmem(ME1(Y))); 			AddState(11);	break;
    // 	case 0x18:	X=Y;											AddState(11);	break;
    // 	case 0x19:	AND(cpu_readmem(ME1(Y)));						AddState(11);	break;
    // 	case 0x1a:	POP_WORD(&lh5801.y);							AddState(15);	break;
    // 	case 0x1b:	ORA(cpu_readmem(ME1(Y)));						AddState(11);	break;
    // 	case 0x1c:	DCS(cpu_readmem(ME1(Y)));						AddState(17);/**/ 	break;
    // 	case 0x1d:	EOR(cpu_readmem(ME1(Y))); 						AddState(11);	break;
    // 	case 0x1e:	cpu_writemem(ME1(Y),lh5801.a); 					AddState(10);/**/	break;
    // 	case 0x1f:	BIT(cpu_readmem(ME1(Y)),lh5801.a);				AddState(11);	break;
    // 	case 0x21:	SBC(cpu_readmem(ME1(U)));						AddState(11);	break;
    // 	case 0x23:	ADC(cpu_readmem(ME1(U)));						AddState(11);	break;
    // 	case 0x25:	LDA(cpu_readmem(ME1(U)));						AddState(10);	break;
    // 	case 0x27:	CPA(lh5801.a, cpu_readmem(ME1(U))); 			AddState(11);	break;
    // 	case 0x28:	X=U;											AddState(11);	break;
    // 	case 0x29:	AND(cpu_readmem(ME1(U)));						AddState(11);	break;
    // 	case 0x2a:	POP_WORD(&lh5801.u);							AddState(15);	break;
    // 	case 0x2b:	ORA(cpu_readmem(ME1(U)));						AddState(11);	break;
    // 	case 0x2c:	DCS(cpu_readmem(ME1(U)));						AddState(17);/**/ 	break;
    // 	case 0x2d:	EOR(cpu_readmem(ME1(U)));						AddState(11);	break;
    // 	case 0x2e:	cpu_writemem(ME1(U),lh5801.a); 					AddState(10);	break;
    // 	case 0x2f:	BIT(cpu_readmem(ME1(U)),lh5801.a); 				AddState(11);	break;
    //     case 0x3a:	S++;S++;            							AddState(15);	break;
    //     case 0x40:	INC(&XH);										AddState(9);	break;
    // 	case 0x42:	DEC(&XH);										AddState(9);	break;
    // 	case 0x48:	X=S;											AddState(11);	break;
    // 	case 0x49:	AND_MEM(ME1(X), cpu_readop(P++));				AddState(17);	break;
    // 	case 0x4a:	X=X;	AddLog(LOG_MASTER,"X=X op4a");			AddState(11);	break; //!!!
    // 	case 0x4b:	ORA_MEM(ME1(X), cpu_readop(P++)); 				AddState(17);	break;
    // 	case 0x4c:	lh5801.bf=0;/*off ! LOOK*/						AddState(8);	break;
    // 	case 0x4d:	BIT(cpu_readmem(ME1(X)), cpu_readop(P++));		AddState(14);/**/	break;
    // 	case 0x4e:	S=X;											AddState(11);	break;
    // 	case 0x4f:	ADD_MEM(ME1(X), cpu_readop(P++)); 				AddState(17);	break;
    // 	case 0x50:	INC(&YH);										AddState(9);	break;
    // 	case 0x52:	DEC(&YH);										AddState(9);	break;
    // 	case 0x58:	X=P;											AddState(11);	break;
    // 	case 0x59:	AND_MEM(ME1(Y), cpu_readop(P++));				AddState(17);	break;
    // 	case 0x5a:	Y=X;											AddState(11);	break;
    // 	case 0x5b:	ORA_MEM(ME1(Y), cpu_readop(P++)); 				AddState(17);	break;
    // 	case 0x5d:	BIT(cpu_readmem(ME1(Y)), cpu_readop(P++));		AddState(14);/**/	break;
    //     case 0x5e:	JMP(X);	CallSubLevel--;										AddState(11);	break; // P=X
    // 	case 0x5f:	ADD_MEM(ME1(Y), cpu_readop(P++));				AddState(17);	break;
    // 	case 0x60:	INC(&UH);										AddState(9);	break;
    // 	case 0x62:	DEC(&UH);										AddState(9);	break;
    // 	case 0x69:	AND_MEM(ME1(U), cpu_readop(P++)); 				AddState(17);	break;
    // 	case 0x6a:	U=X;											AddState(11);	break;
    // 	case 0x6b:	ORA_MEM(ME1(U), cpu_readop(P++)); 				AddState(17);	break;
    // 	case 0x6d:	BIT(cpu_readmem(ME1(X)), cpu_readop(P++));		AddState(14);/**/	break;
    // 	case 0x6f:	ADD_MEM(ME1(U), cpu_readop(P++)); 				AddState(17);	break;
    // 	case 0x81:	SET_IE; /*sie !*/								AddState(8);/**/	break;
    // 	case 0x88:	PUSH_WORD(X);									AddState(14);	break;
    // 	case 0x8a:	POP();											AddState(12);	break;
    // 	case 0x8c:	DCA(cpu_readmem(ME1(X)));						AddState(19); 	break;
    // //	case 0x8e:	/*cdv clears internal devider*/		/* LOOK*/	AddState(8);	break;
    // 	case 0x98:	PUSH_WORD(Y);									AddState(14);	break;
    // 	case 0x9c:	DCA(cpu_readmem(ME1(Y)));						AddState(19); 	break;
    // 	case 0xa1:	SBC(cpu_readmem(ME1(readop_word()))); 			AddState(17);	break;
    // 	case 0xa3:	ADC(cpu_readmem(ME1(readop_word()))); 			AddState(17);	break;
    // 	case 0xa5:	LDA(cpu_readmem(ME1(readop_word()))); 			AddState(16);/**/	break;
    // 	case 0xa7:	CPA(lh5801.a, cpu_readmem(ME1(readop_word())));	AddState(17);	break;
    // 	case 0xa8:	PUSH_WORD(U);									AddState(14);	break;
    // 	case 0xa9:	AND(cpu_readmem(ME1(readop_word()))); 			AddState(17);	break;
    // 	case 0xaa:	LDA(lh5801.t);CHECK_Z(lh5801.t);				AddState(9);	break;
    // 	case 0xab:	ORA(cpu_readmem(ME1(readop_word()))); 			AddState(17);	break;
    // 	case 0xac:	DCA(cpu_readmem(ME1(U)));						AddState(19); 	break;
    // 	case 0xad:	EOR(cpu_readmem(ME1(readop_word()))); 			AddState(17);	break;
    // 	case 0xae:	cpu_writemem(ME1(readop_word()),lh5801.a);		AddState(16);	break;
    // 	case 0xaf:	BIT(cpu_readmem(ME1(readop_word())),lh5801.a);	AddState(17);	break;
    //     case 0xb1:	lh5801.HLT=0;AddLog(0x01,"HALT");/* LOOK */		AddState(8);	break;
    //     case 0xb8:	PUSH_WORD((lh5801.s.b.h)<<8);   				AddState(14);	break;
    //     case 0xba:	ITA();											AddState(9);	break;
    // 	case 0xbe:	UNSET_IE; /*rie !*/								AddState(8);/**/	break;
    // 	case 0xc0:	lh5801.dp=0; /*rdp !*/							AddState(8);	break;
    // 	case 0xc1:	lh5801.dp=1; /*sdp !*/							AddState(8);	break;
    // 	case 0xc8:	PUSH(lh5801.a);									AddState(11);	break;
    // 	case 0xca:	ADR(&lh5801.x);									AddState(11);	break;
    // //	case 0xcc:	/*atp sends a to data bus*/		/* LOOK */		AddState(9);	break;
    // 	case 0xce:	AM(lh5801.a);									AddState(9); 	break;
    // 	case 0xd3:	DRR(ME1(X)); 									AddState(16);/**/ 	break;
    // 	case 0xd7:	DRL(ME1(X));									AddState(16);/**/ 	break;
    // 	case 0xda:	ADR(&lh5801.y);									AddState(11);	break;
    // 	case 0xde:	AM(lh5801.a|0x100);								AddState(9); 	break;
    // 	case 0xea:	ADR(&lh5801.u);									AddState(11);	break;
    // 	case 0xe9:	adr=ME1(readop_word());AND_MEM(adr, cpu_readop(P++));
    // 																AddState(23);	break;
    // 	case 0xeb: 	adr=ME1(readop_word());ORA_MEM(adr, cpu_readop(P++));
    // 																AddState(23);	break;
    // 	case 0xec:	lh5801.t=lh5801.a & 0x1F;						AddState(9);	break;
    // 	case 0xed:	adr=ME1(readop_word());BIT(cpu_readmem(adr), cpu_readop(P++));
    // 																AddState(20);/**/	break;
    // 	case 0xef:	adr=ME1(readop_word());ADD_MEM(adr, cpu_readop(P++));
    // 																AddState(23);	break;
    // 	default:
    //         if (!resetFlag) {
    // 				AddLog(LOG_MASTER,tr("lh5801 illegal opcode at %1  fd%2").arg((P-2),4,16,QChar('0')).arg((int)oper,2,16,QChar('0')));
    //                 qWarning()<<tr("lh5801 illegal opcode at %1  fd%2").arg((P-2),4,16,QChar('0')).arg((int)oper,2,16,QChar('0'));
    //                 pPC->BreakSubLevel = 99999;
    //                 pPC->DasmStep = true;
    //                 pPC->DasmFlag = false;
    //                 pPC->pBreakpointManager->breakMsg=tr("ill op at %1 %2").arg(P-1,4,16,QChar('0')).arg(oper,4,16,QChar('0'));
    //                 emit showDasm();
    //         }
    //                 break;
    // 	}
    // }
    fn instruction_fd(&mut self, memory: &mut MemoryBus) {
        let oper = self.cpu_readop(memory, self.p);
        self.p = self.p.wrapping_add(1);

        match oper {
            0x01 => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.sbc(read);
                self.add_state(11);
            }
            0x03 => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.adc(read);
                self.add_state(11);
            }
            0x05 => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.lda(read);
                self.add_state(10);
            }
            0x07 => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.cpa(self.a, read);
                self.add_state(11);
            }
            0x08 => {
                // X=X; AddLog(LOG_MASTER,"X=X op08"); // Assuming this is a no-op
                self.add_state(11);
            }
            0x09 => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.and(read);
                self.add_state(11);
            }
            0x0a => {
                self.x = self.pop_word(memory);
                self.add_state(15);
            }
            0x0b => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.ora(read);
                self.add_state(11);
            }
            0x0c => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.dcs(read);
                self.add_state(17);
            }
            0x0d => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.eor(read);
                self.add_state(11);
            }
            0x0e => {
                self.cpu_writemem(memory, self.me1(self.x()), self.a);
                self.add_state(10);
            }
            0x0f => {
                let data = self.cpu_readmem(memory, self.me1(self.x()));
                self.bit(data, self.a);
                self.add_state(11);
            }
            0x11 => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.sbc(read);
                self.add_state(11);
            }
            0x13 => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.adc(read);
                self.add_state(11);
            }
            0x15 => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.lda(read);
                self.add_state(10);
            }
            0x17 => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.cpa(self.a, read);
                self.add_state(11);
            }
            0x18 => {
                self.x = self.y;
                self.add_state(11);
            }
            0x19 => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.and(read);
                self.add_state(11);
            }
            0x1a => {
                self.y = self.pop_word(memory);
                self.add_state(15);
            }
            0x1b => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.ora(read);
                self.add_state(11);
            }
            0x1c => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.dcs(read);
                self.add_state(17);
            }
            0x1d => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.eor(read);
                self.add_state(11);
            }
            0x1e => {
                self.cpu_writemem(memory, self.me1(self.y()), self.a);
                self.add_state(10);
            }
            0x1f => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.bit(read, self.a);
                self.add_state(11);
            }
            0x21 => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.sbc(read);
                self.add_state(11);
            }
            0x23 => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.adc(read);
                self.add_state(11);
            }
            0x25 => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.lda(read);
                self.add_state(10);
            }
            0x27 => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.cpa(self.a, read);
                self.add_state(11);
            }
            0x28 => {
                self.x = self.u;
                self.add_state(11);
            }
            0x29 => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.and(read);
                self.add_state(11);
            }
            0x2a => {
                self.u = self.pop_word(memory);
                self.add_state(15);
            }
            0x2b => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.ora(read);
                self.add_state(11);
            }
            0x2c => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.dcs(read);
                self.add_state(17);
            }
            0x2d => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.eor(read);
                self.add_state(11);
            }
            0x2e => {
                self.cpu_writemem(memory, self.me1(self.u()), self.a);
                self.add_state(10);
            }
            0x2f => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                self.bit(read, self.a);
                self.add_state(11);
            }
            0x3a => {
                self.s = self.s.wrapping_add(2);
                self.add_state(15);
            }
            0x40 => {
                let inc = self.inc(self.xh());
                self.set_xh(inc);
                self.add_state(9);
            }
            0x42 => {
                let dec = self.dec(self.xh());
                self.set_xh(dec);
                self.add_state(9);
            }
            0x4a => {
                // X = X (no-op)
                self.add_state(11);
            }
            0x4b => {
                let op = self.cpu_readop(memory, self.p);
                self.ora_mem(memory, self.me1(self.x()), op);
                self.p = self.p.wrapping_add(1);
                self.add_state(14);
            }
            0x4d => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                let op = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.bit(read, op);
                self.add_state(14);
            }
            0x4f => {
                let op = self.cpu_readop(memory, self.p);
                self.add_mem(memory, self.me1(self.x()), op);
                self.p = self.p.wrapping_add(1);
                self.add_state(14);
            }
            0x50 => {
                let inc = self.inc(self.yh());
                self.set_yh(inc);
                self.add_state(9);
            }
            0x52 => {
                let dec = self.dec(self.yh());
                self.set_yh(dec);
                self.add_state(9);
            }
            0x58 => {
                self.y = self.x;
                self.add_state(11);
            }
            0x5a => {
                // Y = Y (no-op)
                self.add_state(11);
            }
            0x5b => {
                let op = self.cpu_readop(memory, self.p);
                self.ora_mem(memory, self.me1(self.y()), op);
                self.p = self.p.wrapping_add(1);
                self.add_state(14);
            }
            0x5d => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                let op = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.bit(read, op);
                self.add_state(14);
            }
            0x5f => {
                let op = self.cpu_readop(memory, self.p);
                self.add_mem(memory, self.me1(self.y()), op);
                self.p = self.p.wrapping_add(1);
                self.add_state(14);
            }
            0x60 => {
                let inc = self.inc(self.uh());
                self.set_uh(inc);
                self.add_state(9);
            }
            0x62 => {
                let dec = self.dec(self.uh());
                self.set_uh(dec);
                self.add_state(9);
            }
            0x68 => {
                self.u = self.x;
                self.add_state(11);
            }
            0x6a => {
                // U = U (no-op)
                self.add_state(11);
            }
            0x6b => {
                let op = self.cpu_readop(memory, self.p);
                self.ora_mem(memory, self.me1(self.u()), op);
                self.p = self.p.wrapping_add(1);
                self.add_state(14);
            }
            0x6d => {
                let read = self.cpu_readmem(memory, self.me1(self.u()));
                let op = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.bit(read, op);
                self.add_state(14);
            }
            0x6f => {
                let op = self.cpu_readop(memory, self.p);
                self.add_mem(memory, self.me1(self.u()), op);
                self.p = self.p.wrapping_add(1);
                self.add_state(17);
            }
            0x81 => {
                self.set_flag(IE, true); // Set Interrupt Enable
                self.add_state(8);
            }
            0x88 => {
                self.push_word(memory, self.x);
                self.add_state(14);
            }
            0x8a => {
                self.pop(memory);
                self.add_state(12);
            }
            0x8c => {
                let read = self.cpu_readmem(memory, self.me1(self.x()));
                self.dca(read);
                self.add_state(19);
            }
            // 0x8e => {} FIXME: clear internal divider
            0x98 => {
                self.push_word(memory, self.y);
                self.add_state(14);
            }
            0x9c => {
                let read = self.cpu_readmem(memory, self.me1(self.y()));
                self.dca(read);
                self.add_state(19);
            }
            0xa1 => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.sbc(read);
                self.add_state(17);
            }
            0xa3 => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.adc(read);
                self.add_state(17);
            }
            0xa5 => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.lda(read);
                self.add_state(16);
            }
            0xa7 => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.cpa(self.a, read);
                self.add_state(17);
            }
            0xa8 => {
                self.push_word(memory, self.u);
                self.add_state(14);
            }
            0xa9 => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.and(read);
                self.add_state(17);
            }
            0xaa => {
                self.lda(self.t);
                self.check_z(self.t);
                self.add_state(9);
            }
            0xab => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.ora(read);
                self.add_state(17);
            }
            0xac => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.dca(read);
                self.add_state(17);
            }
            0xad => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.eor(read);
                self.add_state(17);
            }
            0xae => {
                let op = self.readop_word(memory);
                self.cpu_writemem(memory, self.me1(op), self.a);
                self.add_state(16);
            }
            0xaf => {
                let op = self.readop_word(memory);
                let read = self.cpu_readmem(memory, self.me1(op));
                self.bit(read, self.a);
                self.add_state(17);
            }
            0xb1 => {
                self.is_halted = false;
                self.add_state(8);
            }
            0xb8 => {
                self.push_word(memory, (self.sh() as u16) << 8);
                self.add_state(14);
            }
            0xba => {
                self.ita(memory);
                self.add_state(9);
            }
            0xbe => {
                self.set_flag(IE, false); // Unset Interrupt Enable
                self.add_state(8);
            }
            0xc0 => {
                self.disp = false;
                self.add_state(8);
            }
            0xc1 => {
                self.disp = true;
                self.add_state(8);
            }
            0xc8 => {
                self.push(memory, self.a);
                self.add_state(11);
            }
            0xca => {
                self.x = self.adr(self.x);
                self.add_state(11);
            }
            0xcc => {
                // FIXME: ATP, unimplemented
                self.add_state(9);
            }
            0xce => {
                self.am(u16::from(self.a));
                self.add_state(9);
            }
            0xd3 => {
                self.drr(memory, self.me1(self.x()));
                self.add_state(16);
            }
            0xd7 => {
                self.drl(memory, self.me1(self.x()));
                self.add_state(16);
            }
            0xda => {
                self.y = self.adr(self.y);
                self.add_state(11);
            }
            0xde => {
                self.am(u16::from(self.a) | 0x100);
                self.add_state(9);
            }
            0xea => {
                self.u = self.adr(self.u);
                self.add_state(11);
            }
            0xe9 => {
                let op = self.readop_word(memory);
                let adr = self.me1(op);
                let read = self.cpu_readop(memory, self.p);
                self.and_mem(memory, adr, read);
                self.p = self.p.wrapping_add(1);
                self.add_state(23);
            }
            0xeb => {
                let op = self.readop_word(memory);
                let adr = self.me1(op);
                let read = self.cpu_readop(memory, self.p);
                self.ora_mem(memory, adr, read);
                self.p = self.p.wrapping_add(1);
                self.add_state(23);
            }
            0xec => {
                self.t = self.a & 0x1F;
                self.add_state(9);
            }
            0xed => {
                let op = self.readop_word(memory);
                let adr = self.me1(op);
                let read = self.cpu_readmem(memory, adr);
                let op2 = self.cpu_readop(memory, self.p);
                self.bit(read, op2);
                self.p = self.p.wrapping_add(1);
                self.add_state(20);
            }
            0xef => {
                let op = self.readop_word(memory);
                let adr = self.me1(op);
                let op2 = self.cpu_readop(memory, self.p);
                self.and_mem(memory, adr, op2);
                self.p = self.p.wrapping_add(1);
                self.add_state(23);
            }
            _ => {
                // panic for now
                panic!("Illegal opcode");
            }
        }
    }

    // INLINE void CLH5801::instruction(void)
    // {
    // 	int oper;
    // 	int adr;

    // 	oper=cpu_readop(P++);

    // //	Log_Oper(0,oper);

    // 	switch (oper) {
    // 	case 0x00: SBC(XL);									AddState(6);/**/	break;	//OK SBC XL
    // 	case 0x01: SBC(cpu_readmem(X));						AddState(7);/**/	break;	//OK SBC(X)
    // 	case 0x02: ADC(XL);									AddState(6);/**/	break;	//OK ADC XL
    // 	case 0x03: ADC(cpu_readmem(X));						AddState(7);/**/	break;	//OK ADC(X)
    // 	case 0x04: LDA(XL);									AddState(5);/**/	break;	//OK LDA XL
    // 	case 0x05: LDA(cpu_readmem(X));						AddState(6);/**/	break;	//OK LDA(X)
    // 	case 0x06: CPA(lh5801.a, XL);						AddState(6);/**/	break;	//OK CPA XL
    // 	case 0x07: CPA(lh5801.a, cpu_readmem(X));			AddState(7);/**/	break;	//OK CPA(X)
    // 	case 0x08: XH=lh5801.a;								AddState(5);/**/	break;	//OK STA XH
    // 	case 0x09: AND(cpu_readmem(X));						AddState(7);/**/	break;	//OK AND(X)
    // 	case 0x0a: XL=lh5801.a;								AddState(5);/**/	break;	//OK STA XL
    // 	case 0x0b: ORA(cpu_readmem(X));						AddState(7);/**/	break;	//OK ORA(X)
    // 	case 0x0c: DCS(cpu_readmem(X));						AddState(13);/**/	break;	//OK DCS(X)
    // 	case 0x0d: EOR(cpu_readmem(X));						AddState(7);/**/	break;	// EOR(X)
    // 	case 0x0e: cpu_writemem(X,lh5801.a);				AddState(6);/**/	break;	// STA(X)
    // 	case 0x0f: BIT(cpu_readmem(X),lh5801.a);			AddState(7);/**/	break;	// BIT(X)
    // 	case 0x10: SBC(YL);									AddState(6);/**/	break;	// SBC YL
    // 	case 0x11: SBC(cpu_readmem(Y));						AddState(7);/**/	break;	// SBC(Y)
    // 	case 0x12: ADC(YL);									AddState(6);/**/	break;	//OK ADC YL
    // 	case 0x13: ADC(cpu_readmem(Y));						AddState(7);/**/	break;	//OK ADC(Y)
    // 	case 0x14: LDA(YL);									AddState(5);/**/	break;
    // 	case 0x15: LDA(cpu_readmem(Y));						AddState(6);/**/	break;
    // 	case 0x16: CPA(lh5801.a, YL);						AddState(6);/**/	break;
    // 	case 0x17: CPA(lh5801.a, cpu_readmem(Y));			AddState(7);/**/	break;
    // 	case 0x18: YH=lh5801.a;								AddState(5);/**/	break;
    // 	case 0x19: AND(cpu_readmem(Y));						AddState(7);/**/	break;
    // 	case 0x1a: YL=lh5801.a;								AddState(5);/**/	break;
    // 	case 0x1b: ORA(cpu_readmem(Y));						AddState(7);/**/	break;
    // 	case 0x1c: DCS(cpu_readmem(Y));						AddState(13);/**/	break;
    // 	case 0x1d: EOR(cpu_readmem(Y));						AddState(7);/**/	break;
    // 	case 0x1e: cpu_writemem(Y,lh5801.a);				AddState(6);/**/	break;
    // 	case 0x1f: BIT(cpu_readmem(Y),lh5801.a);			AddState(7);/**/	break;
    // 	case 0x20: SBC(UL);									AddState(6);/**/	break;
    // 	case 0x21: SBC(cpu_readmem(U));						AddState(7);/**/	break;
    // 	case 0x22: ADC(UL);									AddState(6);/**/	break;
    // 	case 0x23: ADC(cpu_readmem(U));						AddState(7);/**/	break;
    // 	case 0x24: LDA(UL);									AddState(5);/**/	break;
    // 	case 0x25: LDA(cpu_readmem(U));						AddState(6);/**/	break;
    // 	case 0x26: CPA(lh5801.a, UL);						AddState(6);/**/	break;
    // 	case 0x27: CPA(lh5801.a, cpu_readmem(U));			AddState(7);/**/	break;
    // 	case 0x28: UH = lh5801.a;							AddState(5);/**/	break;
    // 	case 0x29: AND(cpu_readmem(U));						AddState(7);/**/	break;
    // 	case 0x2a: UL = lh5801.a;							AddState(5);/**/	break;
    // 	case 0x2b: ORA(cpu_readmem(U));						AddState(7);/**/	break;
    // 	case 0x2c: DCS(cpu_readmem(U));						AddState(13);/**/	break;
    // 	case 0x2d: EOR(cpu_readmem(U));						AddState(7);/**/	break;
    // 	case 0x2e: cpu_writemem(U,lh5801.a);				AddState(6);/**/	break;
    // 	case 0x2f: BIT(cpu_readmem(U),lh5801.a);			AddState(7);/**/	break;
    //     case 0x30: SBC(0);									AddState(6);/**/	break;
    //     case 0x32: ADC(0);									AddState(6);/**/	break;
    //     case 0x34: LDA(0);									AddState(5);/**/	break;
    //     case 0x36: CPA(lh5801.a, 0);						AddState(6);/**/	break;
    //     case 0x38: /*nop*/									AddState(5);/**/	break;
    // 	case 0x40: INC(&XL);								AddState(5);/**/	break;
    // 	case 0x41: SIN(&lh5801.x);							AddState(6);/**/	break;
    // 	case 0x42: DEC(&XL);								AddState(5);/**/	break;
    // 	case 0x43: SDE(&lh5801.x);							AddState(6);/**/	break;
    // 	case 0x44: X++;										AddState(5);/**/	break;
    // 	case 0x45: LIN(&lh5801.x);							AddState(6);/**/	break;
    // 	case 0x46: X--;										AddState(5);/**/	break;
    // 	case 0x47: LDE(&lh5801.x);							AddState(6);/**/	break;
    // 	case 0x48: XH=cpu_readop(P++);						AddState(6);/**/	break;
    // 	case 0x49: AND_MEM(X, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x4a: XL=cpu_readop(P++);						AddState(6);/**/	break;
    // 	case 0x4b: ORA_MEM(X, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x4c: CPA(XH, cpu_readop(P++));				AddState(7);/**/	break;
    // 	case 0x4d: BIT(cpu_readmem(X), cpu_readop(P++));	AddState(10);/**/	break;
    // 	case 0x4e: CPA(XL, cpu_readop(P++));				AddState(7);/**/	break;
    // 	case 0x4f: ADD_MEM(X, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x50: INC(&YL);								AddState(5);/**/	break;
    // 	case 0x51: SIN(&lh5801.y);							AddState(6);/**/	break;
    // 	case 0x52: DEC(&YL);								AddState(5);/**/	break;
    // 	case 0x53: SDE(&lh5801.y);							AddState(6);/**/	break;
    // 	case 0x54: Y++;										AddState(5);/**/	break;
    // 	case 0x55: LIN(&lh5801.y);							AddState(6);/**/	break;
    // 	case 0x56: Y--;										AddState(5);/**/	break;
    // 	case 0x57: LDE(&lh5801.y);							AddState(6);/**/	break;
    // 	case 0x58: YH=cpu_readop(P++);						AddState(6);/**/	break;
    // 	case 0x59: AND_MEM(Y, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x5a: YL=cpu_readop(P++);						AddState(6);/**/	break;
    // 	case 0x5b: ORA_MEM(Y, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x5c: CPA(YH, cpu_readop(P++));				AddState(7);/**/	break;
    // 	case 0x5d: BIT(cpu_readmem(Y), cpu_readop(P++));	AddState(10);/**/	break;
    // 	case 0x5e: CPA(YL, cpu_readop(P++));				AddState(7);/**/	break;
    // 	case 0x5f: ADD_MEM(Y, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x60: INC(&UL);								AddState(5);/**/	break;
    // 	case 0x61: SIN(&lh5801.u);							AddState(6);/**/	break;
    // 	case 0x62: DEC(&UL);								AddState(5);/**/	break;
    // 	case 0x63: SDE(&lh5801.u);							AddState(6);/**/	break;
    // 	case 0x64: U++;										AddState(5);/**/	break;
    // 	case 0x65: LIN(&lh5801.u);							AddState(6);/**/	break;
    // 	case 0x66: U--;										AddState(5);/**/	break;
    // 	case 0x67: LDE(&lh5801.u);							AddState(6);/**/	break;
    // 	case 0x68: UH=cpu_readop(P++);						AddState(6);/**/	break;
    // 	case 0x69: AND_MEM(U, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x6a: UL=cpu_readop(P++);						AddState(6);/**/	break;
    // 	case 0x6b: ORA_MEM(U, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x6c: CPA(UH, cpu_readop(P++));				AddState(7);/**/	break;
    // 	case 0x6d: BIT(cpu_readmem(U), cpu_readop(P++));	AddState(10);/**/	break;
    // 	case 0x6e: CPA(UL, cpu_readop(P++));				AddState(7);/**/	break;
    // 	case 0x6f: ADD_MEM(U, cpu_readop(P++));				AddState(13);/**/	break;
    // 	case 0x80: SBC(XH);									AddState(6);/**/	break;
    // 	case 0x81: BRANCH_PLUS(!F_C);						AddState(8);/**/	break;
    // 	case 0x82: ADC(XH);									AddState(6);/**/	break;
    // 	case 0x83: BRANCH_PLUS(F_C);						AddState(8);/**/	break;
    // 	case 0x84: LDA(XH);									AddState(5);/**/	break;
    // 	case 0x85: BRANCH_PLUS(!F_H);						AddState(8);/**/	break;
    // 	case 0x86: CPA(lh5801.a, XH);						AddState(6);/**/	break;
    // 	case 0x87: BRANCH_PLUS(F_H);						AddState(8);/**/	break;
    // 	case 0x88: LOP();												/**/	break;
    // 	case 0x89: BRANCH_PLUS(!F_Z);						AddState(8);/**/	break;
    // 	case 0x8a: RTI();									AddState(14);/**/	break;
    // 	case 0x8b: BRANCH_PLUS(F_Z);						AddState(8);/**/	break;
    // 	case 0x8c: DCA(cpu_readmem(X));						AddState(15);/**/	break;
    // 	case 0x8d: BRANCH_PLUS(!F_V);						AddState(8);/**/	break;
    // 	case 0x8e: BRANCH_PLUS(1);							AddState(6);/**/	break;
    // 	case 0x8f: BRANCH_PLUS(F_V);						AddState(8);/**/	break;
    // 	case 0x90: SBC(YH);									AddState(6);/**/	break;
    // 	case 0x91: BRANCH_MINUS(!F_C);						AddState(8);/**/	break;
    // 	case 0x92: ADC(YH);									AddState(6);/**/	break;
    // 	case 0x93: BRANCH_MINUS(F_C);						AddState(8);/**/	break;
    // 	case 0x94: LDA(YH);									AddState(5);/**/	break;
    // 	case 0x95: BRANCH_MINUS(!F_H);						AddState(8);/**/	break;
    // 	case 0x96: CPA(lh5801.a, YH);						AddState(6);/**/	break;
    // 	case 0x97: BRANCH_MINUS(F_H);						AddState(8);/**/	break;
    // 	case 0x99: BRANCH_MINUS(!F_Z);						AddState(8);/**/	break;
    // 	case 0x9a: RTN();									AddState(11);/**/	break;
    // 	case 0x9b: BRANCH_MINUS(F_Z);						AddState(8);/**/	break;
    // 	case 0x9c: DCA(cpu_readmem(Y));						AddState(15);/**/	break;
    // 	case 0x9d: BRANCH_MINUS(!F_V);						AddState(8);/**/	break;
    // 	case 0x9e: BRANCH_MINUS(1);							AddState(6);/**/	break;
    // 	case 0x9f: BRANCH_MINUS(F_V);						AddState(8);/**/	break;
    // 	case 0xa0: SBC(UH);									AddState(6);/**/	break;
    // 	case 0xa2: ADC(UH);									AddState(6);/**/	break;
    // 	case 0xa1: SBC(cpu_readmem(readop_word()));			AddState(13);/**/	break;
    // 	case 0xa3: ADC(cpu_readmem(readop_word()));			AddState(13);/**/	break;
    // 	case 0xa4: LDA(UH);									AddState(5);/**/	break;
    // 	case 0xa5: LDA(cpu_readmem(readop_word()));			AddState(12);/**/	break;
    // 	case 0xa6: CPA(lh5801.a, UH);						AddState(6);/**/	break;
    // 	case 0xa7: CPA(lh5801.a,cpu_readmem(readop_word()));AddState(13);/**/	break;
    // 	case 0xa8: lh5801.pv=1;/*spv!*/						AddState(4);/**/	break;
    // 	case 0xa9: AND(cpu_readmem(readop_word()));			AddState(13);/**/	break;
    // 	case 0xaa: S=readop_word();							AddState(12);/**/	break;
    // 	case 0xab: ORA(cpu_readmem(readop_word()));			AddState(13);/**/	break;
    // 	case 0xac: DCA(cpu_readmem(U));						AddState(15);/**/	break;
    // 	case 0xad: EOR(cpu_readmem(readop_word()));			AddState(13);/**/	break;
    // 	case 0xae: cpu_writemem(readop_word(),lh5801.a);	AddState(12);/**/	break;
    // 	case 0xaf: BIT(cpu_readmem(readop_word()),lh5801.a);AddState(13);/**/	break;
    // 	case 0xb1: SBC(cpu_readop(P++));					AddState(7);/**/	break;
    // 	case 0xb3: ADC(cpu_readop(P++));					AddState(7);/**/	break;
    // 	case 0xb5: LDA(cpu_readop(P++));					AddState(6);/**/	break;
    // 	case 0xb7: CPA(lh5801.a, cpu_readop(P++));			AddState(7);/**/	break;
    // 	case 0xb8: lh5801.pv=0;/*rpv!*/						AddState(4);/**/	break;
    // 	case 0xb9: AND(cpu_readop(P++));					AddState(7);/**/	break;
    // 	case 0xba: JMP(readop_word());						AddState(12);/**/	break;
    // 	case 0xbb: ORA(cpu_readop(P++));					AddState(7);/**/	break;
    // 	case 0xbd: EOR(cpu_readop(P++));					AddState(7);/**/	break;
    // 	case 0xbe: SJP();									AddState(19);/**/	break;
    // 	case 0xbf: BIT(lh5801.a,cpu_readop(P++));			AddState(7);/**/	break;
    // 	case 0xc1: VECTOR(!F_C, cpu_readop(P++));			AddState(8);/**/	break;
    // 	case 0xc3: VECTOR(F_C,  cpu_readop(P++));			AddState(8);/**/	break;
    // 	case 0xc5: VECTOR(!F_H, cpu_readop(P++));			AddState(8);/**/	break;
    // 	case 0xc7: VECTOR(F_H , cpu_readop(P++));			AddState(8);/**/	break;
    // 	case 0xc9: VECTOR(!F_Z, cpu_readop(P++));			AddState(8);/**/	break;
    // 	case 0xcb: VECTOR(F_Z , cpu_readop(P++));			AddState(8);/**/	break;
    // 	case 0xcd: VECTOR(  1 , cpu_readop(P++));			AddState(7);/**/	break;
    // 	case 0xcf: VECTOR(F_V , cpu_readop(P++));			AddState(8);/**/	break;
    // 	case 0xd1: ROR();									AddState(9);/**/	break;
    // 	case 0xd3: DRR(X);									AddState(12);/**/	break;
    // 	case 0xd5: SHR();									AddState(9);/**/	break;
    // 	case 0xd7: DRL(X);									AddState(12);/**/	break;
    // 	case 0xd9: SHL();									AddState(6);/**/	break;
    // 	case 0xdb: ROL();									AddState(8);/**/	break;
    // 	case 0xdd: INC(&lh5801.a);							AddState(5);/**/	break;
    // 	case 0xdf: DEC(&lh5801.a);							AddState(5);/**/	break;
    // 	case 0xe1: lh5801.pu=1;/*spu!*/						AddState(4);/**/	break;
    // 	case 0xe3: lh5801.pu=0;/*rpu!*/						AddState(4);/**/	break;
    // 	case 0xe9: adr=readop_word();
    // 			   AND_MEM(adr, cpu_readop(P++)); 			AddState(19);	break;
    // 	case 0xeb: adr=readop_word();
    // 			   ORA_MEM(adr, cpu_readop(P++));			AddState(19);	break;
    // 	case 0xed:
    // 		adr=readop_word();BIT(cpu_readmem(adr), cpu_readop(P++));
    // 														AddState(16);	break;
    // 	case 0xef:
    // 		adr=readop_word();ADD_MEM(adr, cpu_readop(P++));
    // 														AddState(19);	break;
    // 	case 0xf1: AEX();									AddState(6);/**/	break;
    // 	case 0xf5: cpu_writemem(Y++, cpu_readmem(X++));		AddState(7);	break; //TIN
    // 	case 0xf7: CPA(lh5801.a, cpu_readmem(X++));			AddState(7);	break; //CIN
    // 	case 0xf9: UNSET_C;									AddState(4);/**/	break;
    // 	case 0xfb: SET_C;									AddState(4);/**/	break;
    // 	case 0xfd: instruction_fd();										break;
    // 	case 0xc0: case 0xc2: case 0xc4: case 0xc6:
    // 	case 0xc8: case 0xca: case 0xcc: case 0xce:
    // 	case 0xd0: case 0xd2: case 0xd4: case 0xd6:
    // 	case 0xd8: case 0xda: case 0xdc: case 0xde:
    // 	case 0xe0: case 0xe2: case 0xe4: case 0xe6:
    // 	case 0xe8: case 0xea: case 0xec: case 0xee:
    // 	case 0xf0: case 0xf2: case 0xf4: case 0xf6:
    // 				VECTOR(1, oper);						AddState(4);/**/	break;
    // 	default:
    //         if (!resetFlag) {
    //             AddLog(LOG_MASTER,tr("lh5801 illegal opcode at %1 %2").arg(P-1,4,16,QChar('0')).arg(oper,4,16,QChar('0')));
    //             qWarning()<<tr("lh5801 illegal opcode at %1 %2").arg(P-1,4,16,QChar('0')).arg(oper,4,16,QChar('0'));

    //             pPC->BreakSubLevel = 99999;
    //             pPC->DasmStep = true;
    //             pPC->DasmFlag = false;
    //             pPC->pBreakpointManager->breakMsg=tr("ill op at %1 %2").arg(P-1,4,16,QChar('0')).arg(oper,4,16,QChar('0'));
    //             emit showDasm();
    //         }
    //         break;
    // 	}

    // }
    fn instruction(&mut self, memory: &mut MemoryBus) {
        let oper = self.cpu_readop(memory, self.p);
        self.p = self.p.wrapping_add(1);

        match oper {
            0x00 => {
                self.sbc(self.xl());
                self.add_state(6);
            }
            0x01 => {
                let val = self.cpu_readmem(memory, self.x());
                self.sbc(val);
                self.add_state(7);
            }
            0x02 => {
                self.adc(self.xl());
                self.add_state(6);
            }
            0x03 => {
                let val = self.cpu_readmem(memory, self.x());
                self.adc(val);
                self.add_state(7);
            }
            0x04 => {
                self.lda(self.xl());
                self.add_state(5);
            }
            0x05 => {
                let val = self.cpu_readmem(memory, self.x());
                self.lda(val);
                self.add_state(6);
            }
            0x06 => {
                self.cpa(self.a, self.xl());
                self.add_state(6);
            }
            0x07 => {
                let val = self.cpu_readmem(memory, self.x());
                self.cpa(self.a, val);
                self.add_state(7);
            }
            0x08 => {
                self.set_xh(self.a);
                self.add_state(5);
            }
            0x09 => {
                let val = self.cpu_readmem(memory, self.x());
                self.and(val);
                self.add_state(7);
            }
            0x0a => {
                self.set_xl(self.a);
                self.add_state(5);
            }
            0x0b => {
                let val = self.cpu_readmem(memory, self.x());
                self.ora(val);
                self.add_state(7);
            }
            0x0c => {
                let val = self.cpu_readmem(memory, self.x());
                self.dcs(val);
                self.add_state(13);
            }
            0x0d => {
                let val = self.cpu_readmem(memory, self.x());
                self.eor(val);
                self.add_state(7);
            }
            0x0e => {
                self.cpu_writemem(memory, self.x(), self.a);
                self.add_state(6);
            }
            0x0f => {
                let val = self.cpu_readmem(memory, self.x());
                self.bit(val, self.a);
                self.add_state(7);
            }

            0x10 => {
                self.sbc(self.yl());
                self.add_state(6);
            }
            0x11 => {
                let val = self.cpu_readmem(memory, self.y());
                self.sbc(val);
                self.add_state(7);
            }
            0x12 => {
                self.adc(self.yl());
                self.add_state(6);
            }
            0x13 => {
                let val = self.cpu_readmem(memory, self.y());
                self.adc(val);
                self.add_state(7);
            }
            0x14 => {
                self.lda(self.yl());
                self.add_state(5);
            }
            0x15 => {
                let val = self.cpu_readmem(memory, self.y());
                self.lda(val);
                self.add_state(6);
            }
            0x16 => {
                self.cpa(self.a, self.yl());
                self.add_state(6);
            }
            0x17 => {
                let val = self.cpu_readmem(memory, self.y());
                self.cpa(self.a, val);
                self.add_state(7);
            }
            0x18 => {
                self.set_yh(self.a);
                self.add_state(5);
            }
            0x19 => {
                let val = self.cpu_readmem(memory, self.y());
                self.and(val);
                self.add_state(7);
            }
            0x1a => {
                self.set_yl(self.a);
                self.add_state(5);
            }
            0x1b => {
                let val = self.cpu_readmem(memory, self.y());
                self.ora(val);
                self.add_state(7);
            }
            0x1c => {
                let val = self.cpu_readmem(memory, self.y());
                self.dcs(val);
                self.add_state(13);
            }
            0x1d => {
                let val = self.cpu_readmem(memory, self.y());
                self.eor(val);
                self.add_state(7);
            }
            0x1e => {
                self.cpu_writemem(memory, self.y(), self.a);
                self.add_state(6);
            }
            0x1f => {
                let val = self.cpu_readmem(memory, self.y());
                self.bit(val, self.a);
                self.add_state(7);
            }

            0x20 => {
                self.sbc(self.ul());
                self.add_state(6);
            }
            0x21 => {
                let val = self.cpu_readmem(memory, self.u());
                self.sbc(val);
                self.add_state(7);
            }
            0x22 => {
                self.adc(self.ul());
                self.add_state(6);
            }
            0x23 => {
                let val = self.cpu_readmem(memory, self.u());
                self.adc(val);
                self.add_state(7);
            }
            0x24 => {
                self.lda(self.ul());
                self.add_state(5);
            }
            0x25 => {
                let val = self.cpu_readmem(memory, self.u());
                self.lda(val);
                self.add_state(6);
            }
            0x26 => {
                self.cpa(self.a, self.ul());
                self.add_state(6);
            }
            0x27 => {
                let val = self.cpu_readmem(memory, self.u());
                self.cpa(self.a, val);
                self.add_state(7);
            }
            0x28 => {
                self.set_uh(self.a);
                self.add_state(5);
            }
            0x29 => {
                let val = self.cpu_readmem(memory, self.u());
                self.and(val);
                self.add_state(7);
            }
            0x2a => {
                self.set_ul(self.a);
                self.add_state(5);
            }
            0x2b => {
                let val = self.cpu_readmem(memory, self.u());
                self.ora(val);
                self.add_state(7);
            }
            0x2c => {
                let val = self.cpu_readmem(memory, self.u());
                self.dcs(val);
                self.add_state(13);
            }
            0x2d => {
                let val = self.cpu_readmem(memory, self.u());
                self.eor(val);
                self.add_state(7);
            }
            0x2e => {
                self.cpu_writemem(memory, self.u(), self.a);
                self.add_state(6);
            }
            0x2f => {
                let val = self.cpu_readmem(memory, self.u());
                self.bit(val, self.a);
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
                self.cpa(self.a, 0);
                self.add_state(6);
            }
            0x38 => {
                self.add_state(5);
            } // NOP

            0x40 => {
                let inc = self.inc(self.xl());
                self.set_xl(inc);
                self.add_state(5);
            }
            0x41 => {
                self.x = self.sin(memory, self.x);
                self.add_state(6);
            }
            0x42 => {
                let dec = self.dec(self.xl());
                self.set_xl(dec);
                self.add_state(5);
            }
            0x43 => {
                self.x = self.sde(memory, self.x);
                self.add_state(6);
            }
            0x44 => {
                self.x = self.x.wrapping_add(1);
                self.add_state(5);
            }
            0x45 => {
                self.x = self.lin(memory, self.x);
                self.add_state(6);
            }
            0x46 => {
                self.x = self.x.wrapping_sub(1);
                self.add_state(5);
            }
            0x47 => {
                self.x = self.lde(memory, self.x);
                self.add_state(6);
            }
            0x48 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.set_xh(val);
                self.add_state(6);
            }
            0x49 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.and_mem(memory, self.x(), val);
                self.add_state(13);
            }
            0x4a => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.set_xl(val);
                self.add_state(6);
            }
            0x4b => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.ora_mem(memory, self.x(), val);
                self.add_state(13);
            }
            0x4c => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.cpa(self.xh(), val);
                self.add_state(7);
            }
            0x4d => {
                let mem = self.cpu_readmem(memory, self.x());
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.bit(mem, val);
                self.add_state(10);
            }
            0x4e => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.cpa(self.xl(), val);
                self.add_state(7);
            }
            0x4f => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.add_mem(memory, self.x(), val);
                self.add_state(13);
            }

            0x50 => {
                let inc = self.inc(self.yl());
                self.set_yl(inc);
                self.add_state(5);
            }
            0x51 => {
                self.y = self.sin(memory, self.y);
                self.add_state(6);
            }
            0x52 => {
                let dec = self.dec(self.yl());
                self.set_yl(dec);
                self.add_state(5);
            }
            0x53 => {
                self.y = self.sde(memory, self.y);
                self.add_state(6);
            }
            0x54 => {
                self.y = self.y.wrapping_add(1);
                self.add_state(5);
            }
            0x55 => {
                self.y = self.lin(memory, self.y);
                self.add_state(6);
            }
            0x56 => {
                self.y = self.y.wrapping_sub(1);
                self.add_state(5);
            }
            0x57 => {
                self.y = self.lde(memory, self.y);
                self.add_state(6);
            }
            0x58 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.set_yh(val);
                self.add_state(6);
            }
            0x59 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.and_mem(memory, self.y(), val);
                self.add_state(13);
            }
            0x5a => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.set_yl(val);
                self.add_state(6);
            }
            0x5b => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.ora_mem(memory, self.y(), val);
                self.add_state(13);
            }
            0x5c => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.cpa(self.yh(), val);
                self.add_state(7);
            }
            0x5d => {
                let mem = self.cpu_readmem(memory, self.y());
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.bit(mem, val);
                self.add_state(10);
            }
            0x5e => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.cpa(self.yl(), val);
                self.add_state(7);
            }
            0x5f => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.add_mem(memory, self.y(), val);
                self.add_state(13);
            }

            0x60 => {
                let inc = self.inc(self.ul());
                self.set_ul(inc);
                self.add_state(5);
            }
            0x61 => {
                self.u = self.sin(memory, self.u);
                self.add_state(6);
            }
            0x62 => {
                let dec = self.dec(self.ul());
                self.set_ul(dec);
                self.add_state(5);
            }
            0x63 => {
                self.u = self.sde(memory, self.u);
                self.add_state(6);
            }
            0x64 => {
                self.u = self.u.wrapping_add(1);
                self.add_state(5);
            }
            0x65 => {
                self.u = self.lin(memory, self.u);
                self.add_state(6);
            }
            0x66 => {
                self.u = self.u.wrapping_sub(1);
                self.add_state(5);
            }
            0x67 => {
                self.u = self.lde(memory, self.u);
                self.add_state(6);
            }
            0x68 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.set_uh(val);
                self.add_state(6);
            }
            0x69 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.and_mem(memory, self.u(), val);
                self.add_state(13);
            }
            0x6a => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.set_ul(val);
                self.add_state(6);
            }
            0x6b => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.ora_mem(memory, self.u(), val);
                self.add_state(13);
            }
            0x6c => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.cpa(self.uh(), val);
                self.add_state(7);
            }
            0x6d => {
                let mem = self.cpu_readmem(memory, self.u());
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.bit(mem, val);
                self.add_state(10);
            }
            0x6e => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.cpa(self.ul(), val);
                self.add_state(7);
            }
            0x6f => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.add_mem(memory, self.u(), val);
                self.add_state(13);
            }

            0x80 => {
                self.sbc(self.xh());
                self.add_state(6);
            }
            0x81 => {
                self.branch_plus(memory, !self.get_carry_flag());
                self.add_state(8);
            }
            0x82 => {
                self.adc(self.xh());
                self.add_state(6);
            }
            0x83 => {
                self.branch_plus(memory, self.get_carry_flag());
                self.add_state(8);
            }
            0x84 => {
                self.lda(self.xh());
                self.add_state(5);
            }
            0x85 => {
                self.branch_plus(memory, !self.get_half_carry_flag());
                self.add_state(8);
            }
            0x86 => {
                self.cpa(self.a, self.xh());
                self.add_state(6);
            }
            0x87 => {
                self.branch_plus(memory, self.get_half_carry_flag());
                self.add_state(8);
            }
            0x88 => {
                self.lop(memory);
            }
            0x89 => {
                self.branch_plus(memory, !self.get_zero_flag());
                self.add_state(8);
            }
            0x8a => {
                self.rti(memory);
                self.add_state(14);
            }
            0x8b => {
                self.branch_plus(memory, self.get_zero_flag());
                self.add_state(8);
            }
            0x8c => {
                let val = self.cpu_readmem(memory, self.x());
                self.dca(val);
                self.add_state(15);
            }
            0x8d => {
                self.branch_plus(memory, !self.get_overflow_flag());
                self.add_state(8);
            }
            0x8e => {
                self.branch_plus(memory, true);
                self.add_state(6);
            }
            0x8f => {
                self.branch_plus(memory, self.get_overflow_flag());
                self.add_state(8);
            }

            0x90 => {
                self.sbc(self.yh());
                self.add_state(6);
            }
            0x91 => {
                self.branch_minus(memory, !self.get_carry_flag());
                self.add_state(8);
            }
            0x92 => {
                self.adc(self.yh());
                self.add_state(6);
            }
            0x93 => {
                self.branch_minus(memory, self.get_carry_flag());
                self.add_state(8);
            }
            0x94 => {
                self.lda(self.yh());
                self.add_state(5);
            }
            0x95 => {
                self.branch_minus(memory, !self.get_half_carry_flag());
                self.add_state(8);
            }
            0x96 => {
                self.cpa(self.a, self.yh());
                self.add_state(6);
            }
            0x97 => {
                self.branch_minus(memory, self.get_half_carry_flag());
                self.add_state(8);
            }
            0x99 => {
                self.branch_minus(memory, !self.get_zero_flag());
                self.add_state(8);
            }
            0x9a => {
                self.rtn(memory);
                self.add_state(11);
            }
            0x9b => {
                self.branch_minus(memory, self.get_zero_flag());
                self.add_state(8);
            }
            0x9c => {
                let val = self.cpu_readmem(memory, self.y());
                self.dca(val);
                self.add_state(15);
            }
            0x9d => {
                self.branch_minus(memory, !self.get_overflow_flag());
                self.add_state(8);
            }
            0x9e => {
                self.branch_minus(memory, true);
                self.add_state(6);
            }
            0x9f => {
                self.branch_minus(memory, self.get_overflow_flag());
                self.add_state(8);
            }

            0xa0 => {
                self.sbc(self.uh());
                self.add_state(6);
            }
            0xa1 => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readmem(memory, addr);
                self.sbc(val);
                self.add_state(13);
            }
            0xa2 => {
                self.adc(self.uh());
                self.add_state(6);
            }
            0xa3 => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readmem(memory, addr);
                self.adc(val);
                self.add_state(13);
            }
            0xa4 => {
                self.lda(self.uh());
                self.add_state(5);
            }
            0xa5 => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readmem(memory, addr);
                self.lda(val);
                self.add_state(12);
            }
            0xa6 => {
                self.cpa(self.a, self.uh());
                self.add_state(6);
            }
            0xa7 => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readmem(memory, addr);
                self.cpa(self.a, val);
                self.add_state(13);
            }
            0xa8 => {
                self.pv = true;
                self.add_state(4);
            } // SPV
            0xa9 => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readmem(memory, addr);
                self.and(val);
                self.add_state(13);
            }
            0xaa => {
                let addr = self.readop_word(memory);
                self.s = addr;
                self.add_state(12);
            }
            0xab => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readmem(memory, addr);
                self.ora(val);
                self.add_state(13);
            }
            0xac => {
                let val = self.cpu_readmem(memory, self.u());
                self.dca(val);
                self.add_state(15);
            }
            0xad => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readmem(memory, addr);
                self.eor(val);
                self.add_state(13);
            }
            0xae => {
                let addr = self.readop_word(memory);
                self.cpu_writemem(memory, addr, self.a);
                self.add_state(12);
            }
            0xaf => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readmem(memory, addr);
                self.bit(val, self.a);
                self.add_state(13);
            }

            0xb1 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.sbc(val);
                self.add_state(7);
            }
            0xb3 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.adc(val);
                self.add_state(7);
            }
            0xb5 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.lda(val);
                self.add_state(6);
            }
            0xb7 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.cpa(self.a, val);
                self.add_state(7);
            }
            0xb8 => {
                self.pv = false;
                self.add_state(4);
            } // RPV
            0xb9 => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.and(val);
                self.add_state(7);
            }
            0xba => {
                let addr = self.readop_word(memory);
                self.jmp(addr);
                self.add_state(12);
            }
            0xbb => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.ora(val);
                self.add_state(7);
            }
            0xbd => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.eor(val);
                self.add_state(7);
            }
            0xbe => {
                self.sjp(memory);
                self.add_state(19);
            }
            0xbf => {
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.bit(self.a, val);
                self.add_state(7);
            }

            0xc1 => {
                let nr = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.vector(memory, !self.get_carry_flag(), nr);
                self.add_state(8);
            }
            0xc3 => {
                let nr = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.vector(memory, self.get_carry_flag(), nr);
                self.add_state(8);
            }
            0xc5 => {
                let nr = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.vector(memory, !self.get_half_carry_flag(), nr);
                self.add_state(8);
            }
            0xc7 => {
                let nr = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.vector(memory, self.get_half_carry_flag(), nr);
                self.add_state(8);
            }
            0xc9 => {
                let nr = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.vector(memory, !self.get_zero_flag(), nr);
                self.add_state(8);
            }
            0xcb => {
                let nr = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.vector(memory, self.get_zero_flag(), nr);
                self.add_state(8);
            }
            0xcd => {
                let nr = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.vector(memory, true, nr);
                self.add_state(7);
            }
            0xcf => {
                let nr = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.vector(memory, self.get_overflow_flag(), nr);
                self.add_state(8);
            }

            0xd1 => {
                self.ror();
                self.add_state(9);
            }
            0xd3 => {
                self.drr(memory, self.x());
                self.add_state(12);
            }
            0xd5 => {
                self.shr();
                self.add_state(9);
            }
            0xd7 => {
                self.drl(memory, self.x());
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
                self.a = self.inc(self.a);
                self.add_state(5);
            }
            0xdf => {
                self.a = self.dec(self.a);
                self.add_state(5);
            }

            0xe1 => {
                self.pu = true;
                self.add_state(4);
            } // SPU
            0xe3 => {
                self.pu = false;
                self.add_state(4);
            } // RPU
            0xe9 => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.and_mem(memory, addr, val);
                self.add_state(19);
            }
            0xeb => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.ora_mem(memory, addr, val);
                self.add_state(19);
            }
            0xed => {
                let addr = self.readop_word(memory);
                let mem = self.cpu_readmem(memory, addr);
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.bit(mem, val);
                self.add_state(16);
            }
            0xef => {
                let addr = self.readop_word(memory);
                let val = self.cpu_readop(memory, self.p);
                self.p = self.p.wrapping_add(1);
                self.add_mem(memory, addr, val);
                self.add_state(19);
            }

            0xf1 => {
                self.aex();
                self.add_state(6);
            }
            0xf5 => {
                let val = self.cpu_readmem(memory, self.x);
                self.cpu_writemem(memory, self.y, val);
                self.x = self.x.wrapping_add(1);
                self.y = self.y.wrapping_add(1);
                self.add_state(7);
            } // TIN
            0xf7 => {
                let val = self.cpu_readmem(memory, self.x);
                self.cpa(self.a, val);
                self.x = self.x.wrapping_add(1);
                self.add_state(7);
            } // CIN
            0xf9 => {
                self.set_carry_flag(false);
                self.add_state(4);
            }
            0xfb => {
                self.set_carry_flag(true);
                self.add_state(4);
            }
            0xfd => {
                self.instruction_fd(memory);
            }

            // Vector instructions with immediate operand
            0xc0 | 0xc2 | 0xc4 | 0xc6 | 0xc8 | 0xca | 0xcc | 0xce | 0xd0 | 0xd2 | 0xd4 | 0xd6
            | 0xd8 | 0xda | 0xdc | 0xde | 0xe0 | 0xe2 | 0xe4 | 0xe6 | 0xe8 | 0xea | 0xec | 0xee
            | 0xf0 | 0xf2 | 0xf4 | 0xf6 => {
                self.vector(memory, true, oper);
                self.add_state(4);
            }

            _ => {
                // Illegal opcode - for now panic, later could be handled more gracefully
                panic!(
                    "Illegal opcode: 0x{:02x} at PC: 0x{:04x}",
                    oper,
                    self.p.wrapping_sub(1)
                );
            }
        }
    }
}
