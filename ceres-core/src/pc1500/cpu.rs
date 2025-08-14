// PC-1500 LH5801 CPU Implementation
// Based on the GameBoy CPU structure but adapted for PC-1500 specifications

use crate::pc1500::memory::MemoryBus;

// CPU Flag constants (based on LH5801 architecture según manual PC-1500)
const ZF: u8 = 0x01; // Z: Zero flag
const CF: u8 = 0x02; // C: Carry flag  
const VF: u8 = 0x04; // V: Overflow flag
const SF: u8 = 0x08; // Sign/Negative flag (not explicitly in manual but common)
const HF: u8 = 0x10; // H: Half carry flag
// IE: Interrupt enable flag - manejado por interrupt_enabled field

#[derive(Debug, Default)]
pub struct Lh5801Cpu {
    // 8-bit registers
    pub a: u8, // Accumulator
    pub b: u8, // B register

    // 16-bit registers
    pub x: u16, // X index register
    pub y: u16, // Y index register
    pub u: u16, // U pointer register
    pub s: u16, // Stack pointer
    pub p: u16, // Program counter

    // Flags register
    pub flags: u8,

    // CPU state
    pub interrupt_enabled: bool,
    pub is_halted: bool,

    // PC-1500 specific fields
    pub pu_flipflop: bool,
    pub pv_flipflop: bool,
    pub bf_flipflop: bool,
    pub display_enabled: bool,
    pub timer_register: u16,
    pub halted: bool,   // Alias for is_halted usado en control_instructions
    pub t_register: u8, // T register for ATT/TTA instructions
}

impl Lh5801Cpu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    // === REGISTER GETTERS (following GameBoy pattern) ===

    #[must_use]
    pub const fn a(&self) -> u8 {
        self.a
    }

    #[must_use]
    pub const fn b(&self) -> u8 {
        self.b
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
    pub const fn flags(&self) -> u8 {
        self.flags
    }

    #[must_use]
    pub const fn interrupt_enabled(&self) -> bool {
        self.interrupt_enabled
    }

    #[must_use]
    pub const fn is_halted(&self) -> bool {
        self.is_halted
    }

    // === REGISTER SETTERS ===

    pub fn set_pc(&mut self, pc: u16) {
        self.p = pc;
    }
    pub fn set_a(&mut self, a: u8) {
        self.a = a;
    }
    pub fn set_b(&mut self, b: u8) {
        self.b = b;
    }

    // === PC-1500 REGISTER ACCESS (según manual) ===

    // Acceso a partes bajas de registros de 16 bits (XL, YL, UL)
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

    // Acceso a partes altas de registros de 16 bits (XH, YH, UH)
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

    // Setters para partes de registros
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

    // === MEMORY ACCESS ===

    fn read(&mut self, memory: &mut MemoryBus, addr: u16) -> u8 {
        memory.read_byte(addr)
    }

    fn write(&mut self, memory: &mut MemoryBus, addr: u16, val: u8) {
        memory.write_byte(addr, val);
    }

    #[must_use]
    fn imm8(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read(memory, self.p);
        self.p = self.p.wrapping_add(1);
        val
    }

    #[must_use]
    fn imm16(&mut self, memory: &mut MemoryBus) -> u16 {
        let lo = u16::from(self.imm8(memory));
        let hi = u16::from(self.imm8(memory));
        (hi << 8) | lo
    }

    // === PC-1500 MEMORY ACCESS MODES (según manual) ===

    // (Rreg): Contenido de memoria accedido con ME0
    fn read_me0(&mut self, memory: &mut MemoryBus, rreg: u16) -> u8 {
        // ME0: acceso directo a memoria
        self.read(memory, rreg)
    }

    // #(Rreg): Contenido de memoria accedido con ME1
    fn read_me1(&mut self, memory: &mut MemoryBus, rreg: u16) -> u8 {
        // ME1: acceso indirecto a memoria
        let addr = self.read_word_me0(memory, rreg);
        self.read(memory, addr)
    }

    fn write_me0(&mut self, memory: &mut MemoryBus, rreg: u16, val: u8) {
        // ME0: escritura directa a memoria
        self.write(memory, rreg, val);
    }

    fn write_me1(&mut self, memory: &mut MemoryBus, rreg: u16, val: u8) {
        // ME1: escritura indirecta a memoria
        let addr = self.read_word_me0(memory, rreg);
        self.write(memory, addr, val);
    }

    // Lectura de palabra (16 bits) con ME0
    fn read_word_me0(&mut self, memory: &mut MemoryBus, addr: u16) -> u16 {
        let lo = u16::from(self.read(memory, addr));
        let hi = u16::from(self.read(memory, addr.wrapping_add(1)));
        (hi << 8) | lo
    }

    // (ab): 16-bit immediate address con ME0
    fn read_ab_me0(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        self.read(memory, addr)
    }

    // #(ab): 16-bit immediate address con ME1
    fn read_ab_me1(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        let target_addr = self.read_word_me0(memory, addr);
        self.read(memory, target_addr)
    }

    // === FLAG OPERATIONS ===

    const fn get_flag(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
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
    fn set_negative_flag(&mut self, value: bool) {
        self.set_flag(SF, value);
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
    fn get_negative_flag(&self) -> bool {
        self.get_flag(SF)
    }
    fn get_half_carry_flag(&self) -> bool {
        self.get_flag(HF)
    }

    // === ARITHMETIC OPERATIONS (según manual PC-1500) ===

    // ADC - ADD with Carry operation según manual
    fn adc_with_flags(&mut self, val: u8) -> u8 {
        let a = self.a;
        let carry = if self.get_carry_flag() { 1 } else { 0 };
        let result16 = u16::from(a) + u16::from(val) + u16::from(carry);
        let result = result16 as u8;

        // Set flags según manual PC-1500: C, H, Z, and V may change
        self.set_zero_flag(result == 0);
        self.set_carry_flag(result16 > 0xFF);
        self.set_half_carry_flag((a & 0x0F) + (val & 0x0F) + carry > 0x0F);

        // Overflow: resultado tiene signo diferente a ambos operandos
        let overflow = (a & 0x80) == (val & 0x80) && (a & 0x80) != (result & 0x80);
        self.set_overflow_flag(overflow);

        result
    }

    // ADD operation con flags según manual
    fn add_with_flags(&mut self, val: u8) -> u8 {
        let a = self.a;
        let result = a.wrapping_add(val);

        // Set flags según manual PC-1500
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(a) + u16::from(val) > 0xFF);
        self.set_half_carry_flag((a & 0x0F) + (val & 0x0F) > 0x0F);

        // Overflow: resultado tiene signo diferente a ambos operandos
        let overflow = (a & 0x80) == (val & 0x80) && (a & 0x80) != (result & 0x80);
        self.set_overflow_flag(overflow);

        result
    }

    // SBC - SUBTRACT with Carry operation según manual
    fn sbc_with_flags(&mut self, val: u8) -> u8 {
        let a = self.a;
        let carry = if self.get_carry_flag() { 1 } else { 0 };
        let result16 = i16::from(a) - i16::from(val) - i16::from(carry);
        let result = result16 as u8;

        // Set flags según manual PC-1500: C, H, Z, and V may change
        self.set_zero_flag(result == 0);
        self.set_carry_flag(result16 < 0); // Borrow flag
        self.set_half_carry_flag((a & 0x0F) < (val & 0x0F) + carry); // Half borrow

        // Overflow en substracción con carry
        let overflow = (a & 0x80) != (val & 0x80) && (a & 0x80) != (result & 0x80);
        self.set_overflow_flag(overflow);

        result
    }

    // SUBTRACT operation con flags según manual
    fn sub_with_flags(&mut self, val: u8) -> u8 {
        let a = self.a;
        let result = a.wrapping_sub(val);

        // Set flags según manual PC-1500
        self.set_zero_flag(result == 0);
        self.set_carry_flag(a < val); // Borrow flag
        self.set_half_carry_flag((a & 0x0F) < (val & 0x0F)); // Half borrow

        // Overflow en substracción
        let overflow = (a & 0x80) != (val & 0x80) && (a & 0x80) != (result & 0x80);
        self.set_overflow_flag(overflow);

        result
    }

    // AND operation con flags
    fn and_with_flags(&mut self, val: u8) -> u8 {
        let result = self.a & val;
        self.set_zero_flag(result == 0);
        self.set_carry_flag(false); // AND clear carry según manual
        self.set_overflow_flag(false); // AND clear overflow según manual
        result
    }

    // OR operation con flags
    fn or_with_flags(&mut self, val: u8) -> u8 {
        let result = self.a | val;
        self.set_zero_flag(result == 0);
        self.set_carry_flag(false); // OR clear carry según manual
        self.set_overflow_flag(false); // OR clear overflow según manual
        result
    }

    // XOR operation con flags (Exclusive OR según manual)
    fn xor_with_flags(&mut self, val: u8) -> u8 {
        let result = self.a ^ val;
        self.set_zero_flag(result == 0);
        self.set_carry_flag(false); // XOR clear carry según manual
        self.set_overflow_flag(false); // XOR clear overflow según manual
        result
    }

    // DCA - Decimal addition según manual
    fn dca_with_flags(&mut self, val: u8) -> u8 {
        let a = self.a;
        let carry = if self.get_carry_flag() { 1u8 } else { 0u8 };

        // Paso 1: A + 66H -> A
        let mut result = a.wrapping_add(0x66);

        // Paso 2: A + [operand] + C -> A (decimal addition)
        let result16 = u16::from(result) + u16::from(val) + u16::from(carry);
        result = result16 as u8;

        // Set flags según manual PC-1500: C, H, Z, and V may change
        self.set_zero_flag(result == 0);
        self.set_carry_flag(result16 > 0xFF);

        // Paso 3: A + DA -> A (decimal adjustment)
        let da_value = match (self.get_carry_flag(), self.get_half_carry_flag()) {
            (false, false) => 0x9A, // C=0, H=0
            (false, true) => 0xA0,  // C=0, H=1
            (true, false) => 0xFA,  // C=1, H=0
            (true, true) => 0x00,   // C=1, H=1
        };

        result = result.wrapping_add(da_value);

        // Update flags after decimal adjustment
        self.set_zero_flag(result == 0);
        // Carry and half-carry depend on the DA compensation

        result
    }

    // DCS - Decimal subtraction según manual
    fn dcs_with_flags(&mut self, val: u8) -> u8 {
        let a = self.a;
        let carry = if self.get_carry_flag() { 1u8 } else { 0u8 };

        // Paso 1: A + [operand] + C -> A
        let result16 = u16::from(a) + u16::from(val) + u16::from(carry);
        let mut result = result16 as u8;

        // Set flags según manual PC-1500: C, H, Z, and V may change
        self.set_zero_flag(result == 0);
        self.set_carry_flag(result16 > 0xFF);

        // Paso 2: A + DA -> A (decimal adjustment para substracción)
        let da_value = match (self.get_carry_flag(), self.get_half_carry_flag()) {
            (false, false) => 0x9A, // C=0, H=0
            (false, true) => 0xA0,  // C=0, H=1
            (true, false) => 0xFA,  // C=1, H=0
            (true, true) => 0x00,   // C=1, H=1
        };

        result = result.wrapping_add(da_value);

        // Update final flags
        self.set_zero_flag(result == 0);

        result
    }

    // INC - Increment según manual
    fn inc_with_flags(&mut self, val: u8) -> u8 {
        let result = val.wrapping_add(1);

        // Flags C, V, H and Z changed para registros 8-bit (A, RL, RH)
        // Para registros 16-bit (Rreg), no flag change takes place
        self.set_zero_flag(result == 0);
        self.set_carry_flag(val == 0xFF); // Overflow from 8-bit
        self.set_half_carry_flag((val & 0x0F) == 0x0F); // Overflow from 4-bit

        // Overflow: solo si cambia el bit de signo
        let overflow = (val & 0x80) == 0 && (result & 0x80) != 0;
        self.set_overflow_flag(overflow);

        result
    }

    // DEC - Decrement según manual
    fn dec_with_flags(&mut self, val: u8) -> u8 {
        let result = val.wrapping_sub(1);

        // Flags C, V, H and Z changed para registros 8-bit (A, RL, RH)
        // Para registros 16-bit (Rreg), no flag change takes place
        self.set_zero_flag(result == 0);
        self.set_carry_flag(val == 0x00); // Underflow from 8-bit
        self.set_half_carry_flag((val & 0x0F) == 0x00); // Underflow from 4-bit

        // Overflow: solo si cambia el bit de signo
        let overflow = (val & 0x80) != 0 && (result & 0x80) == 0;
        self.set_overflow_flag(overflow);

        result
    }

    // === INTERRUPT HANDLING (PC-1500 specific) ===

    /// Check if CPU should handle interrupts
    /// Based on IE flag and halt state
    pub fn should_handle_interrupt(&self) -> bool {
        self.interrupt_enabled && !self.is_halted
    }

    /// Handle interrupt processing sequence according to PC-1500 manual
    /// Implements the interrupt processing flow from the technical manual
    pub fn handle_interrupt(&mut self, vector: u16, memory: &mut MemoryBus) {
        if !self.should_handle_interrupt() {
            return;
        }

        // Start interrupt processing sequence
        // 1. Save current IE flag state to stack (part of context save)
        let ie_state = if self.interrupt_enabled { 1u8 } else { 0u8 };
        
        // 2. Disable interrupts (IE = 0)
        self.interrupt_enabled = false;
        
        // 3. Save Program Counter to stack (PH first, then PL)
        self.s = self.s.wrapping_sub(1);
        self.write(memory, self.s, (self.p >> 8) as u8); // PH
        self.s = self.s.wrapping_sub(1);
        self.write(memory, self.s, (self.p & 0xFF) as u8); // PL
        
        // 4. Save IE flag state to stack
        self.s = self.s.wrapping_sub(1);
        self.write(memory, self.s, ie_state);
        
        // 5. Load interrupt vector address
        // Vector points to address where interrupt routine address is stored
        let interrupt_routine_low = self.read(memory, vector);
        let interrupt_routine_high = self.read(memory, vector + 1);
        let interrupt_routine_addr = ((interrupt_routine_high as u16) << 8) | (interrupt_routine_low as u16);
        
        // 6. Jump to interrupt processing routine
        self.p = interrupt_routine_addr;
        
        // 7. Clear halt state if CPU was halted
        self.is_halted = false;
        self.halted = false;
    }

    /// Return from interrupt (RTI instruction implementation)
    /// Restores CPU state from stack according to PC-1500 manual
    pub fn return_from_interrupt(&mut self, memory: &mut MemoryBus) {
        // 1. Restore IE flag state from stack
        let ie_state = self.read(memory, self.s);
        self.s = self.s.wrapping_add(1);
        self.interrupt_enabled = ie_state != 0;
        
        // 2. Restore Program Counter from stack (PL first, then PH)
        let pl = self.read(memory, self.s);
        self.s = self.s.wrapping_add(1);
        let ph = self.read(memory, self.s);
        self.s = self.s.wrapping_add(1);
        
        // 3. Restore Program Counter
        self.p = ((ph as u16) << 8) | (pl as u16);
    }

    // === MAIN EXECUTION LOOP ===

    pub fn step(&mut self, memory: &mut MemoryBus) -> u8 {
        if self.is_halted {
            return 1; // Halt cycles
        }

        let opcode = self.imm8(memory);
        self.execute(opcode, memory)
    }

    fn execute(&mut self, opcode: u8, memory: &mut MemoryBus) -> u8 {
        match opcode {
            // === INSTRUCCIONES MULTI-BYTE CON PREFIJOS ===
            // Los opcodes que se repiten son PREFIJOS que requieren un segundo byte
            0xFD => self.execute_fd_prefix(memory), // FD prefix - requiere segundo byte
            // 0x05, 0x15, 0x25 son LDA (X), LDA (Y), LDA (U) - instrucciones directas, no prefijos

            // === INSTRUCCIONES DE UN SOLO BYTE (según manual LH5801) ===

            // SBC (X), (Y), (U) - Subtract with carry (según tablas oficiales PC-1500)
            0x01 => {
                let val = self.read_me0(memory, self.x);
                self.a = self.sbc_with_flags(val);
                7
            } // SBC (X)
            0x11 => {
                let val = self.read_me0(memory, self.y);
                self.a = self.sbc_with_flags(val);
                7
            } // SBC (Y)
            0x21 => {
                let val = self.read_me0(memory, self.u);
                self.a = self.sbc_with_flags(val);
                7
            } // SBC (U)

            // EOR (X), (Y), (U) - Exclusive OR (según tablas oficiales PC-1500)
            0x0D => {
                let val = self.read_me0(memory, self.x);
                self.a = self.xor_with_flags(val);
                7
            } // EOR (X)
            0x1D => {
                let val = self.read_me0(memory, self.y);
                self.a = self.xor_with_flags(val);
                7
            } // EOR (Y)
            0x2D => {
                let val = self.read_me0(memory, self.u);
                self.a = self.xor_with_flags(val);
                7
            } // EOR (U)

            // ADC (Add with Carry) - A + [operand] + C -> A
            0x02 => {
                let val = self.xl();
                self.a = self.adc_with_flags(val);
                5
            } // ADC XL
            0x12 => {
                let val = self.yl();
                self.a = self.adc_with_flags(val);
                5
            } // ADC YL
            0x22 => {
                let val = self.ul();
                self.a = self.adc_with_flags(val);
                5
            } // ADC UL
            0x82 => {
                let val = self.xh();
                self.a = self.adc_with_flags(val);
                5
            } // ADC XH
            0x92 => {
                let val = self.yh();
                self.a = self.adc_with_flags(val);
                5
            } // ADC YH
            0xA2 => {
                let val = self.uh();
                self.a = self.adc_with_flags(val);
                5
            } // ADC UH
            0x03 => {
                let val = self.read_me0(memory, self.x);
                self.a = self.adc_with_flags(val);
                7
            } // ADC (X)
            0x13 => {
                let val = self.read_me0(memory, self.y);
                self.a = self.adc_with_flags(val);
                7
            } // ADC (Y)
            0x23 => {
                let val = self.read_me0(memory, self.u);
                self.a = self.adc_with_flags(val);
                7
            } // ADC (U)
            0xA3 => {
                let val = self.read_ab_me0(memory);
                self.a = self.adc_with_flags(val);
                8
            } // ADC (ab)

            // ADI (Add Immediate) - A + i + C -> A or [operand] + i -> [operand]
            0xB3 => {
                let i = self.imm8(memory);
                self.a = self.adc_with_flags(i);
                6
            } // ADI A,i
            0x4F => {
                let i = self.imm8(memory);
                self.adi_x_i(memory, i);
                9
            } // ADI (X),i
            0x5F => {
                let i = self.imm8(memory);
                self.adi_y_i(memory, i);
                9
            } // ADI (Y),i
            0x6F => {
                let i = self.imm8(memory);
                self.adi_u_i(memory, i);
                9
            } // ADI (U),i
            0xEF => {
                let addr = self.imm16(memory);
                let i = self.imm8(memory);
                self.adi_ab_i(memory, addr, i);
                11
            } // ADI (ab),i

            // AND - A ∧ [operand] -> A
            0x09 => {
                let val = self.read_me0(memory, self.x);
                self.a = self.and_with_flags(val);
                7
            } // AND (X)
            0x19 => {
                let val = self.read_me0(memory, self.y);
                self.a = self.and_with_flags(val);
                7
            } // AND (Y)
            0x29 => {
                let val = self.read_me0(memory, self.u);
                self.a = self.and_with_flags(val);
                7
            } // AND (U)
            0xA9 => {
                let val = self.read_ab_me0(memory);
                self.a = self.and_with_flags(val);
                8
            } // AND (ab)

            // ANI (AND Immediate) - [operand] ∧ i -> [operand] or A ∧ i -> A
            0xB9 => {
                let i = self.imm8(memory);
                self.a = self.and_with_flags(i);
                6
            } // ANI A,i

            // INC - Increment operations ([operand] + 1 -> [operand])
            0xDD => self.inc_a(memory),  // INC A (6 cycles)
            0x40 => self.inc_xl(memory), // INC XL (5 cycles)
            0x50 => self.inc_yl(memory), // INC YL (5 cycles)
            0x60 => self.inc_ul(memory), // INC UL (5 cycles)
            0x44 => self.inc_x(memory),  // INC X (9 cycles)
            0x54 => self.inc_y(memory),  // INC Y (9 cycles)
            0x64 => self.inc_u(memory),  // INC U (9 cycles)

            // DEC - Decrement operations ([operand] - 1 -> [operand])
            0xDF => self.dec_a(memory),  // DEC A (6 cycles)
            0x42 => self.dec_xl(memory), // DEC XL (5 cycles)
            0x52 => self.dec_yl(memory), // DEC YL (5 cycles)
            0x62 => self.dec_ul(memory), // DEC UL (5 cycles)
            0x46 => self.dec_x(memory),  // DEC X (9 cycles)
            0x56 => self.dec_y(memory),  // DEC Y (9 cycles)
            0x66 => self.dec_u(memory),  // DEC U (9 cycles)

            0x49 => {
                let i = self.imm8(memory);
                self.ani_x_i(memory, i);
                9
            } // ANI (X),i
            0x59 => {
                let i = self.imm8(memory);
                self.ani_y_i(memory, i);
                9
            } // ANI (Y),i
            0x69 => {
                let i = self.imm8(memory);
                self.ani_u_i(memory, i);
                9
            } // ANI (U),i
            0xE9 => {
                let addr = self.imm16(memory);
                let i = self.imm8(memory);
                self.ani_ab_i(memory, addr, i);
                11
            } // ANI (ab),i

            // BIT - Test bit operations
            0x0F => {
                self.bit_x();
                7
            } // BIT (X)
            0x1F => {
                self.bit_y();
                7
            } // BIT (Y)
            0x2F => {
                self.bit_u();
                7
            } // BIT (U)
            0xAF => {
                let addr = self.imm16(memory);
                self.bit_ab(memory, addr);
                8
            } // BIT (ab)

            // BVS/BVR - Branch on overflow set/reset
            0x8F => {
                let offset = self.imm8(memory) as i8;
                if self.get_overflow_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BVS +
            0x9F => {
                let offset = self.imm8(memory) as i8;
                if !self.get_overflow_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BVR +

            // BZS/BZR - Branch on zero set/reset
            0x8B => {
                let offset = self.imm8(memory) as i8;
                if self.get_zero_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BZS +
            0x9B => {
                let offset = self.imm8(memory) as i8;
                if !self.get_zero_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BZR +

            // BCS/BCR - Branch on carry set/reset
            0x83 => {
                let offset = self.imm8(memory) as i8;
                if self.get_carry_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BCS +
            0x93 => {
                let offset = self.imm8(memory) as i8;
                if !self.get_carry_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BCR +
            0x81 => {
                let offset = self.imm8(memory) as i8;
                if self.get_carry_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BCR +
            0x91 => {
                let offset = self.imm8(memory) as i8;
                if !self.get_carry_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BCR -

            // BHS/BHR - Branch on half carry set/reset
            0x87 => {
                let offset = self.imm8(memory) as i8;
                if self.get_half_carry_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BHS +
            0x97 => {
                let offset = self.imm8(memory) as i8;
                if !self.get_half_carry_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BHR +
            0x85 => {
                let offset = self.imm8(memory) as i8;
                if self.get_half_carry_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BHR +
            0x95 => {
                let offset = self.imm8(memory) as i8;
                if !self.get_half_carry_flag() {
                    self.branch(offset);
                    8
                } else {
                    5
                }
            } // BHR -

            // BII - Branch if input
            0xBF => {
                let i = self.imm8(memory);
                self.bii_a_i(memory, i);
                6
            } // BII A,i
            0x4D => {
                let i = self.imm8(memory);
                self.bii_x_i(memory, i);
                9
            } // BII (X),i
            0x5D => {
                let i = self.imm8(memory);
                self.bii_y_i(memory, i);
                9
            } // BII (Y),i
            0x6D => {
                let i = self.imm8(memory);
                self.bii_u_i(memory, i);
                9
            } // BII (U),i
            0xED => {
                let addr = self.imm16(memory);
                let i = self.imm8(memory);
                self.bii_ab_i(memory, addr, i);
                11
            } // BII (ab),i

            // CDV/CIN/CPA/CPI - Control/Compare operations
            0xF7 => self.cin_block(memory), // CIN (Block Transfer Compare and Increment)
            0x06 => {
                let val = self.xl();
                self.cpa_with_flags(val);
                5
            } // CPA XL
            0x16 => {
                let val = self.yl();
                self.cpa_with_flags(val);
                5
            } // CPA YL
            0x26 => {
                let val = self.ul();
                self.cpa_with_flags(val);
                5
            } // CPA UL
            0x86 => {
                let val = self.xh();
                self.cpa_with_flags(val);
                5
            } // CPA XH
            0x96 => {
                let val = self.yh();
                self.cpa_with_flags(val);
                5
            } // CPA YH
            0xA6 => {
                let val = self.uh();
                self.cpa_with_flags(val);
                5
            } // CPA UH
            0x07 => {
                let val = self.read_me0(memory, self.x);
                self.cpa_with_flags(val);
                7
            } // CPA (X)
            0x17 => {
                let val = self.read_me0(memory, self.y);
                self.cpa_with_flags(val);
                7
            } // CPA (Y)
            0x27 => {
                let val = self.read_me0(memory, self.u);
                self.cpa_with_flags(val);
                7
            } // CPA (U)
            0xA7 => {
                let val = self.read_ab_me0(memory);
                self.cpa_with_flags(val);
                8
            } // CPA (ab)

            0xB7 => {
                let i = self.imm8(memory);
                self.cpa_with_flags(i);
                6
            } // CPI A,i
            0x4E => {
                let i = self.imm8(memory);
                self.cpi_x_i(memory, i);
                9
            } // CPI (X),i
            0x5E => {
                let i = self.imm8(memory);
                self.cpi_y_i(memory, i);
                9
            } // CPI (Y),i
            0x6E => {
                let i = self.imm8(memory);
                self.cpi_u_i(memory, i);
                9
            } // CPI (U),i
            0x4C => {
                let i = self.imm8(memory);
                self.cpi_xh_i(i);
                6
            } // CPI XH,i (no conflict - OFF is FD 4C)
            0x5C => {
                let i = self.imm8(memory);
                self.cpi_yh_i(i);
                6
            } // CPI YH,i
            0x6C => {
                let i = self.imm8(memory);
                self.cpi_uh_i(i);
                6
            } // CPI UH,i

            // DCA/DCS - Decimal operations
            0x8C => {
                let val = self.read_me0(memory, self.x);
                self.a = self.dca_with_flags(val);
                7
            } // DCA (X)
            0x9C => {
                let val = self.read_me0(memory, self.y);
                self.a = self.dca_with_flags(val);
                7
            } // DCA (Y)
            0xAC => {
                let val = self.read_me0(memory, self.u);
                self.a = self.dca_with_flags(val);
                7
            } // DCA (U)

            0x0C => {
                let val = self.read_me0(memory, self.x);
                self.a = self.dcs_with_flags(val);
                7
            } // DCS (X)
            0x1C => {
                let val = self.read_me0(memory, self.y);
                self.a = self.dcs_with_flags(val);
                7
            } // DCS (Y)
            0x2C => {
                let val = self.read_me0(memory, self.u);
                self.a = self.dcs_with_flags(val);
                7
            } // DCS (U)

            // ===== TRANSFER AND SEARCH INSTRUCTIONS (single byte) =====

            // LDA XL, YL, UL, XH, YH, UH, (X), (Y), (U), (ab) - Load to accumulator
            0x04 => self.lda_rl(self.xl()),        // LDA XL
            0x14 => self.lda_rl(self.yl()),        // LDA YL
            0x24 => self.lda_rl(self.ul()),        // LDA UL
            0x84 => self.lda_rh(self.xh()),        // LDA XH
            0x94 => self.lda_rh(self.yh()),        // LDA YH
            0xA4 => self.lda_rh(self.uh()),        // LDA UH
            0x05 => self.lda_rreg(memory, self.x), // LDA (X)
            0x15 => self.lda_rreg(memory, self.y), // LDA (Y)
            0x25 => self.lda_rreg(memory, self.u), // LDA (U)
            0xA5 => self.lda_ab(memory),           // LDA (ab)

            // LDI A,XL,YL,UL,XH,YH,UH,S - Load immediate
            0xB5 => self.ldi_a(memory), // LDI A,i
            0x4A => self.ldi_rl(memory, |cpu, val| cpu.set_xl(val)), // LDI XL,i
            0x5A => self.ldi_rl(memory, |cpu, val| cpu.set_yl(val)), // LDI YL,i
            0x6A => self.ldi_rl(memory, |cpu, val| cpu.set_ul(val)), // LDI UL,i
            0x48 => self.ldi_rh(memory, |cpu, val| cpu.set_xh(val)), // LDI XH,i
            0x58 => self.ldi_rh(memory, |cpu, val| cpu.set_yh(val)), // LDI YH,i
            0x68 => self.ldi_rh(memory, |cpu, val| cpu.set_uh(val)), // LDI UH,i
            0xAA => self.ldi_s(memory), // LDI S,i

            // LDE X,Y,U - Load and decrement
            0x47 => {
                let mut x = self.x;
                let cycles = self.lde_rreg(memory, &mut x);
                self.x = x;
                cycles
            }
            0x57 => {
                let mut y = self.y;
                let cycles = self.lde_rreg(memory, &mut y);
                self.y = y;
                cycles
            }
            0x67 => {
                let mut u = self.u;
                let cycles = self.lde_rreg(memory, &mut u);
                self.u = u;
                cycles
            }

            // LIN X,Y,U - Load and increment
            0x45 => {
                let mut x = self.x;
                let cycles = self.lin_rreg(memory, &mut x);
                self.x = x;
                cycles
            }
            0x55 => {
                let mut y = self.y;
                let cycles = self.lin_rreg(memory, &mut y);
                self.y = y;
                cycles
            }
            0x65 => {
                let mut u = self.u;
                let cycles = self.lin_rreg(memory, &mut u);
                self.u = u;
                cycles
            }

            // STA XL,YL,UL,XH,YH,UH,(X),(Y),(U),(ab) - Store accumulator
            0x0A => self.sta_rl(|cpu, val| cpu.set_xl(val)), // STA XL
            0x1A => self.sta_rl(|cpu, val| cpu.set_yl(val)), // STA YL
            0x2A => self.sta_rl(|cpu, val| cpu.set_ul(val)), // STA UL
            0x08 => self.sta_rh(|cpu, val| cpu.set_xh(val)), // STA XH
            0x18 => self.sta_rh(|cpu, val| cpu.set_yh(val)), // STA YH
            0x28 => self.sta_rh(|cpu, val| cpu.set_uh(val)), // STA UH
            0x0E => self.sta_rreg(memory, self.x),           // STA (X)
            0x1E => self.sta_rreg(memory, self.y),           // STA (Y)
            0x2E => self.sta_rreg(memory, self.u),           // STA (U)
            0xAE => {
                let addr = self.imm16(memory);
                self.sta_ab(memory, addr)
            } // STA (ab)

            // SDE X,Y,U - Store and decrement
            0x43 => {
                let mut x = self.x;
                let cycles = self.sde_rreg(memory, &mut x);
                self.x = x;
                cycles
            }
            0x53 => {
                let mut y = self.y;
                let cycles = self.sde_rreg(memory, &mut y);
                self.y = y;
                cycles
            }
            0x63 => {
                let mut u = self.u;
                let cycles = self.sde_rreg(memory, &mut u);
                self.u = u;
                cycles
            }

            // SIN X,Y,U - Store and increment
            0x41 => {
                let mut x = self.x;
                let cycles = self.sin_rreg(memory, &mut x);
                self.x = x;
                cycles
            }
            0x51 => {
                let mut y = self.y;
                let cycles = self.sin_rreg(memory, &mut y);
                self.y = y;
                cycles
            }
            0x61 => {
                let mut u = self.u;
                let cycles = self.sin_rreg(memory, &mut u);
                self.u = u;
                cycles
            }

            // ===== BLOCK TRANSFER AND SEARCH INSTRUCTIONS (2-4-5) =====

            // TIN - Transfer and increment (Xreg to Yreg, then increment both)
            0xF5 => self.tin(memory), // TIN

            // ===== ROTATE AND SHIFT INSTRUCTIONS (2-4-6) =====

            // ROL - Rotate left through carry
            0xDB => self.rol(), // ROL (8 cycles)

            // ROR - Rotate right through carry
            0xD1 => self.ror(), // ROR (9 cycles)

            // SHL - Shift left logical
            0xD9 => self.shl(), // SHL (6 cycles)

            // SHR - Shift right logical
            0xD5 => self.shr(), // SHR (9 cycles)

            // DRL - Digit rotate left (direct addressing)
            0xD7 => self.drl_xreg(memory), // DRL (X) (12 cycles)

            // DRR - Digit rotate right (direct addressing)
            0xD3 => self.drr_xreg(memory), // DRR (X) (12 cycles)

            // AEX - Exchange accumulator high and low nibbles
            0xF1 => self.aex(), // AEX (6 cycles)

            // ===== CPU CONTROL INSTRUCTIONS =====

            // SPU - Set PU flipflop
            0xE1 => self.spu(), // SPU (4 cycles)

            // RPU - Reset PU flipflop
            0xE3 => self.rpu(), // RPU (4 cycles)

            // SPV - Set PV flipflop
            0xA8 => self.spv(), // SPV (4 cycles)

            // RPV - Reset PV flipflop
            0xB8 => self.rpv(), // RPV (4 cycles)

            // NOP - No operation
            0x38 => self.nop(), // NOP (5 cycles)

            // SEC - Set carry flag
            0xFB => self.sec(), // SEC (4 cycles)

            // REC - Reset carry flag
            0xF9 => self.rec(), // REC (4 cycles)

            // ===== JUMP INSTRUCTIONS (2-4-8) =====

            // JMP - Absolute jump (3 bytes: opcode + 16-bit address)
            0xBA => self.jmp(memory), // JMP i,j (12 cycles)

            // BCH - Relative branch (2 bytes: opcode + 8-bit offset)
            0x8E => self.bch(memory),       // BCH+ i (8 cycles)
            0x9E => self.bch_minus(memory), // BCH- i (8 cycles)

            // LOP - Loop instruction (2 bytes: opcode + 8-bit offset)
            0x88 => self.lop(memory), // LOP i (8/11 cycles)

            // ===== SUBROUTINE JUMP INSTRUCTIONS (2-4-9) =====

            // ===== SUBROUTINE JUMP INSTRUCTIONS (2-4-9) =====

            // SJP - Subroutine jump with absolute address (3 bytes: opcode + 16-bit address)
            0xBE => self.sjp(memory), // SJP i,j (19 cycles)

            // VEJ - Vector subroutine jump instructions (official PC-1500 manual opcodes)
            0xC0 => self.vej(memory, 0xC0), // VEJ C0
            0xC1 => self.vcr(memory),       // VCR i (Vector subroutine jump if C Reset)
            0xC2 => self.vej(memory, 0xC2), // VEJ C2
            0xC4 => self.vej(memory, 0xC4), // VEJ C4
            0xC6 => self.vej(memory, 0xC6), // VEJ C6
            0xC8 => self.vej(memory, 0xC8), // VEJ C8
            0xCA => self.vej(memory, 0xCA), // VEJ CA
            0xCC => self.vej(memory, 0xCC), // VEJ CC
            0xCE => self.vej(memory, 0xCE), // VEJ CE
            0xD0 => self.vej(memory, 0xD0), // VEJ D0
            0xD2 => self.vej(memory, 0xD2), // VEJ D2
            0xD4 => self.vej(memory, 0xD4), // VEJ D4
            0xD6 => self.vej(memory, 0xD6), // VEJ D6
            0xD8 => self.vej(memory, 0xD8), // VEJ D8
            0xDA => self.vej(memory, 0xDA), // VEJ DA
            0xDC => self.vej(memory, 0xDC), // VEJ DC
            0xDE => self.vej(memory, 0xDE), // VEJ DE
            0xE0 => self.vej(memory, 0xE0), // VEJ E0
            0xE2 => self.vej(memory, 0xE2), // VEJ E2
            0xE4 => self.vej(memory, 0xE4), // VEJ E4
            0xE6 => self.vej(memory, 0xE6), // VEJ E6
            0xE8 => self.vej(memory, 0xE8), // VEJ E8
            0xEA => self.vej(memory, 0xEA), // VEJ EA
            0xEC => self.vej(memory, 0xEC), // VEJ EC
            0xEE => self.vej(memory, 0xEE), // VEJ EE
            0xF0 => self.vej(memory, 0xF0), // VEJ F0
            0xF2 => self.vej(memory, 0xF2), // VEJ F2
            0xF4 => self.vej(memory, 0xF4), // VEJ F4
            0xF6 => self.vej(memory, 0xF6), // VEJ F6

            // ===== RETURN INSTRUCTIONS (2-4-10) =====

            // RTI - Return from interrupt service routine
            0x8A => self.rti(memory), // RTI (14 cycles)

            // RTN - Return from subroutine
            0x9A => self.rtn(memory), // RTN (11 cycles)

            // ===== MISSING INSTRUCTIONS =====

            // SBC - Subtract with carry (missing register variants)
            0x00 => {
                let val = self.xl();
                self.a = self.sbc_with_flags(val);
                5
            } // SBC XL
            0x10 => {
                let val = self.yl();
                self.a = self.sbc_with_flags(val);
                5
            } // SBC YL
            0x20 => {
                let val = self.ul();
                self.a = self.sbc_with_flags(val);
                5
            } // SBC UL
            0x80 => {
                let val = self.xh();
                self.a = self.sbc_with_flags(val);
                5
            } // SBC XH
            0x90 => {
                let val = self.yh();
                self.a = self.sbc_with_flags(val);
                5
            } // SBC YH
            0xA0 => {
                let val = self.uh();
                self.a = self.sbc_with_flags(val);
                5
            } // SBC UH
            0xA1 => {
                let val = self.read_ab_me0(memory);
                self.a = self.sbc_with_flags(val);
                7
            } // SBC (ab)

            // EOR - Exclusive OR (missing ab variant)
            0xAD => {
                let val = self.read_ab_me0(memory);
                self.a = self.xor_with_flags(val);
                7
            } // EOR (ab)

            // BZR/BVR - Branch on zero/overflow clear (missing variants)
            0x89 => {
                let disp = self.imm8(memory) as i8;
                if !self.get_zero_flag() {
                    self.p = (self.p as i32 + disp as i32) as u16;
                };
                8
            } // BZR +
            0x99 => {
                let disp = self.imm8(memory) as i8;
                if !self.get_zero_flag() {
                    self.p = (self.p as i32 - disp as i32) as u16;
                };
                8
            } // BZR -
            0x8D => {
                let disp = self.imm8(memory) as i8;
                if !self.get_overflow_flag() {
                    self.p = (self.p as i32 + disp as i32) as u16;
                };
                8
            } // BVR +
            0x9D => {
                let disp = self.imm8(memory) as i8;
                if !self.get_overflow_flag() {
                    self.p = (self.p as i32 - disp as i32) as u16;
                };
                8
            } // BVR -

            // ORI - OR immediate
            0xBB => {
                let i = self.imm8(memory);
                self.a = self.or_with_flags(i);
                6
            } // ORI A,i
            0x4B => {
                let i = self.imm8(memory);
                self.ori_x_i(memory, i);
                8
            } // ORI (X),i
            0x5B => {
                let i = self.imm8(memory);
                self.ori_y_i(memory, i);
                8
            } // ORI (Y),i
            0x6B => {
                let i = self.imm8(memory);
                self.ori_u_i(memory, i);
                8
            } // ORI (U),i
            0xEB => {
                let addr = self.imm16(memory);
                let i = self.imm8(memory);
                self.ori_ab_i(memory, addr, i);
                10
            } // ORI (ab),i

            // EAI - Extended accumulator instruction
            0xBD => {
                self.eai_a(memory);
                5
            } // EAI

            // V-instructions (Vector/interrupt related)
            0xC3 => {
                self.vcs(memory);
                5
            } // VCS
            0xC5 => {
                self.vvs(memory);
                5
            } // VVS
            0xC7 => {
                self.vhs(memory);
                5
            } // VHS
            0xC9 => {
                self.vzr(memory);
                5
            } // VZR (note: conflicts with VHR in table)
            0xCB => {
                self.vzs(memory);
                5
            } // VZS
            0xCD => {
                self.vmj(memory);
                5
            } // VMJ

            // Temporal: NOP para opcodes no implementados aún
            _ => {
                println!("Unknown opcode: {:02X}", opcode);
                5 // NOP timing - cycles por instrucción hasta implementar cada una
            }
        }
    }

    // === DECODIFICADORES DE PREFIJOS MULTI-BYTE ===

    fn execute_fd_prefix(&mut self, memory: &mut MemoryBus) -> u8 {
        let second_byte = self.imm8(memory);

        match second_byte {
            // ADC #(X), #(Y), #(U), #(ab) - Add with carry indirect
            0x03 => {
                let val = self.read_me1(memory, self.x);
                self.a = self.adc_with_flags(val);
                9
            } // ADC #(X)
            0x13 => {
                let val = self.read_me1(memory, self.y);
                self.a = self.adc_with_flags(val);
                9
            } // ADC #(Y)
            0x23 => {
                let val = self.read_me1(memory, self.u);
                self.a = self.adc_with_flags(val);
                9
            } // ADC #(U)
            0xA3 => {
                let val = self.read_ab_me1(memory);
                self.a = self.adc_with_flags(val);
                10
            } // ADC #(ab)

            // ADI #(X), #(Y), #(U), #(ab) - Add immediate indirect
            0x4F => {
                let i = self.imm8(memory);
                self.adi_indirect_x_i(memory, i);
                11
            } // ADI #(X),i
            0x5F => {
                let i = self.imm8(memory);
                self.adi_indirect_y_i(memory, i);
                11
            } // ADI #(Y),i
            0x6F => {
                let i = self.imm8(memory);
                self.adi_indirect_u_i(memory, i);
                11
            } // ADI #(U),i
            0xEF => {
                let addr = self.imm16(memory);
                let i = self.imm8(memory);
                self.adi_indirect_ab_i(memory, addr, i);
                13
            } // ADI #(ab),i

            // ADR - Add register
            0xCA => {
                self.adr_x();
                6
            } // ADR X
            0xDA => {
                self.adr_y();
                6
            } // ADR Y
            0xEA => {
                self.adr_u();
                6
            } // ADR U

            // AEX - Exchange accumulator
            0xF1 => {
                self.aex();
                5
            } // AEX

            // AND #(X), #(Y), #(U), #(ab) - AND indirect
            0x09 => {
                let val = self.read_me1(memory, self.x);
                self.a = self.and_with_flags(val);
                9
            } // AND #(X)
            0x19 => {
                let val = self.read_me1(memory, self.y);
                self.a = self.and_with_flags(val);
                9
            } // AND #(Y)
            0x29 => {
                let val = self.read_me1(memory, self.u);
                self.a = self.and_with_flags(val);
                9
            } // AND #(U)
            0xA9 => {
                let val = self.read_ab_me1(memory);
                self.a = self.and_with_flags(val);
                10
            } // AND #(ab)

            // ANI #(X), #(Y), #(U), #(ab) - AND immediate indirect
            0x49 => {
                let i = self.imm8(memory);
                self.ani_indirect_x_i(memory, i);
                11
            } // ANI #(X),i
            0x59 => {
                let i = self.imm8(memory);
                self.ani_indirect_y_i(memory, i);
                11
            } // ANI #(Y),i
            0x69 => {
                let i = self.imm8(memory);
                self.ani_indirect_u_i(memory, i);
                11
            } // ANI #(U),i
            0xE9 => {
                let addr = self.imm16(memory);
                let i = self.imm8(memory);
                self.ani_indirect_ab_i(memory, addr, i);
                13
            } // ANI #(ab),i

            // AM0/AM1 - Address mode
            // 0xCE removed - conflicts with VEJ CE opcode
            0xDE => self.am1(memory), // AM1 - returns cycles

            // ATP/ATT - Address transfer
            // 0xCC removed - conflicts with VEJ CC opcode
            0xEC => {
                self.att();
                8
            } // ATT

            // BIT #(X), #(Y), #(U), #(ab) - Bit test indirect
            0x0F => {
                self.bit_indirect_x(memory);
                9
            } // BIT #(X)
            0x1F => {
                self.bit_indirect_y(memory);
                9
            } // BIT #(Y)
            0x2F => {
                self.bit_indirect_u(memory);
                9
            } // BIT #(U)
            0xAF => {
                let addr = self.imm16(memory);
                self.bit_indirect_ab(memory, addr);
                10
            } // BIT #(ab)

            // BII #(X), #(Y), #(U), #(ab) - Branch if input indirect
            0x4D => {
                let i = self.imm8(memory);
                self.bii_indirect_x_i(memory, i);
                11
            } // BII #(X),i
            0x5D => {
                let i = self.imm8(memory);
                self.bii_indirect_y_i(memory, i);
                11
            } // BII #(Y),i
            0x6D => {
                let i = self.imm8(memory);
                self.bii_indirect_u_i(memory, i);
                11
            } // BII #(U),i
            0xED => {
                let addr = self.imm16(memory);
                let i = self.imm8(memory);
                self.bii_indirect_ab_i(memory, addr, i);
                13
            } // BII #(ab),i

            // CDV - Clear divide
            0x8E => self.cdv(), // CDV - returns cycles

            // CIN - Clear interrupt
            0xF7 => {
                self.cin();
                5
            } // CIN

            // CPA #(X), #(Y), #(U), #(ab) - Compare accumulator indirect
            0x07 => {
                let val = self.read_me1(memory, self.x);
                self.cpa_with_flags(val);
                9
            } // CPA #(X)
            0x17 => {
                let val = self.read_me1(memory, self.y);
                self.cpa_with_flags(val);
                9
            } // CPA #(Y)
            0x27 => {
                let val = self.read_me1(memory, self.u);
                self.cpa_with_flags(val);
                9
            } // CPA #(U)
            0xA7 => {
                let val = self.read_ab_me1(memory);
                self.cpa_with_flags(val);
                10
            } // CPA #(ab)

            // DCA #(X), #(Y), #(U) - Decimal add indirect
            0x8C => {
                let val = self.read_me1(memory, self.x);
                self.a = self.dca_with_flags(val);
                9
            } // DCA #(X)
            0x9C => {
                let val = self.read_me1(memory, self.y);
                self.a = self.dca_with_flags(val);
                9
            } // DCA #(Y)
            0xAC => {
                let val = self.read_me1(memory, self.u);
                self.a = self.dca_with_flags(val);
                9
            } // DCA #(U)

            // DCS #(X), #(Y), #(U) - Decimal subtract indirect
            0x0C => {
                let val = self.read_me1(memory, self.x);
                self.a = self.dcs_with_flags(val);
                9
            } // DCS #(X)
            0x1C => {
                let val = self.read_me1(memory, self.y);
                self.a = self.dcs_with_flags(val);
                9
            } // DCS #(Y)
            0x2C => {
                let val = self.read_me1(memory, self.u);
                self.a = self.dcs_with_flags(val);
                9
            } // DCS #(U)

            // INC XH, YH, UH - Increment high register bytes (FD prefixed)
            0x40 => self.inc_xh(memory), // INC XH (FD 40) (5 cycles)
            0x50 => self.inc_yh(memory), // INC YH (FD 50) (5 cycles)
            0x60 => self.inc_uh(memory), // INC UH (FD 60) (5 cycles)

            // DEC XH, YH, UH - Decrement high register bytes (FD prefixed)
            0x42 => self.dec_xh(memory), // DEC XH (FD 42) (5 cycles)
            0x52 => self.dec_yh(memory), // DEC YH (FD 52) (5 cycles)
            0x62 => self.dec_uh(memory), // DEC UH (FD 62) (5 cycles)

            // ===== TRANSFER AND SEARCH INSTRUCTIONS (FD prefix) =====

            // LDA #(X), #(Y), #(U), #(ab) - Load indirect to accumulator
            0x05 => self.lda_rreg_indirect(memory, self.x), // LDA #(X)
            0x15 => self.lda_rreg_indirect(memory, self.y), // LDA #(Y)
            0x25 => self.lda_rreg_indirect(memory, self.u), // LDA #(U)
            0xA5 => self.lda_ab_indirect(memory),           // LDA #(ab)

            // LDX X,Y,U,S,P - Load exchange
            0x08 => {
                let y_val = self.y;
                self.x = y_val;
                5
            } // LDX X (load Y to X)
            0x18 => {
                let x_val = self.x;
                self.y = x_val;
                5
            } // LDX Y (load X to Y)
            0x28 => {
                let x_val = self.x;
                self.u = x_val;
                5
            } // LDX U (load X to U)
            0x48 => {
                self.s = self.x;
                5
            } // LDX S (load X to S)
            0x58 => {
                self.p = self.x;
                5
            } // LDX P (load X to P)

            // STA #(X), #(Y), #(U), #(ab) - Store indirect from accumulator
            0x0E => self.sta_rreg_indirect(memory, self.x), // STA #(X)
            0x1E => self.sta_rreg_indirect(memory, self.y), // STA #(Y)
            0x2E => self.sta_rreg_indirect(memory, self.u), // STA #(U)
            0xAE => {
                let addr = self.imm16(memory);
                self.sta_ab_indirect(memory, addr)
            } // STA #(ab)

            // STX X,Y,U,S,P - Store exchange
            0x4A => {
                let x_val = self.x;
                self.y = x_val;
                5
            } // STX X (store X to Y)
            0x5A => {
                let y_val = self.y;
                self.x = y_val;
                5
            } // STX Y (store Y to X)
            0x6A => {
                let u_val = self.u;
                self.x = u_val;
                5
            } // STX U (store U to X)
            0x4E => {
                self.s = self.x;
                5
            } // STX S (store X to S)
            0x5E => {
                self.p = self.x;
                5
            } // STX P (store X to P)

            // PSH A,X,Y,U - Push to stack (official PC-1500 manual opcodes)
            0xC8 => self.psh_a(memory), // PSH A (FD C8)
            0x88 => self.psh_rreg(memory, self.xh(), self.xl()), // PSH X (FD 88)
            0x98 => self.psh_rreg(memory, self.yh(), self.yl()), // PSH Y (FD 98)
            0xA8 => self.psh_rreg(memory, self.uh(), self.ul()), // PSH U (FD A8)

            // POP A,X,Y,U - Pop from stack (corrected opcodes from manual)
            0x8A => self.pop_a(memory), // POP A (FD 8A)
            0x0A => self.pop_rreg(
                memory,
                |cpu, val| cpu.set_xl(val),
                |cpu, val| cpu.set_xh(val),
            ), // POP X (FD 0A)
            0x1A => self.pop_rreg(
                memory,
                |cpu, val| cpu.set_yl(val),
                |cpu, val| cpu.set_yh(val),
            ), // POP Y (FD 1A)
            0x2A => self.pop_rreg(
                memory,
                |cpu, val| cpu.set_ul(val),
                |cpu, val| cpu.set_uh(val),
            ), // POP U (FD 2A)

            // ATT/TTA - Accumulator/T register transfer
            // 0xBA removed - conflicts with JMP instruction
            0xAA => self.tta(), // TTA (T register to A)

            // DRL # - Digit rotate left (indirect addressing)
            0xD7 => self.drl_xreg_indirect(memory), // DRL #(X) (16 cycles)

            // DRR # - Digit rotate right (indirect addressing)
            0xD3 => self.drr_xreg_indirect(memory), // DRR #(X) (16 cycles)

            // CPU Control Instructions (FD prefix)
            0xCC => self.atp(memory), // ATP - A to output port (FD CC) (9 cycles)
            0x4C => self.off(),       // OFF - Turn off system (FD 4C) (8 cycles)
            0xC0 => self.rdp(),       // RDP - Reset display (8 cycles)
            0xB6 => self.ita(memory), // ITA - Input to accumulator (9 cycles)
            0x81 => self.sie(),       // SIE - Set interrupt enable (8 cycles)
            0x9E => self.rie(),       // RIE - Reset interrupt enable (8 cycles)
            0xB1 => self.hlt(),       // HLT - Halt (9 cycles)

            // ===== MISSING FD PREFIX INSTRUCTIONS =====

            // SBC #(X), #(Y), #(U), #(ab) - Subtract with carry indirect
            0x01 => {
                let val = self.read_me1(memory, self.x);
                self.a = self.sbc_with_flags(val);
                9
            } // SBC #(X)
            0x11 => {
                let val = self.read_me1(memory, self.y);
                self.a = self.sbc_with_flags(val);
                9
            } // SBC #(Y)
            0x21 => {
                let val = self.read_me1(memory, self.u);
                self.a = self.sbc_with_flags(val);
                9
            } // SBC #(U)
            0xA1 => {
                let val = self.read_ab_me1(memory);
                self.a = self.sbc_with_flags(val);
                10
            } // SBC #(ab)

            // EOR #(X), #(Y), #(U), #(ab) - Exclusive OR indirect
            0x0D => {
                let val = self.read_me1(memory, self.x);
                self.a = self.xor_with_flags(val);
                9
            } // EOR #(X)
            0x1D => {
                let val = self.read_me1(memory, self.y);
                self.a = self.xor_with_flags(val);
                9
            } // EOR #(Y)
            0x2D => {
                let val = self.read_me1(memory, self.u);
                self.a = self.xor_with_flags(val);
                9
            } // EOR #(U)
            0xAD => {
                let val = self.read_ab_me1(memory);
                self.a = self.xor_with_flags(val);
                10
            } // EOR #(ab)

            // ORI #(X), #(Y), #(U), #(ab) - OR immediate indirect
            0x4B => {
                let i = self.imm8(memory);
                self.ori_indirect_x_i(memory, i);
                11
            } // ORI #(X),i
            0x5B => {
                let i = self.imm8(memory);
                self.ori_indirect_y_i(memory, i);
                11
            } // ORI #(Y),i
            0x6B => {
                let i = self.imm8(memory);
                self.ori_indirect_u_i(memory, i);
                11
            } // ORI #(U),i
            0xEB => {
                let addr = self.imm16(memory);
                let i = self.imm8(memory);
                self.ori_indirect_ab_i(memory, addr, i);
                13
            } // ORI #(ab),i

            // RIE - Reset interrupt enable
            0xBE => {
                self.rie();
                8
            } // RIE

            // AM0/LDX - Load/store operations
            0xCE => {
                self.am0(memory);
                8
            } // AM0 - Address mode 0
            0xD8 => {
                self.ldx_x();
                8
            } // LDX X - Load X register

            // Instrucciones FD no reconocidas
            _ => {
                println!("Unknown FD prefix instruction: FD {:02X}", second_byte);
                5
            }
        }
    }

    // === IMPLEMENTACIÓN DE INSTRUCCIONES PASO A PASO ===
    // A partir de aquí implementaremos cada instrucción del manual PC-1500
    // siguiendo el patrón del GameBoy CPU pero con la funcionalidad específica
    // del LH5801 que usa el PC-1500
    //
    // PROCESO:
    // 1. Mirar el manual PC-1500
    // 2. Encontrar la primera instrucción
    // 3. Implementar exactamente como aparece en el manual
    // 4. Compilar y probar
    // 5. Repetir para la siguiente instrucción
    //
    // READY PARA EMPEZAR IMPLEMENTACIÓN MANUAL

    // === INSTRUCCIONES ARITMÉTICAS IMPLEMENTADAS SEGÚN MANUAL PC-1500 ===

    // ① ADC (ADd with Carry) - A + [operand] + C -> A
    // C, H, Z, and V may change

    fn adc_rl(&mut self, _memory: &mut MemoryBus) -> u8 {
        let rl = self.xl(); // RL puede ser XL, YL, o UL según contexto
        self.a = self.adc_with_flags(rl);
        5 // Cycles
    }

    fn adc_rh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let rh = self.xh(); // RH puede ser XH, YH, o UH según contexto
        self.a = self.adc_with_flags(rh);
        5 // Cycles
    }

    fn adc_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x; // Rreg puede ser X, Y, o U según contexto
        let val = self.read_me0(memory, rreg);
        self.a = self.adc_with_flags(val);
        7 // Cycles
    }

    fn adc_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x; // Rreg puede ser X, Y, o U según contexto
        let val = self.read_me1(memory, rreg);
        self.a = self.adc_with_flags(val);
        9 // Cycles
    }

    fn adc_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me0(memory);
        self.a = self.adc_with_flags(val);
        8 // Cycles
    }

    fn adc_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me1(memory);
        self.a = self.adc_with_flags(val);
        10 // Cycles
    }

    // ② ADI (ADd Immediate) - A + i + C -> A or [operand] + i -> [operand]
    // C, H, Z, and V may change

    fn adi_a(&mut self, memory: &mut MemoryBus) -> u8 {
        let immediate = self.imm8(memory);
        self.a = self.adc_with_flags(immediate);
        6 // Cycles
    }

    fn adi_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x; // Rreg puede ser X, Y, o U según contexto  
        let immediate = self.imm8(memory);
        let val = self.read_me0(memory, rreg);
        let result = val.wrapping_add(immediate);
        self.write_me0(memory, rreg, result);

        // Set flags based on operation
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);

        9 // Cycles
    }

    fn adi_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let immediate = self.imm8(memory);
        let val = self.read_me1(memory, rreg);
        let result = val.wrapping_add(immediate);
        self.write_me1(memory, rreg, result);

        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);

        11 // Cycles
    }

    fn adi_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        let immediate = self.imm8(memory);
        let val = self.read(memory, addr);
        let result = val.wrapping_add(immediate);
        self.write(memory, addr, result);

        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);

        11 // Cycles
    }

    fn adi_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        let immediate = self.imm8(memory);
        let target_addr = self.read_word_me0(memory, addr);
        let val = self.read(memory, target_addr);
        let result = val.wrapping_add(immediate);
        self.write(memory, target_addr, result);

        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);

        13 // Cycles
    }

    // ③ DCA (DeCimal Add) - Decimal addition
    // C, H, Z, and V may change

    fn dca_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me0(memory, rreg);
        self.a = self.dca_with_flags(val);
        7 // Cycles
    }

    fn dca_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me1(memory, rreg);
        self.a = self.dca_with_flags(val);
        9 // Cycles
    }

    // ④ ADR (ADd Rreg) - Contents of accumulator are added to R register
    // C, H, Z, and V may change

    fn adr_rreg(&mut self, _memory: &mut MemoryBus) -> u8 {
        let a_val = self.a;
        let x_val = self.x;
        let result = x_val.wrapping_add(u16::from(a_val));
        self.x = result;

        // Set flags para operación 16-bit
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u32::from(x_val) + u32::from(a_val) > 0xFFFF);

        6 // Cycles
    }

    // ⑤ SBC (SuBtract with Carry) - A - [operand] - C -> A
    // C, H, Z, and V may change

    fn sbc_rl(&mut self, _memory: &mut MemoryBus) -> u8 {
        let rl = self.xl();
        self.a = self.sbc_with_flags(rl);
        5 // Cycles
    }

    fn sbc_rh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let rh = self.xh();
        self.a = self.sbc_with_flags(rh);
        5 // Cycles
    }

    fn sbc_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me0(memory, rreg);
        self.a = self.sbc_with_flags(val);
        7 // Cycles
    }

    fn sbc_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me1(memory, rreg);
        self.a = self.sbc_with_flags(val);
        9 // Cycles
    }

    fn sbc_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me0(memory);
        self.a = self.sbc_with_flags(val);
        8 // Cycles
    }

    fn sbc_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me1(memory);
        self.a = self.sbc_with_flags(val);
        10 // Cycles
    }

    // ⑥ SBI (SuBtract Immediate) - A - i - C -> A
    // C, H, Z, and V may change

    fn sbi_a(&mut self, memory: &mut MemoryBus) -> u8 {
        let immediate = self.imm8(memory);
        self.a = self.sbc_with_flags(immediate);
        6 // Cycles
    }

    // ⑦ DCS (DeCimal Subtract) - Decimal subtraction
    // C, H, Z, and V may change

    fn dcs_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me0(memory, rreg);
        self.a = self.dcs_with_flags(val);
        7 // Cycles
    }

    fn dcs_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me1(memory, rreg);
        self.a = self.dcs_with_flags(val);
        9 // Cycles
    }

    // ⑧ AND - A ∧ [operand] -> A (Only flag Z changes)

    fn and_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me0(memory, rreg);
        self.a = self.and_with_flags(val);
        7 // Cycles
    }

    fn and_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me1(memory, rreg);
        self.a = self.and_with_flags(val);
        9 // Cycles
    }

    fn and_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me0(memory);
        self.a = self.and_with_flags(val);
        8 // Cycles
    }

    fn and_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me1(memory);
        self.a = self.and_with_flags(val);
        10 // Cycles
    }

    // ⑨ ANI (AND Immediate) - [operand] ∧ i -> [operand] (Flag Z changes)

    fn ani_a(&mut self, memory: &mut MemoryBus) -> u8 {
        let immediate = self.imm8(memory);
        self.a = self.and_with_flags(immediate);
        6 // Cycles
    }

    fn ani_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let immediate = self.imm8(memory);
        let val = self.read_me0(memory, rreg);
        let result = val & immediate;
        self.write_me0(memory, rreg, result);
        self.set_zero_flag(result == 0);
        9 // Cycles
    }

    fn ani_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let immediate = self.imm8(memory);
        let val = self.read_me1(memory, rreg);
        let result = val & immediate;
        self.write_me1(memory, rreg, result);
        self.set_zero_flag(result == 0);
        11 // Cycles
    }

    fn ani_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        let immediate = self.imm8(memory);
        let val = self.read(memory, addr);
        let result = val & immediate;
        self.write(memory, addr, result);
        self.set_zero_flag(result == 0);
        11 // Cycles
    }

    fn ani_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        let immediate = self.imm8(memory);
        let target_addr = self.read_word_me0(memory, addr);
        let val = self.read(memory, target_addr);
        let result = val & immediate;
        self.write(memory, target_addr, result);
        self.set_zero_flag(result == 0);
        13 // Cycles
    }

    // ⑩ ORA (OR Acc) - A ∨ [operand] -> A (Only flag Z changes)

    fn ora_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me0(memory, rreg);
        self.a = self.or_with_flags(val);
        7 // Cycles
    }

    fn ora_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me1(memory, rreg);
        self.a = self.or_with_flags(val);
        9 // Cycles
    }

    fn ora_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me0(memory);
        self.a = self.or_with_flags(val);
        8 // Cycles
    }

    fn ora_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me1(memory);
        self.a = self.or_with_flags(val);
        10 // Cycles
    }

    // ⑪ ORI (OR Immediate) - [operand] ∨ i -> [operand] (Only flag Z changes)

    fn ori_a(&mut self, memory: &mut MemoryBus) -> u8 {
        let immediate = self.imm8(memory);
        self.a = self.or_with_flags(immediate);
        6 // Cycles
    }

    fn ori_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let immediate = self.imm8(memory);
        let val = self.read_me0(memory, rreg);
        let result = val | immediate;
        self.write_me0(memory, rreg, result);
        self.set_zero_flag(result == 0);
        9 // Cycles
    }

    fn ori_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let immediate = self.imm8(memory);
        let val = self.read_me1(memory, rreg);
        let result = val | immediate;
        self.write_me1(memory, rreg, result);
        self.set_zero_flag(result == 0);
        11 // Cycles
    }

    fn ori_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        let immediate = self.imm8(memory);
        let val = self.read(memory, addr);
        let result = val | immediate;
        self.write(memory, addr, result);
        self.set_zero_flag(result == 0);
        11 // Cycles
    }

    fn ori_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        let immediate = self.imm8(memory);
        let target_addr = self.read_word_me0(memory, addr);
        let val = self.read(memory, target_addr);
        let result = val | immediate;
        self.write(memory, target_addr, result);
        self.set_zero_flag(result == 0);
        13 // Cycles
    }

    // ORI operations for memory locations with immediate
    fn ori_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.x);
        let result = val | immediate;
        self.write_me0(memory, self.x, result);
        self.set_zero_flag(result == 0);
    }

    fn ori_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.y);
        let result = val | immediate;
        self.write_me0(memory, self.y, result);
        self.set_zero_flag(result == 0);
    }

    fn ori_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.u);
        let result = val | immediate;
        self.write_me0(memory, self.u, result);
        self.set_zero_flag(result == 0);
    }

    fn ori_ab_i(&mut self, memory: &mut MemoryBus, addr: u16, immediate: u8) {
        let val = self.read(memory, addr);
        let result = val | immediate;
        self.write(memory, addr, result);
        self.set_zero_flag(result == 0);
    }

    // ORI indirect operations
    fn ori_indirect_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.x);
        let result = val | immediate;
        self.write_me1(memory, self.x, result);
        self.set_zero_flag(result == 0);
    }

    fn ori_indirect_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.y);
        let result = val | immediate;
        self.write_me1(memory, self.y, result);
        self.set_zero_flag(result == 0);
    }

    fn ori_indirect_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.u);
        let result = val | immediate;
        self.write_me1(memory, self.u, result);
        self.set_zero_flag(result == 0);
    }

    fn ori_indirect_ab_i(&mut self, memory: &mut MemoryBus, addr: u16, immediate: u8) {
        let target_addr = self.read_word_me0(memory, addr);
        let val = self.read(memory, target_addr);
        let result = val | immediate;
        self.write(memory, target_addr, result);
        self.set_zero_flag(result == 0);
    }

    // ⑫ EOR (Exclusive OR) - A ⊕ [operand] -> A (Only flag Z changes)

    fn eor_rreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me0(memory, rreg);
        self.a = self.xor_with_flags(val);
        7 // Cycles
    }

    fn eor_rreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let rreg = self.x;
        let val = self.read_me1(memory, rreg);
        self.a = self.xor_with_flags(val);
        9 // Cycles
    }

    fn eor_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me0(memory);
        self.a = self.xor_with_flags(val);
        8 // Cycles
    }

    fn eor_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me1(memory);
        self.a = self.xor_with_flags(val);
        10 // Cycles
    }

    // ⑬ EAI (Exclusive Acc and Immediate) - A ⊕ i -> A (Only flag Z changes)

    fn eai_a(&mut self, memory: &mut MemoryBus) -> u8 {
        let immediate = self.imm8(memory);
        self.a = self.xor_with_flags(immediate);
        6 // Cycles
    }

    // ⑭ INC (INCrement) - [Operand] + 1 -> [operand]
    // Para 8-bit registers (A, RL, RH): C, V, H and Z changed
    // Para 16-bit registers (Rreg): no flag change

    fn inc_a(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.a = self.inc_with_flags(self.a);
        5 // Cycles
    }

    fn inc_rl(&mut self, _memory: &mut MemoryBus) -> u8 {
        let rl = self.xl();
        let result = self.inc_with_flags(rl);
        self.set_xl(result);
        5 // Cycles
    }

    fn inc_rh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let rh = self.xh();
        let result = self.inc_with_flags(rh);
        self.set_xh(result);
        5 // Cycles
    }

    fn inc_rreg(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.x = self.x.wrapping_add(1);
        // No flag change takes place para 16-bit registers
        6 // Cycles
    }

    // ⑮ DEC (DECrement) - [Operand] - 1 -> [operand]
    // Para 8-bit registers (A, RL, RH): C, V, H and Z changed
    // Para 16-bit registers (Rreg): no flag change

    fn dec_a(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.a = self.dec_with_flags(self.a);
        5 // Cycles
    }

    fn dec_rl(&mut self, _memory: &mut MemoryBus) -> u8 {
        let rl = self.xl();
        let result = self.dec_with_flags(rl);
        self.set_xl(result);
        5 // Cycles
    }

    fn dec_rh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let rh = self.xh();
        let result = self.dec_with_flags(rh);
        self.set_xh(result);
        5 // Cycles
    }

    fn dec_rreg(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.x = self.x.wrapping_sub(1);
        // No flag change takes place para 16-bit registers
        6 // Cycles
    }

    // Specific register increment functions
    fn inc_xl(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.inc_with_flags(self.xl());
        self.set_xl(result);
        5
    }

    fn inc_yl(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.inc_with_flags(self.yl());
        self.set_yl(result);
        5
    }

    fn inc_ul(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.inc_with_flags(self.ul());
        self.set_ul(result);
        5
    }

    fn inc_xh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.inc_with_flags(self.xh());
        self.set_xh(result);
        5
    }

    fn inc_yh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.inc_with_flags(self.yh());
        self.set_yh(result);
        5
    }

    fn inc_uh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.inc_with_flags(self.uh());
        self.set_uh(result);
        5
    }

    fn inc_x(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.x = self.x.wrapping_add(1);
        6
    }

    fn inc_y(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.y = self.y.wrapping_add(1);
        6
    }

    fn inc_u(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.u = self.u.wrapping_add(1);
        6
    }

    // Specific register decrement functions
    fn dec_xl(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.dec_with_flags(self.xl());
        self.set_xl(result);
        5
    }

    fn dec_yl(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.dec_with_flags(self.yl());
        self.set_yl(result);
        5
    }

    fn dec_ul(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.dec_with_flags(self.ul());
        self.set_ul(result);
        5
    }

    fn dec_xh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.dec_with_flags(self.xh());
        self.set_xh(result);
        5
    }

    fn dec_yh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.dec_with_flags(self.yh());
        self.set_yh(result);
        5
    }

    fn dec_uh(&mut self, _memory: &mut MemoryBus) -> u8 {
        let result = self.dec_with_flags(self.uh());
        self.set_uh(result);
        5
    }

    fn dec_x(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.x = self.x.wrapping_sub(1);
        6
    }

    fn dec_y(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.y = self.y.wrapping_sub(1);
        6
    }

    fn dec_u(&mut self, _memory: &mut MemoryBus) -> u8 {
        self.u = self.u.wrapping_sub(1);
        6
    }

    // === INSTRUCCIONES AUXILIARES IMPLEMENTADAS SEGÚN MANUAL ===

    // ADI operations for memory locations
    fn adi_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.x);
        let result = val.wrapping_add(immediate);
        self.write_me0(memory, self.x, result);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);
    }

    fn adi_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.y);
        let result = val.wrapping_add(immediate);
        self.write_me0(memory, self.y, result);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);
    }

    fn adi_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.u);
        let result = val.wrapping_add(immediate);
        self.write_me0(memory, self.u, result);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);
    }

    fn adi_ab_i(&mut self, memory: &mut MemoryBus, addr: u16, immediate: u8) {
        let val = self.read(memory, addr);
        let result = val.wrapping_add(immediate);
        self.write(memory, addr, result);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);
    }

    // ADI indirect operations
    fn adi_indirect_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.x);
        let result = val.wrapping_add(immediate);
        self.write_me1(memory, self.x, result);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);
    }

    fn adi_indirect_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.y);
        let result = val.wrapping_add(immediate);
        self.write_me1(memory, self.y, result);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);
    }

    fn adi_indirect_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.u);
        let result = val.wrapping_add(immediate);
        self.write_me1(memory, self.u, result);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);
    }

    fn adi_indirect_ab_i(&mut self, memory: &mut MemoryBus, addr: u16, immediate: u8) {
        let target_addr = self.read_word_me0(memory, addr);
        let val = self.read(memory, target_addr);
        let result = val.wrapping_add(immediate);
        self.write(memory, target_addr, result);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(u16::from(val) + u16::from(immediate) > 0xFF);
    }

    // ANI operations
    fn ani_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.x);
        let result = val & immediate;
        self.write_me0(memory, self.x, result);
        self.set_zero_flag(result == 0);
    }

    fn ani_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.y);
        let result = val & immediate;
        self.write_me0(memory, self.y, result);
        self.set_zero_flag(result == 0);
    }

    fn ani_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.u);
        let result = val & immediate;
        self.write_me0(memory, self.u, result);
        self.set_zero_flag(result == 0);
    }

    fn ani_ab_i(&mut self, memory: &mut MemoryBus, addr: u16, immediate: u8) {
        let val = self.read(memory, addr);
        let result = val & immediate;
        self.write(memory, addr, result);
        self.set_zero_flag(result == 0);
    }

    // ANI indirect operations
    fn ani_indirect_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.x);
        let result = val & immediate;
        self.write_me1(memory, self.x, result);
        self.set_zero_flag(result == 0);
    }

    fn ani_indirect_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.y);
        let result = val & immediate;
        self.write_me1(memory, self.y, result);
        self.set_zero_flag(result == 0);
    }

    fn ani_indirect_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.u);
        let result = val & immediate;
        self.write_me1(memory, self.u, result);
        self.set_zero_flag(result == 0);
    }

    fn ani_indirect_ab_i(&mut self, memory: &mut MemoryBus, addr: u16, immediate: u8) {
        let target_addr = self.read_word_me0(memory, addr);
        let val = self.read(memory, target_addr);
        let result = val & immediate;
        self.write(memory, target_addr, result);
        self.set_zero_flag(result == 0);
    }

    // BIT operations
    fn bit_x(&mut self) {
        // TODO: Implement bit test for X register
        self.set_zero_flag(self.x == 0);
    }

    fn bit_y(&mut self) {
        // TODO: Implement bit test for Y register
        self.set_zero_flag(self.y == 0);
    }

    fn bit_u(&mut self) {
        // TODO: Implement bit test for U register
        self.set_zero_flag(self.u == 0);
    }

    fn bit_ab(&mut self, memory: &mut MemoryBus, addr: u16) {
        let val = self.read(memory, addr);
        self.set_zero_flag(val == 0);
    }

    fn bit_indirect_x(&mut self, memory: &mut MemoryBus) {
        let val = self.read_me1(memory, self.x);
        self.set_zero_flag(val == 0);
    }

    fn bit_indirect_y(&mut self, memory: &mut MemoryBus) {
        let val = self.read_me1(memory, self.y);
        self.set_zero_flag(val == 0);
    }

    fn bit_indirect_u(&mut self, memory: &mut MemoryBus) {
        let val = self.read_me1(memory, self.u);
        self.set_zero_flag(val == 0);
    }

    fn bit_indirect_ab(&mut self, memory: &mut MemoryBus, addr: u16) {
        let target_addr = self.read_word_me0(memory, addr);
        let val = self.read(memory, target_addr);
        self.set_zero_flag(val == 0);
    }

    // Branch operation
    fn branch(&mut self, offset: i8) {
        self.p = ((self.p as i32) + (offset as i32)) as u16;
    }

    // BII operations (Branch if Input)
    fn bii_a_i(&mut self, _memory: &mut MemoryBus, immediate: u8) {
        // TODO: Implement based on input port status
        if (self.a & immediate) != 0 {
            // Branch logic would go here
        }
    }

    fn bii_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.x);
        if (val & immediate) != 0 {
            // Branch logic would go here
        }
    }

    fn bii_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.y);
        if (val & immediate) != 0 {
            // Branch logic would go here
        }
    }

    fn bii_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.u);
        if (val & immediate) != 0 {
            // Branch logic would go here
        }
    }

    fn bii_ab_i(&mut self, memory: &mut MemoryBus, addr: u16, immediate: u8) {
        let val = self.read(memory, addr);
        if (val & immediate) != 0 {
            // Branch logic would go here
        }
    }

    // BII indirect operations
    fn bii_indirect_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.x);
        if (val & immediate) != 0 {
            // Branch logic would go here
        }
    }

    fn bii_indirect_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.y);
        if (val & immediate) != 0 {
            // Branch logic would go here
        }
    }

    fn bii_indirect_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me1(memory, self.u);
        if (val & immediate) != 0 {
            // Branch logic would go here
        }
    }

    fn bii_indirect_ab_i(&mut self, memory: &mut MemoryBus, addr: u16, immediate: u8) {
        let target_addr = self.read_word_me0(memory, addr);
        let val = self.read(memory, target_addr);
        if (val & immediate) != 0 {
            // Branch logic would go here
        }
    }

    // Control operations
    fn cin(&mut self) {
        // Clear interrupt flag
        self.interrupt_enabled = false;
    }

    // Compare operations
    fn cpa_with_flags(&mut self, val: u8) {
        let a = self.a;
        let result = a.wrapping_sub(val);

        // Set flags like subtraction but don't store result
        self.set_zero_flag(result == 0);
        self.set_carry_flag(a < val);
        self.set_half_carry_flag((a & 0x0F) < (val & 0x0F));

        let overflow = (a & 0x80) != (val & 0x80) && (a & 0x80) != (result & 0x80);
        self.set_overflow_flag(overflow);
    }

    // CPI operations
    fn cpi_x_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.x);
        let result = val.wrapping_sub(immediate);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(val < immediate);
    }

    fn cpi_y_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.y);
        let result = val.wrapping_sub(immediate);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(val < immediate);
    }

    fn cpi_u_i(&mut self, memory: &mut MemoryBus, immediate: u8) {
        let val = self.read_me0(memory, self.u);
        let result = val.wrapping_sub(immediate);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(val < immediate);
    }

    fn cpi_xh_i(&mut self, immediate: u8) {
        let val = self.xh();
        let result = val.wrapping_sub(immediate);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(val < immediate);
    }

    fn cpi_yh_i(&mut self, immediate: u8) {
        let val = self.yh();
        let result = val.wrapping_sub(immediate);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(val < immediate);
    }

    fn cpi_uh_i(&mut self, immediate: u8) {
        let val = self.uh();
        let result = val.wrapping_sub(immediate);
        self.set_zero_flag(result == 0);
        self.set_carry_flag(val < immediate);
    }

    // ADR operations
    fn adr_x(&mut self) {
        let a_val = self.a;
        let result = self.x.wrapping_add(u16::from(a_val));
        self.x = result;

        self.set_zero_flag(result == 0);
        self.set_carry_flag(u32::from(self.x) + u32::from(a_val) > 0xFFFF);
    }

    fn adr_y(&mut self) {
        let a_val = self.a;
        let result = self.y.wrapping_add(u16::from(a_val));
        self.y = result;

        self.set_zero_flag(result == 0);
        self.set_carry_flag(u32::from(self.y) + u32::from(a_val) > 0xFFFF);
    }

    fn adr_u(&mut self) {
        let a_val = self.a;
        let result = self.u.wrapping_add(u16::from(a_val));
        self.u = result;

        self.set_zero_flag(result == 0);
        self.set_carry_flag(u32::from(self.u) + u32::from(a_val) > 0xFFFF);
    }

    // ========================================================================
    // CONTROL INSTRUCTIONS (moved from control_instructions.rs)
    // ========================================================================

    // ========================================================================
    // FLAG CONTROL INSTRUCTIONS
    // ========================================================================

    /// SEC (Set Carry) - Sets the carry flag
    /// Format: SEC
    /// Operation: 1 → C
    /// Opcode: 0xFB (11111011)
    /// Cycles: 4 (from LH5801 documentation)
    /// Flags: Sets Carry flag, no change to other flags
    pub(super) fn sec(&mut self) -> u8 {
        self.set_carry_flag(true);
        4 // 4 cycles according to documentation
    }

    /// REC (Reset Carry) - Resets the carry flag
    /// Format: REC  
    /// Operation: 0 → C
    /// Opcode: 0xF9 (11111001)
    /// Cycles: 4 (from LH5801 documentation)
    /// Flags: Clears Carry flag, no change to other flags
    pub(super) fn rec(&mut self) -> u8 {
        self.set_carry_flag(false);
        4 // 4 cycles according to documentation
    }

    // ========================================================================
    // SYSTEM CONTROL INSTRUCTIONS
    // ========================================================================

    /// CDV (Clear Divider) - Clears the internal divider
    /// Format: CDV
    /// Operation: 0 → divider
    /// Opcode: FD CE (0xFD 0xCE)
    /// Cycles: 8 (from LH5801 documentation)
    /// Notes: Makes clock reset by the CDV instruction since CPU clock is supplied through divider
    pub(super) fn cdv(&mut self) -> u8 {
        // Clear internal clock divider
        // This would affect system timing in real hardware
        // For emulation, we just acknowledge the operation
        8 // 8 cycles according to documentation
    }

    /// ATP (Acc To Port) - Sends accumulator contents to output port
    /// Format: ATP
    /// Operation: A → Output port (with clock Pφ)
    /// Cycles: 9 (from PC-1500 manual)
    /// Notes: Contents of accumulator sent on data bus, may be used for latch IC clock
    pub(super) fn atp(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        // Send accumulator value to output port
        // In PC-1500, this is used for controlling external devices
        memory.write_output_port(self.a);
        9 // 9 cycles (corrected from PC-1500 manual)
    }

    // ========================================================================
    // GENERAL PURPOSE FLIPFLOP CONTROL
    // ========================================================================

    /// SPU (Set PU) - Sets the general purpose flipflop PU
    /// Format: SPU
    /// Operation: 1 → PU
    /// Opcode: 0xE1 (11100001)
    /// Cycles: 4 (from LH5801 documentation)
    /// Notes: No change takes place in flags
    pub(super) fn spu(&mut self) -> u8 {
        // Set PU flipflop (general purpose control bit)
        // This would control hardware features in real PC-1500
        self.pu_flipflop = true;
        4 // 4 cycles according to documentation
    }

    /// RPU (Reset PU) - Resets the general purpose flipflop PU
    /// Format: RPU
    /// Operation: 0 → PU
    /// Opcode: 0xE3 (11100011)
    /// Cycles: 4 (from LH5801 documentation)
    /// Notes: No change takes place in flags
    pub(super) fn rpu(&mut self) -> u8 {
        // Reset PU flipflop
        self.pu_flipflop = false;
        4 // 4 cycles according to documentation
    }

    /// SPV (Set PV) - Sets the general purpose flipflop PV
    /// Format: SPV
    /// Operation: 1 → PV
    /// Cycles: 4 (from PC-1500 manual)
    /// Notes: No change takes place in flags
    pub(super) fn spv(&mut self) -> u8 {
        // Set PV flipflop (general purpose control bit)
        self.pv_flipflop = true;
        4 // 4 cycles (corrected from PC-1500 manual)
    }

    /// RPV (Reset PV) - Resets the general purpose flipflop PV
    /// Format: RPV
    /// Operation: 0 → PV
    /// Cycles: 4 (from PC-1500 manual)
    /// Notes: No change takes place in flags
    pub(super) fn rpv(&mut self) -> u8 {
        // Reset PV flipflop
        self.pv_flipflop = false;
        4 // 4 cycles (corrected from PC-1500 manual)
    }

    // ========================================================================
    // DISPLAY CONTROL INSTRUCTIONS
    // ========================================================================

    /// SDP (Set DisP) - Sets the LCD on/off control flipflop DISP
    /// Format: SDP
    /// Operation: 1 → DISP
    /// Cycles: 8 (from PC-1500 manual)
    /// Notes: On pattern signal generated from CPU internal LCD backplate signal lines (H0-H7)
    pub(super) fn sdp(&mut self) -> u8 {
        // Enable display
        self.display_enabled = true;
        8 // 8 cycles (corrected from PC-1500 manual)
    }

    /// RDP (Reset DisP) - Resets the LCD on/off control flipflop DISP  
    /// Format: RDP
    /// Operation: 0 → DISP
    /// Cycles: 8 (from PC-1500 manual)
    /// Notes: Off pattern signal generated from CPU internal LCD backplate signal lines (H0-H7)
    pub(super) fn rdp(&mut self) -> u8 {
        // Disable display
        self.display_enabled = false;
        8 // 8 cycles (corrected from PC-1500 manual)
    }

    // ========================================================================
    // INPUT/OUTPUT INSTRUCTIONS
    // ========================================================================

    /// ITA (In To Acc) - Transfers input port contents to accumulator
    /// Format: ITA
    /// Operation: IN0-7 → Accumulator
    /// Cycles: 9 (from PC-1500 manual)
    /// Notes: Only the flag Z changes based on result
    pub(super) fn ita(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        // Read keyboard state from the keyboard controller
        let keyboard_value = memory.read_keyboard_input();

        // Load the keyboard input into the accumulator
        self.a = keyboard_value;

        // Set flags based on the loaded value (only Z flag changes per documentation)
        self.set_zero_flag(keyboard_value == 0);

        9 // 9 cycles (corrected from PC-1500 manual)
    }

    // ========================================================================
    // INTERRUPT CONTROL INSTRUCTIONS
    // ========================================================================

    /// SIE (Set IE) - Sets the interrupt enable flag IE
    /// Format: SIE
    /// Operation: 1 → IE
    /// Cycles: 8 (from PC-1500 manual)
    /// Notes: After this, it becomes ready for maskable interrupt and timer interrupt acknowledge
    pub(super) fn sie(&mut self) -> u8 {
        self.interrupt_enabled = true;
        8 // 8 cycles (corrected from PC-1500 manual)
    }

    /// RIE (Reset IE) - Resets the interrupt enable flag IE
    /// Format: RIE
    /// Operation: 0 → IE
    /// Cycles: 8 (from PC-1500 manual)
    /// Notes: After this, maskable interrupt and timer interrupt are disabled
    pub(super) fn rie(&mut self) -> u8 {
        self.interrupt_enabled = false;
        8 // 8 cycles (corrected from PC-1500 manual)
    }

    // ========================================================================
    // TIMER CONTROL INSTRUCTIONS
    // ========================================================================

    /// AM0 (Acc to Tm and 0) - Transfers accumulator to timer register and sets TM8 to 0
    /// Format: AM0
    /// Operation: A → TM (TM0~TM7), 0 → TM8
    /// Cycles: 9 (from PC-1500 manual)
    /// Notes: No change takes place in other flags
    pub(super) fn am0(&mut self, _memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        // Transfer accumulator to timer register (low 8 bits)
        self.timer_register = (self.timer_register & 0xFF00) | (self.a as u16);
        // Clear TM8 (bit 8 of timer register)
        self.timer_register &= 0x00FF;
        
        // Note: Timer interaction will be handled at system level in mod.rs
        // This instruction sets the internal CPU timer register
        
        9 // 9 cycles (corrected from PC-1500 manual)
    }

    /// AM1 (Acc to Tm and 1) - Transfers accumulator to timer register and sets TM8 to 1
    /// Format: AM1
    /// Operation: A → TM (TM0~TM7), 1 → TM8
    /// Cycles: 9 (from PC-1500 manual)
    /// Notes: Same as AM0, but "1" is entered in the highest order bit
    pub(super) fn am1(&mut self, _memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        // Transfer accumulator to timer register (low 8 bits)
        self.timer_register = (self.timer_register & 0xFF00) | (self.a as u16);
        // Set TM8 (bit 8 of timer register)
        self.timer_register |= 0x0100;
        
        // Note: Timer interaction will be handled at system level in mod.rs
        // This instruction sets the internal CPU timer register
        
        9 // 9 cycles (corrected from PC-1500 manual)
    }

    /// Get the current timer register value for system timer synchronization
    pub const fn get_timer_register(&self) -> u16 {
        self.timer_register
    }

    // ========================================================================
    // SYSTEM OPERATION INSTRUCTIONS
    // ========================================================================

    /// NOP (No Operation) - Does nothing for one cycle
    /// Format: NOP
    /// Operation: No operation
    /// Cycles: 5 (from PC-1500 manual)
    pub(super) fn nop(&mut self) -> u8 {
        // Do nothing
        5 // 5 cycles (corrected from PC-1500 manual)
    }

    /// HLT (Halt) - Stops CPU operation
    /// Format: HLT
    /// Operation: Stops CPU operation (only divider remains in operation)
    /// Cycles: 9 (from PC-1500 manual)
    /// Notes: Released from stop by interrupt. No change takes place in flags.
    pub(super) fn hlt(&mut self) -> u8 {
        // Set CPU to halted state
        self.halted = true;
        9 // 9 cycles (corrected from PC-1500 manual)
    }

    /// OFF - BF flipflop reset instruction
    /// Format: OFF
    /// Operation: BF flipflop reset
    /// Cycles: 8 (from PC-1500 manual)
    /// Notes: No change takes place in flags
    pub(super) fn off(&mut self) -> u8 {
        // Reset BF flipflop (power management)
        self.bf_flipflop = false;
        8 // 8 cycles (corrected from PC-1500 manual)
    }

    // ========================================================================
    // TRANSFER AND SEARCH INSTRUCTIONS (2-4-4)
    // ========================================================================

    // ① LDA (LoaD Accumulator) - Transfer contents to accumulator
    // Only the flag Z changes

    /// LDA RL - Load RL register to accumulator
    pub(super) fn lda_rl(&mut self, reg: u8) -> u8 {
        self.a = reg;
        self.set_zero_flag(self.a == 0);
        5 // 5 cycles
    }

    /// LDA RH - Load RH register to accumulator
    pub(super) fn lda_rh(&mut self, reg: u8) -> u8 {
        self.a = reg;
        self.set_zero_flag(self.a == 0);
        5 // 5 cycles
    }

    /// LDA (Rreg) - Load memory contents via Rreg to accumulator
    pub(super) fn lda_rreg(&mut self, memory: &mut MemoryBus, rreg: u16) -> u8 {
        let val = self.read_me0(memory, rreg);
        self.a = val;
        self.set_zero_flag(self.a == 0);
        7 // 7 cycles
    }

    /// LDA #(Rreg) - Load memory contents via indirect Rreg to accumulator
    pub(super) fn lda_rreg_indirect(&mut self, memory: &mut MemoryBus, rreg: u16) -> u8 {
        let val = self.read_me1(memory, rreg);
        self.a = val;
        self.set_zero_flag(self.a == 0);
        9 // 9 cycles
    }

    /// LDA (ab) - Load memory contents via immediate address to accumulator
    pub(super) fn lda_ab(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me0(memory);
        self.a = val;
        self.set_zero_flag(self.a == 0);
        8 // 8 cycles
    }

    /// LDA #(ab) - Load memory contents via indirect immediate address to accumulator
    pub(super) fn lda_ab_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let val = self.read_ab_me1(memory);
        self.a = val;
        self.set_zero_flag(self.a == 0);
        10 // 10 cycles
    }

    // ② LDE (Load and DEcrement) - Load external memory to accumulator, then decrement Rreg
    // Only the flag Z changes

    /// LDE Rreg - Load memory via Rreg to accumulator, then decrement Rreg
    pub(super) fn lde_rreg(&mut self, memory: &mut MemoryBus, rreg: &mut u16) -> u8 {
        let val = self.read_me0(memory, *rreg);
        self.a = val;
        self.set_zero_flag(self.a == 0);
        *rreg = rreg.wrapping_sub(1);
        7 // 7 cycles
    }

    // ③ LIN (Load and INcrement) - Load external memory to accumulator, then increment Rreg
    // Only the flag Z changes

    /// LIN Rreg - Load memory via Rreg to accumulator, then increment Rreg
    pub(super) fn lin_rreg(&mut self, memory: &mut MemoryBus, rreg: &mut u16) -> u8 {
        let val = self.read_me0(memory, *rreg);
        self.a = val;
        self.set_zero_flag(self.a == 0);
        *rreg = rreg.wrapping_add(1);
        7 // 7 cycles
    }

    // ④ LDI (Load Immediate) - Load immediate data to accumulator, RL, RH, or stack pointer S

    /// LDI A,i - Load immediate to accumulator
    pub(super) fn ldi_a(&mut self, memory: &mut MemoryBus) -> u8 {
        let immediate = self.imm8(memory);
        self.a = immediate;
        self.set_zero_flag(self.a == 0);
        6 // 6 cycles
    }

    /// LDI RL,i - Load immediate to RL register
    pub(super) fn ldi_rl(&mut self, memory: &mut MemoryBus, set_rl: fn(&mut Self, u8)) -> u8 {
        let immediate = self.imm8(memory);
        set_rl(self, immediate);
        // No flag changes for register loads
        6 // 6 cycles
    }

    /// LDI RH,i - Load immediate to RH register
    pub(super) fn ldi_rh(&mut self, memory: &mut MemoryBus, set_rh: fn(&mut Self, u8)) -> u8 {
        let immediate = self.imm8(memory);
        set_rh(self, immediate);
        // No flag changes for register loads
        6 // 6 cycles
    }

    /// LDI S,i - Load immediate to stack pointer (2 bytes)
    pub(super) fn ldi_s(&mut self, memory: &mut MemoryBus) -> u8 {
        let immediate = self.imm16(memory);
        self.s = immediate;
        // No flag changes for register loads
        7 // 7 cycles
    }

    // ⑤ LDX (Load X register) - Load contents to X register, stack pointer, or program counter

    /// LDX Rreg - Load Rreg contents to another register
    pub(super) fn ldx_rreg(&mut self, src_reg: u16, dst_reg: &mut u16) -> u8 {
        *dst_reg = src_reg;
        // No change takes place in flags
        6 // 6 cycles
    }

    /// LDX S - Load stack pointer
    pub(super) fn ldx_s(&mut self, src_reg: u16) -> u8 {
        self.s = src_reg;
        // No change takes place in flags
        6 // 6 cycles
    }

    /// LDX P - Load program counter
    pub(super) fn ldx_p(&mut self, src_reg: u16) -> u8 {
        self.p = src_reg;
        // No change takes place in flags
        6 // 6 cycles
    }

    /// LDX X - Load X register with X (identity operation)
    fn ldx_x(&mut self) {
        // This is essentially a no-op for LDX X since X loads itself
        // But it may have side effects in ME mode - placeholder implementation
    }

    // ⑥ STA (STore Accumulator) - Store accumulator contents to RL, RH, or external memory

    /// STA RL - Store accumulator to RL register
    pub(super) fn sta_rl(&mut self, set_rl: fn(&mut Self, u8)) -> u8 {
        set_rl(self, self.a);
        // No change takes place in flags
        5 // 5 cycles
    }

    /// STA RH - Store accumulator to RH register  
    pub(super) fn sta_rh(&mut self, set_rh: fn(&mut Self, u8)) -> u8 {
        set_rh(self, self.a);
        // No change takes place in flags
        5 // 5 cycles
    }

    /// STA (Rreg) - Store accumulator to memory via Rreg
    pub(super) fn sta_rreg(&mut self, memory: &mut MemoryBus, rreg: u16) -> u8 {
        self.write_me0(memory, rreg, self.a);
        // No change takes place in flags
        7 // 7 cycles
    }

    /// STA #(Rreg) - Store accumulator to memory via indirect Rreg
    pub(super) fn sta_rreg_indirect(&mut self, memory: &mut MemoryBus, rreg: u16) -> u8 {
        self.write_me1(memory, rreg, self.a);
        // No change takes place in flags
        9 // 9 cycles
    }

    /// STA (ab) - Store accumulator to memory via immediate address
    pub(super) fn sta_ab(&mut self, memory: &mut MemoryBus, addr: u16) -> u8 {
        self.write(memory, addr, self.a);
        // No change takes place in flags
        8 // 8 cycles
    }

    /// STA #(ab) - Store accumulator to memory via indirect immediate address
    pub(super) fn sta_ab_indirect(&mut self, memory: &mut MemoryBus, addr: u16) -> u8 {
        let target_addr = self.read_word_me0(memory, addr);
        self.write(memory, target_addr, self.a);
        // No change takes place in flags
        10 // 10 cycles
    }

    // ⑦ SDE (Store and DEcrement) - Store accumulator to external memory, then decrement Rreg

    /// SDE Rreg - Store accumulator to memory via Rreg, then decrement Rreg
    pub(super) fn sde_rreg(&mut self, memory: &mut MemoryBus, rreg: &mut u16) -> u8 {
        self.write_me0(memory, *rreg, self.a);
        *rreg = rreg.wrapping_sub(1);
        // No change takes place in flags
        7 // 7 cycles
    }

    // ⑧ SIN (Store and INcrement) - Store accumulator to external memory, then increment Rreg

    /// SIN Rreg - Store accumulator to memory via Rreg, then increment Rreg
    pub(super) fn sin_rreg(&mut self, memory: &mut MemoryBus, rreg: &mut u16) -> u8 {
        self.write_me0(memory, *rreg, self.a);
        *rreg = rreg.wrapping_add(1);
        // No change takes place in flags
        7 // 7 cycles
    }

    // ⑨ STX (Store X register) - Store X register contents

    /// STX Rreg - Store register contents to another register, stack pointer, or program counter
    pub(super) fn stx_rreg(&mut self, src_reg: u16, dst_reg: &mut u16) -> u8 {
        *dst_reg = src_reg;
        // No change takes place in flags
        6 // 6 cycles
    }

    /// STX S - Store to stack pointer
    pub(super) fn stx_s(&mut self, src_reg: u16) -> u8 {
        self.s = src_reg;
        // No change takes place in flags
        6 // 6 cycles
    }

    /// STX P - Store to program counter
    pub(super) fn stx_p(&mut self, src_reg: u16) -> u8 {
        self.p = src_reg;
        // No change takes place in flags
        6 // 6 cycles
    }

    // ⑩ PSH (Push) - Push accumulator or R register contents to stack

    /// PSH A - Push accumulator to stack
    pub(super) fn psh_a(&mut self, memory: &mut MemoryBus) -> u8 {
        self.s = self.s.wrapping_sub(1);
        self.write(memory, self.s, self.a);
        3 // 3 cycles
    }

    /// Push single byte to stack (helper method for subroutine jumps)
    pub(super) fn push(&mut self, memory: &mut MemoryBus, value: u8) {
        self.s = self.s.wrapping_sub(1);
        self.write(memory, self.s, value);
    }

    /// Pop single byte from stack (helper method for return instructions)
    pub(super) fn pop(&mut self, memory: &mut MemoryBus) -> u8 {
        let value = self.read(memory, self.s);
        self.s = self.s.wrapping_add(1);
        value
    }

    /// PSH Rreg - Push register contents to stack (stores RH first, then RL)
    pub(super) fn psh_rreg(&mut self, memory: &mut MemoryBus, rh: u8, rl: u8) -> u8 {
        // Push RH first
        self.s = self.s.wrapping_sub(1);
        self.write(memory, self.s, rh);
        // Then push RL
        self.s = self.s.wrapping_sub(1);
        self.write(memory, self.s, rl);
        5 // 5 cycles
    }

    // ⑪ POP (Pop) - Pop stack contents to accumulator or R register

    /// POP A - Pop stack to accumulator
    pub(super) fn pop_a(&mut self, memory: &mut MemoryBus) -> u8 {
        self.a = self.read(memory, self.s);
        self.s = self.s.wrapping_add(1);
        self.set_zero_flag(self.a == 0);
        3 // 3 cycles
    }

    /// POP Rreg - Pop stack to register (loads RL first, then RH)
    pub(super) fn pop_rreg(
        &mut self,
        memory: &mut MemoryBus,
        set_rl: fn(&mut Self, u8),
        set_rh: fn(&mut Self, u8),
    ) -> u8 {
        // Pop RL first
        let rl = self.read(memory, self.s);
        self.s = self.s.wrapping_add(1);
        set_rl(self, rl);

        // Then pop RH
        let rh = self.read(memory, self.s);
        self.s = self.s.wrapping_add(1);
        set_rh(self, rh);

        // No flag change
        5 // 5 cycles
    }

    // ⑫ ATT (Accumulator To T) - Transfer accumulator contents to T register

    /// ATT - Transfer accumulator to T register
    pub(super) fn att(&mut self) -> u8 {
        self.t_register = self.a;
        2 // 2 cycles
    }

    // ⑬ TTA (T To Accumulator) - Transfer T register contents to accumulator

    /// TTA - Transfer T register to accumulator
    pub(super) fn tta(&mut self) -> u8 {
        self.a = self.t_register;
        self.set_zero_flag(self.a == 0);
        2 // 2 cycles
    }

    // ========================================================================
    // BLOCK TRANSFER AND SEARCH INSTRUCTIONS (2-4-5)
    // ========================================================================

    // ① TIN (Transfer and INcrement) - Transfer memory block and increment both registers
    // No change takes place in flags

    /// TIN - Transfer contents from (Xreg) to (Yreg), then increment both Xreg and Yreg
    /// Format: TIN
    /// Operation: (Xreg) → (Yreg), Xreg+1 → Xreg, Yreg+1 → Yreg
    /// Flags: No change takes place in flags
    /// Example: If Xreg=4700H, Yreg=4800H, and contents of 4700H is 01110011,
    ///          after TIN execution: Yreg=4801H, Xreg=4701H, contents of 4800H becomes 01110011
    pub(super) fn tin(&mut self, memory: &mut MemoryBus) -> u8 {
        // Read from memory location pointed by X register
        let value = self.read_me0(memory, self.x);

        // Write to memory location pointed by Y register
        self.write_me0(memory, self.y, value);

        // Increment both X and Y registers
        self.x = self.x.wrapping_add(1);
        self.y = self.y.wrapping_add(1);

        // No flags change according to manual
        7 // Estimated 7 cycles for memory-to-memory transfer
    }

    // ② CIN (Compare and INcrement) - Compare accumulator with memory and increment Xreg
    // Changes flags C, H, Z, V (same as CPA instruction)

    /// CIN - Compare accumulator with (Xreg), then increment Xreg
    /// Format: CIN  
    /// Operation: A - (Xreg) → change in C, H, Z, and V; Xreg+1 → Xreg
    /// Flags: C, H, Z, V change (relation between comparison result and flags is same as CPA)
    /// Description: Compares accumulator with external memory contents pointed by Xreg,
    ///              sets flags based on comparison result, then increments Xreg by one
    pub(super) fn cin_block(&mut self, memory: &mut MemoryBus) -> u8 {
        // Read from memory location pointed by X register
        let memory_value = self.read_me0(memory, self.x);

        // Perform comparison (A - memory_value) and set flags
        // This is identical to CPA operation - subtract without storing result
        let result = self.a.wrapping_sub(memory_value);

        // Set flags based on comparison (same as CPA)
        self.set_zero_flag(result == 0);
        self.set_carry_flag(self.a < memory_value); // Carry set if A < memory_value

        // Half-carry flag: set if borrow from bit 4
        let half_carry = (self.a & 0x0F) < (memory_value & 0x0F);
        self.set_half_carry_flag(half_carry);

        // Overflow flag: set if signs are different and result sign differs from A
        let overflow = ((self.a ^ memory_value) & 0x80) != 0 && ((self.a ^ result) & 0x80) != 0;
        self.set_overflow_flag(overflow);

        // Increment X register after comparison
        self.x = self.x.wrapping_add(1);

        6 // Estimated 6 cycles for compare + increment
    }

    // ========================================================================
    // ROTATE AND SHIFT INSTRUCTIONS (2-4-6)
    // ========================================================================

    // ① ROL (ROtate Left) - Rotate accumulator left through carry flag
    // Carry flag changes

    /// ROL - Rotate accumulator left through carry flag
    /// Format: ROL
    /// Operation: C ← [7][6][5][4][3][2][1][0] ← C
    /// Flags: Carry flag changes based on bit 7 of original accumulator
    /// Description: Rotates accumulator contents left through carry flag.
    ///              Bit 7 goes to carry, carry goes to bit 0
    pub(super) fn rol(&mut self) -> u8 {
        let old_carry = if self.get_carry_flag() { 1 } else { 0 };
        let new_carry = (self.a & 0x80) != 0; // Bit 7 becomes new carry

        // Rotate left: shift left and add old carry to bit 0
        self.a = (self.a << 1) | old_carry;

        // Set new carry flag
        self.set_carry_flag(new_carry);

        8 // 8 cycles for ROL operation (from PC-1500 manual)
    }

    // ② ROR (ROtate Right) - Rotate accumulator right through carry flag
    // Carry flag changes

    /// ROR - Rotate accumulator right through carry flag
    /// Format: ROR
    /// Operation: C → [7][6][5][4][3][2][1][0] → C
    /// Flags: Carry flag changes based on bit 0 of original accumulator
    /// Description: Rotates accumulator contents right through carry flag.
    ///              Bit 0 goes to carry, carry goes to bit 7
    pub(super) fn ror(&mut self) -> u8 {
        let old_carry = if self.get_carry_flag() { 0x80 } else { 0 };
        let new_carry = (self.a & 0x01) != 0; // Bit 0 becomes new carry

        // Rotate right: shift right and add old carry to bit 7
        self.a = (self.a >> 1) | old_carry;

        // Set new carry flag
        self.set_carry_flag(new_carry);

        9 // 9 cycles for ROR operation (from PC-1500 manual)
    }

    // ③ SHL (SHift Left) - Shift accumulator left
    // Carry flag changes

    /// SHL - Shift accumulator left
    /// Format: SHL
    /// Operation: C ← [7][6][5][4][3][2][1][0] ← 0
    /// Flags: Carry flag changes based on bit 7 of original accumulator
    /// Description: Shifts accumulator contents left, bit 7 goes to carry, 0 enters bit 0
    pub(super) fn shl(&mut self) -> u8 {
        let new_carry = (self.a & 0x80) != 0; // Bit 7 becomes carry

        // Shift left: multiply by 2 (bit 0 becomes 0)
        self.a = self.a << 1;

        // Set carry flag
        self.set_carry_flag(new_carry);

        6 // 6 cycles for SHL operation (from PC-1500 manual)
    }

    // ④ SHR (SHift Right) - Shift accumulator right
    // Carry flag changes

    /// SHR - Shift accumulator right  
    /// Format: SHR
    /// Operation: 0 → [7][6][5][4][3][2][1][0] → C
    /// Flags: Carry flag changes based on bit 0 of original accumulator
    /// Description: Shifts accumulator contents right, bit 0 goes to carry, 0 enters bit 7
    pub(super) fn shr(&mut self) -> u8 {
        let new_carry = (self.a & 0x01) != 0; // Bit 0 becomes carry

        // Shift right: divide by 2 (bit 7 becomes 0)
        self.a = self.a >> 1;

        // Set carry flag
        self.set_carry_flag(new_carry);

        9 // 9 cycles for SHR operation (from PC-1500 manual)
    }

    // ⑤ DRL (Digit Rotate Left) - Rotate 4-bit digits left between accumulator and memory
    // No change takes place in flags

    /// DRL - Digit rotate left between accumulator and memory (Xreg)
    /// Format: DRL (Xreg)
    /// Operation: Low 4 bits of memory → low 4 bits of A, low 4 bits of A → high 4 bits of A,
    ///           high 4 bits of A → high 4 bits of memory, high 4 bits of memory → low 4 bits of memory
    /// Flags: No change takes place in flags
    /// Description: 4-bit digit rotation between accumulator and external memory in left direction
    pub(super) fn drl_xreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let memory_value = self.read_me0(memory, self.x);

        // Extract 4-bit digits (nibbles)
        let mem_low = memory_value & 0x0F; // Low 4 bits of memory
        let mem_high = (memory_value & 0xF0) >> 4; // High 4 bits of memory
        let acc_low = self.a & 0x0F; // Low 4 bits of accumulator
        let acc_high = (self.a & 0xF0) >> 4; // High 4 bits of accumulator

        // Rotate left: mem_low → acc_low, acc_low → acc_high, acc_high → mem_high, mem_high → mem_low
        let new_acc_low = mem_low;
        let new_acc_high = acc_low;
        let new_mem_high = acc_high;
        let new_mem_low = mem_high;

        // Update accumulator and memory
        self.a = (new_acc_high << 4) | new_acc_low;
        let new_memory_value = (new_mem_high << 4) | new_mem_low;
        self.write_me0(memory, self.x, new_memory_value);

        // No flag changes
        12 // 12 cycles for DRL operation (from PC-1500 manual)
    }

    /// DRL - Digit rotate left between accumulator and memory (indirect Xreg)
    /// Format: DRL #(Xreg)
    /// Operation: Same as DRL (Xreg) but with indirect addressing
    /// Flags: No change takes place in flags
    pub(super) fn drl_xreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let memory_value = self.read_me1(memory, self.x);
        let target_addr = self.read_word_me0(memory, self.x);

        // Same digit rotation logic as direct addressing
        let mem_low = memory_value & 0x0F;
        let mem_high = (memory_value & 0xF0) >> 4;
        let acc_low = self.a & 0x0F;
        let acc_high = (self.a & 0xF0) >> 4;

        let new_acc_low = mem_low;
        let new_acc_high = acc_low;
        let new_mem_high = acc_high;
        let new_mem_low = mem_high;

        self.a = (new_acc_high << 4) | new_acc_low;
        let new_memory_value = (new_mem_high << 4) | new_mem_low;
        self.write(memory, target_addr, new_memory_value);

        // No flag changes
        16 // 16 cycles for DRL # operation (from PC-1500 manual)
    }

    // ⑥ DRR (Digit Rotate Right) - Rotate 4-bit digits right between accumulator and memory
    // No change takes place in flags

    /// DRR - Digit rotate right between accumulator and memory (Xreg)
    /// Format: DRR (Xreg)
    /// Operation: High 4 bits of memory → high 4 bits of A, high 4 bits of A → low 4 bits of A,
    ///           low 4 bits of A → low 4 bits of memory, low 4 bits of memory → high 4 bits of memory
    /// Flags: No change takes place in flags
    /// Description: 4-bit digit rotation between accumulator and external memory in right direction
    pub(super) fn drr_xreg(&mut self, memory: &mut MemoryBus) -> u8 {
        let memory_value = self.read_me0(memory, self.x);

        // Extract 4-bit digits (nibbles)
        let mem_low = memory_value & 0x0F;
        let mem_high = (memory_value & 0xF0) >> 4;
        let acc_low = self.a & 0x0F;
        let acc_high = (self.a & 0xF0) >> 4;

        // Rotate right: mem_high → acc_high, acc_high → acc_low, acc_low → mem_low, mem_low → mem_high
        let new_acc_high = mem_high;
        let new_acc_low = acc_high;
        let new_mem_low = acc_low;
        let new_mem_high = mem_low;

        // Update accumulator and memory
        self.a = (new_acc_high << 4) | new_acc_low;
        let new_memory_value = (new_mem_high << 4) | new_mem_low;
        self.write_me0(memory, self.x, new_memory_value);

        // No flag changes
        12 // 12 cycles for DRR operation (from PC-1500 manual)
    }

    /// DRR - Digit rotate right between accumulator and memory (indirect Xreg)
    /// Format: DRR #(Xreg)
    /// Operation: Same as DRR (Xreg) but with indirect addressing
    /// Flags: No change takes place in flags
    pub(super) fn drr_xreg_indirect(&mut self, memory: &mut MemoryBus) -> u8 {
        let memory_value = self.read_me1(memory, self.x);
        let target_addr = self.read_word_me0(memory, self.x);

        // Same digit rotation logic as direct addressing
        let mem_low = memory_value & 0x0F;
        let mem_high = (memory_value & 0xF0) >> 4;
        let acc_low = self.a & 0x0F;
        let acc_high = (self.a & 0xF0) >> 4;

        let new_acc_high = mem_high;
        let new_acc_low = acc_high;
        let new_mem_low = acc_low;
        let new_mem_high = mem_low;

        self.a = (new_acc_high << 4) | new_acc_low;
        let new_memory_value = (new_mem_high << 4) | new_mem_low;
        self.write(memory, target_addr, new_memory_value);

        // No flag changes
        16 // 16 cycles for DRR # operation (from PC-1500 manual)
    }

    // ⑦ AEX (EXchange Acc) - Exchange high and low 4 bits of accumulator
    // No change takes place in flags

    /// AEX - Exchange accumulator high and low 4 bits
    /// Format: AEX
    /// Operation: High 4 bits of accumulator ↔ Low 4 bits of accumulator
    /// Flags: No change takes place in flags
    /// Description: Swaps the high order 4 bits with the low order 4 bits of the accumulator
    /// Example: If A = 11001010, after AEX: A = 10101100
    pub(super) fn aex(&mut self) -> u8 {
        let low_nibble = self.a & 0x0F; // Extract low 4 bits
        let high_nibble = (self.a & 0xF0) >> 4; // Extract high 4 bits

        // Swap: high becomes low, low becomes high
        self.a = (low_nibble << 4) | high_nibble;

        // No flag changes
        6 // 6 cycles for AEX operation (from PC-1500 manual)
    }

    // ========================================================================
    // JUMP INSTRUCTIONS (2-4-8)
    // ========================================================================

    /// JMP (Jump) - Unconditional absolute jump
    /// Format: JMP i,j (3 bytes)
    /// Operation: i → PH, j → PL
    /// Flags: No change takes place in flags
    /// Description: Jumps to absolute address specified by immediate 16-bit value
    pub(super) fn jmp(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);
        self.p = addr;
        12 // 12 cycles (from PC-1500 manual)
    }

    /// BCH (Branch) - Unconditional relative branch
    /// Format: BCH+i / BCH-i (2 bytes)
    /// Operation: P + i → P / P - i → P
    /// Flags: No change takes place in flags
    /// Description: Jumps to relative address within range -255 < i < 255
    pub(super) fn bch(&mut self, memory: &mut MemoryBus) -> u8 {
        let offset = self.imm8(memory) as i8;
        self.branch(offset);
        8 // 8 cycles (from PC-1500 manual)
    }

    /// BCH- (Branch minus) - Unconditional relative branch backwards  
    /// Format: BCH-i (2 bytes)
    /// Operation: P - i → P  
    /// Flags: No change takes place in flags
    /// Description: Jumps to relative address backwards within range -255 < i < 255
    pub(super) fn bch_minus(&mut self, memory: &mut MemoryBus) -> u8 {
        let offset = self.imm8(memory) as i8;
        self.branch(-offset); // Negative offset for BCH-
        8 // 8 cycles (from PC-1500 manual)
    }

    /// LOP (Loop) - Loop instruction with UL register
    /// Format: LOP i (2 bytes)
    /// Operation: UL = UL - 1 → UL
    ///            If borrow ≠ 0: P + i → P (jump to relative address)
    ///            If borrow = 0: Continue to next step
    /// Flags: No change takes place in flags
    /// Description: Decrements UL and branches if not zero (if borrow not produced)
    pub(super) fn lop(&mut self, memory: &mut MemoryBus) -> u8 {
        let offset = self.imm8(memory) as i8;

        // Decrement UL (low byte of U register)
        let ul = self.ul();
        let new_ul = ul.wrapping_sub(1);
        self.set_ul(new_ul);

        // If borrow is not produced (UL != 0 after decrement), branch
        // This means we continue looping while UL > 0
        if new_ul != 0 {
            self.branch(offset);
            11 // 11 cycles when branch is taken (from PC-1500 manual)
        } else {
            8 // 8 cycles when branch is not taken (from PC-1500 manual)
        }
    }

    // ===== SUBROUTINE JUMP INSTRUCTIONS (2-4-9) =====

    /// SJP (Subroutine Jump) - Jump to subroutine with absolute address
    /// Format: SJP i,j (3 bytes)
    /// Operation: PL → (S), S - 1 → S, PH → (S), S - 1 → S, i → PH, j → PL
    /// Flags: No change takes place in flags
    /// Description: Stores current PC in stack, then jumps to absolute address i,j
    pub(super) fn sjp(&mut self, memory: &mut MemoryBus) -> u8 {
        let addr = self.imm16(memory);

        // Push current PC to stack (high byte first, then low byte)
        self.push(memory, (self.p >> 8) as u8); // PH → (S)
        self.push(memory, (self.p & 0xFF) as u8); // PL → (S)

        // Jump to new address
        self.p = addr;
        19 // 19 cycles (from PC-1500 official table)
    }

    /// VEJ (Vector subroutine Jump) - One step subroutine jump instruction
    /// Format: VEJ i (1 byte operand)
    /// Operation: PL → (S), S - 1 → S, PH → (S), S - 1 → S, (FF00H+i) → PH, (FF01H+i) → PL
    /// Flags: Flag Z is reset
    /// Description: Vector address table contains address area from FF00H to FF6H
    /// There are 28 kinds of VEJ operands within range 110000000 (C0H) to 11110110 (F6H)
    pub(super) fn vej(&mut self, memory: &mut MemoryBus, vector: u8) -> u8 {
        // Push current PC to stack
        self.push(memory, (self.p >> 8) as u8); // PH → (S)
        self.push(memory, (self.p & 0xFF) as u8); // PL → (S)

        // Calculate vector address (FF00H + i for high byte, FF01H + i for low byte)
        let vector_addr_h = 0xFF00 + (vector as u16);
        let vector_addr_l = 0xFF00 + (vector as u16) + 1;

        // Read target address from vector table
        let target_h = self.read(memory, vector_addr_h);
        let target_l = self.read(memory, vector_addr_l);
        let target_addr = ((target_h as u16) << 8) | (target_l as u16);

        // Jump to target address
        self.p = target_addr;

        // Reset Z flag as specified
        self.set_zero_flag(false);

        17 // 17 cycles (from PC-1500 official table)
    }

    /// VMJ (Vector 2 byte Subroutine Jump) - Jump to address indicated by 2-byte vector
    /// Format: VMJ i (2 bytes)
    /// Operation: PL → (S), S - 1 → S, PH → (S), S - 1 → S, (FF00H+i) → PH, (FF01H+i) → PL
    /// Flags: Flag Z is reset
    /// Description: Vector address table contains FF00H thru FF6H, immediate value range 00H-F6H
    pub(super) fn vmj(&mut self, memory: &mut MemoryBus) -> u8 {
        let vector = self.imm8(memory);

        // Push current PC to stack
        self.push(memory, (self.p >> 8) as u8); // PH → (S)
        self.push(memory, (self.p & 0xFF) as u8); // PL → (S)

        // Calculate vector address (FF00H + i for high byte, FF01H + i for low byte)
        let vector_addr_h = 0xFF00 + (vector as u16);
        let vector_addr_l = 0xFF00 + (vector as u16) + 1;

        // Read target address from vector table
        let target_h = self.read(memory, vector_addr_h);
        let target_l = self.read(memory, vector_addr_l);
        let target_addr = ((target_h as u16) << 8) | (target_l as u16);

        // Jump to target address
        self.p = target_addr;

        // Reset Z flag as specified
        self.set_zero_flag(false);

        20 // 20 cycles (from PC-1500 official table)
    }

    /// VCS (Vector subroutine jump if C Set) - Conditional vector subroutine jump
    /// Format: VCS i (2 bytes)
    /// Operation: If C=1, performs vector subroutine jump same as VMJ instruction
    ///           If C=0, control proceeds to succeeding program step
    /// Flags: No change if branch not taken, Z reset if branch taken
    pub(super) fn vcs(&mut self, memory: &mut MemoryBus) -> u8 {
        let vector = self.imm8(memory);

        if self.get_carry_flag() {
            // Perform vector subroutine jump like VMJ
            self.push(memory, (self.p >> 8) as u8);
            self.push(memory, (self.p & 0xFF) as u8);

            let vector_addr_h = 0xFF00 + (vector as u16);
            let vector_addr_l = 0xFF00 + (vector as u16) + 1;

            let target_h = self.read(memory, vector_addr_h);
            let target_l = self.read(memory, vector_addr_l);
            let target_addr = ((target_h as u16) << 8) | (target_l as u16);

            self.p = target_addr;
            self.set_zero_flag(false);

            21 // 21 cycles when jump is taken (from PC-1500 official table)
        } else {
            8 // 8 cycles when jump is not taken (from PC-1500 official table)
        }
    }

    /// VCR (Vector subroutine jump if C Reset) - Conditional vector subroutine jump
    /// Format: VCR i (2 bytes)  
    /// Operation: If C=0, performs vector subroutine jump same as VMJ instruction
    ///           If C=1, control proceeds to succeeding program step
    /// Flags: No change if branch not taken, Z reset if branch taken
    pub(super) fn vcr(&mut self, memory: &mut MemoryBus) -> u8 {
        let vector = self.imm8(memory);

        if !self.get_carry_flag() {
            // Perform vector subroutine jump like VMJ
            self.push(memory, (self.p >> 8) as u8);
            self.push(memory, (self.p & 0xFF) as u8);

            let vector_addr_h = 0xFF00 + (vector as u16);
            let vector_addr_l = 0xFF00 + (vector as u16) + 1;

            let target_h = self.read(memory, vector_addr_h);
            let target_l = self.read(memory, vector_addr_l);
            let target_addr = ((target_h as u16) << 8) | (target_l as u16);

            self.p = target_addr;
            self.set_zero_flag(false);

            21 // 21 cycles when jump is taken (from PC-1500 official table)
        } else {
            8 // 8 cycles when jump is not taken (from PC-1500 official table)
        }
    }

    /// VHS (Vector subroutine jump if H Set) - Conditional vector subroutine jump
    /// Format: VHS i (2 bytes)
    /// Operation: If H=1, performs vector subroutine jump same as VMJ instruction
    ///           If H=0, control proceeds to succeeding program step
    /// Flags: No change if branch not taken, Z reset if branch taken
    pub(super) fn vhs(&mut self, memory: &mut MemoryBus) -> u8 {
        let vector = self.imm8(memory);

        if self.get_half_carry_flag() {
            // Perform vector subroutine jump like VMJ
            self.push(memory, (self.p >> 8) as u8);
            self.push(memory, (self.p & 0xFF) as u8);

            let vector_addr_h = 0xFF00 + (vector as u16);
            let vector_addr_l = 0xFF00 + (vector as u16) + 1;

            let target_h = self.read(memory, vector_addr_h);
            let target_l = self.read(memory, vector_addr_l);
            let target_addr = ((target_h as u16) << 8) | (target_l as u16);

            self.p = target_addr;
            self.set_zero_flag(false);

            21 // 21 cycles when jump is taken (from PC-1500 official table)
        } else {
            8 // 8 cycles when jump is not taken (from PC-1500 official table)
        }
    }

    /// VHR (Vector subroutine jump if H Reset) - Conditional vector subroutine jump
    /// Format: VHR i (2 bytes)
    /// Operation: If H=0, performs vector subroutine jump same as VMJ instruction
    ///           If H=1, control proceeds to succeeding program step
    /// Flags: No change if branch not taken, Z reset if branch taken
    pub(super) fn vhr(&mut self, memory: &mut MemoryBus) -> u8 {
        let vector = self.imm8(memory);

        if !self.get_half_carry_flag() {
            // Perform vector subroutine jump like VMJ
            self.push(memory, (self.p >> 8) as u8);
            self.push(memory, (self.p & 0xFF) as u8);

            let vector_addr_h = 0xFF00 + (vector as u16);
            let vector_addr_l = 0xFF00 + (vector as u16) + 1;

            let target_h = self.read(memory, vector_addr_h);
            let target_l = self.read(memory, vector_addr_l);
            let target_addr = ((target_h as u16) << 8) | (target_l as u16);

            self.p = target_addr;
            self.set_zero_flag(false);

            21 // 21 cycles when jump is taken (from PC-1500 official table)
        } else {
            8 // 8 cycles when jump is not taken (from PC-1500 official table)
        }
    }

    /// VZS (Vector subroutine jump if Z Set) - Conditional vector subroutine jump
    /// Format: VZS i (2 bytes)
    /// Operation: If Z=1, performs vector subroutine jump same as VMJ instruction
    ///           If Z=0, control proceeds to succeeding program step
    /// Flags: No change if branch not taken, Z reset if branch taken
    pub(super) fn vzs(&mut self, memory: &mut MemoryBus) -> u8 {
        let vector = self.imm8(memory);

        if self.get_zero_flag() {
            // Perform vector subroutine jump like VMJ
            self.push(memory, (self.p >> 8) as u8);
            self.push(memory, (self.p & 0xFF) as u8);

            let vector_addr_h = 0xFF00 + (vector as u16);
            let vector_addr_l = 0xFF00 + (vector as u16) + 1;

            let target_h = self.read(memory, vector_addr_h);
            let target_l = self.read(memory, vector_addr_l);
            let target_addr = ((target_h as u16) << 8) | (target_l as u16);

            self.p = target_addr;
            self.set_zero_flag(false);

            21 // 21 cycles when jump is taken (from PC-1500 official table)
        } else {
            8 // 8 cycles when jump is not taken (from PC-1500 official table)
        }
    }

    /// VZR (Vector subroutine jump if Z Reset) - Conditional vector subroutine jump
    /// Format: VZR i (2 bytes)
    /// Operation: If Z=0, performs vector subroutine jump same as VMJ instruction
    ///           If Z=1, control proceeds to succeeding program step
    /// Flags: No change if branch not taken, Z reset if branch taken
    pub(super) fn vzr(&mut self, memory: &mut MemoryBus) -> u8 {
        let vector = self.imm8(memory);

        if !self.get_zero_flag() {
            // Perform vector subroutine jump like VMJ
            self.push(memory, (self.p >> 8) as u8);
            self.push(memory, (self.p & 0xFF) as u8);

            let vector_addr_h = 0xFF00 + (vector as u16);
            let vector_addr_l = 0xFF00 + (vector as u16) + 1;

            let target_h = self.read(memory, vector_addr_h);
            let target_l = self.read(memory, vector_addr_l);
            let target_addr = ((target_h as u16) << 8) | (target_l as u16);

            self.p = target_addr;
            self.set_zero_flag(false);

            21 // 21 cycles when jump is taken (from PC-1500 official table)
        } else {
            8 // 8 cycles when jump is not taken (from PC-1500 official table)
        }
    }

    /// VVS (Vector subroutine jump if V Set) - Conditional vector subroutine jump
    /// Format: VVS i (2 bytes)
    /// Operation: If V=1, performs vector subroutine jump same as VMJ instruction
    ///           If V=0, control proceeds to succeeding program step
    /// Flags: No change if branch not taken, Z reset if branch taken
    pub(super) fn vvs(&mut self, memory: &mut MemoryBus) -> u8 {
        let vector = self.imm8(memory);

        if self.get_overflow_flag() {
            // Perform vector subroutine jump like VMJ
            self.push(memory, (self.p >> 8) as u8);
            self.push(memory, (self.p & 0xFF) as u8);

            let vector_addr_h = 0xFF00 + (vector as u16);
            let vector_addr_l = 0xFF00 + (vector as u16) + 1;

            let target_h = self.read(memory, vector_addr_h);
            let target_l = self.read(memory, vector_addr_l);
            let target_addr = ((target_h as u16) << 8) | (target_l as u16);

            self.p = target_addr;
            self.set_zero_flag(false);

            21 // 21 cycles when jump is taken (from PC-1500 official table)
        } else {
            8 // 8 cycles when jump is not taken (from PC-1500 official table)
        }
    }

    // ===== RETURN INSTRUCTIONS (2-4-10) =====

    /// RTN (ReTurN from subroutine) - Return from subroutine to main routine
    /// Format: RTN (1 byte)
    /// Operation: S+1 -> S, (S) -> PL, S+1 -> S, (S) -> PH
    /// The previous program address is gotten from the external memory stack to be
    /// transferred to the program counter. No change takes place in flags.
    /// The next instruction will be fetched from the address indicated by the program counter.
    pub(super) fn rtn(&mut self, memory: &mut MemoryBus) -> u8 {
        // Pop low byte first (reverse of SJP push order)
        let low_byte = self.pop(memory);
        // Pop high byte
        let high_byte = self.pop(memory);

        // Restore program counter
        self.p = ((high_byte as u16) << 8) | (low_byte as u16);

        11 // 11 cycles according to PC-1500 manual timing
    }

    /// RTI (ReTurn from Interrupt) - Return from interrupt service routine to main routine
    /// Format: RTI (1 byte)  
    /// Operation: Restore context from stack and return to interrupted program
    /// Based on PC-1500 interrupt processing sequence from technical manual
    pub(super) fn rti(&mut self, memory: &mut MemoryBus) -> u8 {
        // Use the dedicated return from interrupt method
        // This follows the PC-1500 interrupt processing sequence exactly
        self.return_from_interrupt(memory);
        
        14 // 14 cycles according to PC-1500 manual timing
    }
}
