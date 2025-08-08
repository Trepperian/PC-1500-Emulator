/// Sharp LH5801 CPU implementation for PC-1500
///
/// The LH5801 is an 8-bit microprocessor used in Sharp pocket computers.
/// It features a different instruction set and register layout compared to the GameBoy's SM83.

#[derive(Debug, Clone)]
pub struct Lh5801Cpu {
    // 8-bit registers
    pub a: u8, // Accumulator
    pub b: u8, // B register

    // 16-bit address registers
    pub p: u16, // Program counter
    pub s: u16, // Stack pointer
    pub u: u16, // U pointer register
    pub x: u16, // X index register
    pub y: u16, // Y index register

    // Status and control
    pub flags: u8, // Processor status register
    pub interrupt_enabled: bool,
    
    // PC-1500 specific control flipflops and registers
    pub pu_flipflop: bool,      // General purpose flipflop PU
    pub pv_flipflop: bool,      // General purpose flipflop PV
    pub bf_flipflop: bool,      // BF flipflop (power management)
    pub display_enabled: bool,  // Display on/off control (DISP)
    pub timer_register: u16,    // Timer register (TM0-TM8)
    pub halted: bool,           // CPU halt state
}

impl Default for Lh5801Cpu {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            p: 0x0000, // Program counter starts at 0
            s: 0x9FFF, // Stack starts at top of RAM
            u: 0,
            x: 0,
            y: 0,
            flags: 0,
            interrupt_enabled: false,
            
            // PC-1500 specific control state
            pu_flipflop: false,
            pv_flipflop: false,
            bf_flipflop: true,        // BF starts as true (power on)
            display_enabled: false,   // Display starts disabled
            timer_register: 0,        // Timer starts at 0
            halted: false,           // CPU starts running
        }
    }
}

impl Lh5801Cpu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    // === GETTERS (following GameBoy CPU pattern) ===

    /// Get accumulator register
    #[must_use]
    pub const fn a(&self) -> u8 {
        self.a
    }

    /// Get B register
    #[must_use]
    pub const fn b(&self) -> u8 {
        self.b
    }

    /// Get program counter
    #[must_use]
    pub const fn p(&self) -> u16 {
        self.p
    }

    /// Get stack pointer
    #[must_use]
    pub const fn s(&self) -> u16 {
        self.s
    }

    /// Get U register
    #[must_use]
    pub const fn u(&self) -> u16 {
        self.u
    }

    /// Get X register
    #[must_use]
    pub const fn x(&self) -> u16 {
        self.x
    }

    /// Get Y register
    #[must_use]
    pub const fn y(&self) -> u16 {
        self.y
    }

    /// Get flags register
    #[must_use]
    pub const fn flags(&self) -> u8 {
        self.flags
    }

    /// Check if interrupts are enabled
    #[must_use]
    pub const fn interrupt_enabled(&self) -> bool {
        self.interrupt_enabled
    }

    // === SETTERS (for testing and debugging) ===

    /// Set program counter (for testing)
    pub fn set_pc(&mut self, pc: u16) {
        self.p = pc;
    }

    /// Set accumulator (for testing)
    pub fn set_a(&mut self, a: u8) {
        self.a = a;
    }

    /// Set B register (for testing)
    pub fn set_b(&mut self, b: u8) {
        self.b = b;
    }

    // === INTERRUPT HANDLING ===

    /// Check if CPU should handle interrupt
    pub fn should_handle_interrupt(&self) -> bool {
        self.interrupt_enabled
    }

    /// Handle interrupt (simplified for now)
    pub fn handle_interrupt(&mut self, vector: u16) {
        if self.interrupt_enabled {
            // Disable interrupts
            self.interrupt_enabled = false;
            // TODO: Push PC to stack and jump to vector
            self.p = vector;
        }
    }

    /// Request interrupt (for keyboard, etc.)
    pub fn request_interrupt(&mut self) {
        // TODO: Set interrupt pending flag
        // For now, just a placeholder
    }

    /// Execute one instruction cycle
    /// Returns the number of cycles consumed
    pub fn step(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        // Fetch instruction
        let opcode = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);

        // Decode and execute
        self.execute_instruction(opcode, memory)
    }

    /// Execute a specific instruction
    fn execute_instruction(
        &mut self,
        opcode: u8,
        memory: &mut crate::pc1500::memory::MemoryBus,
    ) -> u8 {
        match opcode {
            // === Load Instructions ===
            0x05 => self.lda_immediate(memory), // LDA #nn
            0x04 => self.lda_absolute(memory),  // LDA nnnn
            0x07 => self.sta_absolute(memory),  // STA nnnn
            0x06 => self.ldi_a(memory),         // LDI A,nn
            0x09 => self.ldb_immediate(memory), // LDB #nn
            0x08 => self.ldb_absolute(memory),  // LDB nnnn
            0x0B => self.stb_absolute(memory),  // STB nnnn

            // Load index registers
            0x02 => self.ldx_absolute(memory), // LDX nnnn
            0x03 => self.ldy_absolute(memory), // LDY nnnn
            0x0A => self.ldu_absolute(memory), // LDU nnnn

            // === Arithmetic Instructions ===
            0x0D => self.adc_immediate(memory), // ADC #nn
            0x0C => self.adc_absolute(memory),  // ADC nnnn
            0x0F => self.sbc_immediate(memory), // SBC #nn
            0x0E => self.sbc_absolute(memory),  // SBC nnnn
            0x01 => self.add_immediate(memory), // ADD #nn
            0x10 => self.sub_immediate(memory), // SUB #nn

            // === Logical Instructions ===
            0x11 => self.and_immediate(memory), // AND #nn
            0x12 => self.or_immediate(memory),  // OR #nn
            0x13 => self.xor_immediate(memory), // XOR #nn
            0x14 => self.cmp_immediate(memory), // CMP #nn
            0x15 => self.cmp_absolute(memory),  // CMP nnnn

            // === Increment/Decrement ===
            0x40 => self.inc_a(), // INC A
            0x42 => self.dec_a(), // DEC A
            0x41 => self.inc_b(), // INC B
            0x43 => self.dec_b(), // DEC B
            0x44 => self.inc_x(), // INC X
            0x45 => self.dec_x(), // DEC X
            0x46 => self.inc_y(), // INC Y
            0x47 => self.dec_y(), // DEC Y

            // === Jump Instructions ===
            0x20 => self.jmp_absolute(memory), // JMP nnnn
            0x21 => self.jsr_absolute(memory), // JSR nnnn (Jump to Subroutine)
            0x38 => self.rts(memory),          // RTS (Return from Subroutine)
            0x22 => self.jmp_indirect(memory), // JMP (nnnn)

            // === Branch Instructions ===
            0x81 => self.bra_relative(memory), // BRA nn (Branch Always)
            0x83 => self.bcs_relative(memory), // BCS nn (Branch if Carry Set)
            0x85 => self.bcc_relative(memory), // BCC nn (Branch if Carry Clear)
            0x87 => self.beq_relative(memory), // BEQ nn (Branch if Equal/Zero)
            0x89 => self.bne_relative(memory), // BNE nn (Branch if Not Equal)
            0x8B => self.bmi_relative(memory), // BMI nn (Branch if Minus)
            0x8D => self.bpl_relative(memory), // BPL nn (Branch if Plus)
            0x8F => self.bvc_relative(memory), // BVC nn (Branch if Overflow Clear)
            0x91 => self.bvs_relative(memory), // BVS nn (Branch if Overflow Set)

            // === Stack Operations ===
            0x48 => self.pha(memory), // PHA (Push A)
            0x4A => self.pla(memory), // PLA (Pull A)
            0x49 => self.phb(memory), // PHB (Push B)
            0x4B => self.plb(memory), // PLB (Pull B)
            0x4C => self.php(memory), // PHP (Push Processor Status)
            0x4D => self.plp(memory), // PLP (Pull Processor Status)

            // === Bit Manipulation ===
            0x50 => self.bit_immediate(memory), // BIT #nn
            0x51 => self.bit_absolute(memory),  // BIT nnnn

            // === Shift/Rotate Instructions ===
            0x60 => self.asl_a(), // ASL A
            0x61 => self.lsr_a(), // LSR A
            0x62 => self.rol_a(), // ROL A
            0x63 => self.ror_a(), // ROR A

            // === System Instructions ===
            0x00 => self.nop(),       // NOP
            0x39 => self.rti(memory), // RTI (Return from Interrupt)
            0x3A => self.sei(),       // SEI (Set Interrupt) 
            0x3B => self.cli(),       // CLI (Clear Interrupt)
            0x3D => self.clc(),       // CLC (Clear Carry)
            0x3E => self.sev(),       // SEV (Set Overflow)
            0x3F => self.clv(),       // CLV (Clear Overflow)
            
            // === PC-1500 Control Instructions (LH5801 official opcodes from documentation) ===
            0xFB => self.sec(),       // SEC (Set Carry) - 11111011 (1 byte, 4 cycles)
            0xF9 => self.rec(),       // REC (Reset Carry) - 11111001 (1 byte, 4 cycles)
            0xE1 => self.spu(),       // SPU (Set PU) - 11100001 (1 byte, 4 cycles)
            0xE3 => self.rpu(),       // RPU (Reset PU) - 11100011 (1 byte, 4 cycles)
            0xA8 => self.spv(),       // SPV (Set PV) - 10101000 (1 byte, 4 cycles)
            0xB8 => self.rpv(),       // RPV (Reset PV) - 10111000 (1 byte, 4 cycles)
            
            // Multi-byte instructions with FD prefix (0xFD)
            0xFD => self.execute_fd_instruction(memory), // FD prefix for extended instructions

            // === Transfer Instructions ===
            0x70 => self.tab(), // TAB (Transfer A to B)
            0x71 => self.tba(), // TBA (Transfer B to A)
            0x72 => self.tax(), // TAX (Transfer A to X)
            0x73 => self.txa(), // TXA (Transfer X to A)
            0x74 => self.tay(), // TAY (Transfer A to Y)
            0x75 => self.tya(), // TYA (Transfer Y to A)
            0x76 => self.txs(), // TXS (Transfer X to Stack)
            0x77 => self.tsx(), // TSX (Transfer Stack to X)

            // Unknown instruction
            _ => {
                // For debugging: increment PC and continue
                2 // Default cycle count for unknown instructions
            }
        }
    }

    /// Execute instructions with FD prefix (extended instruction set)
    fn execute_fd_instruction(
        &mut self,
        memory: &mut crate::pc1500::memory::MemoryBus,
    ) -> u8 {
        let second_byte = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        
        match second_byte {
            // Timer and system control instructions
            0xCE => self.am0(),       // AM0 (Acc to Tm and 0) - FD CE (2 bytes, 9 cycles)
            0xCF => self.am1(),       // AM1 (Acc to Tm and 1) - FD CF (2 bytes, 9 cycles) 
            0xCD => self.cdv(),       // CDV (Clear Divider) - FD CD (2 bytes, 8 cycles)
            0xCC => self.atp(memory), // ATP (Acc To Port) - FD CC (2 bytes, 9 cycles)
            0xC1 => self.sdp(),       // SDP (Set DisP) - FD C1 (2 bytes, 8 cycles)
            0xC0 => self.rdp(),       // RDP (Reset DisP) - FD C0 (2 bytes, 8 cycles)
            0xBA => self.ita(memory), // ITA (IN→A) - FD BA (2 bytes, 9 cycles)
            0xBE => self.rie(),       // RIE (Reset IE) - FD BE (2 bytes, 8 cycles)
            0x81 => self.sie(),       // SIE (Set IE) - FD 81 (2 bytes, 8 cycles)
            0xB1 => self.hlt(),       // HLT (Halt) - FD B1 (2 bytes, 9 cycles)
            0x4C => self.off(),       // OFF (BF reset) - FD 4C (2 bytes, 8 cycles)
            
            // Unknown FD instruction
            _ => {
                // For debugging: default cycle count for unknown FD instructions
                8 // Default 2-byte instruction cycles
            }
        }
    }

    // === INSTRUCTION IMPLEMENTATIONS ===

    // === LOAD INSTRUCTIONS ===

    /// LDA #nn - Load Accumulator Immediate
    fn lda_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        self.a = value;
        self.set_zero_flag(value == 0);
        self.set_negative_flag(value & 0x80 != 0);
        3 // cycles
    }

    /// LDA nnnn - Load Accumulator Absolute
    fn lda_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        let value = memory.read_byte(addr);
        self.a = value;
        self.set_zero_flag(value == 0);
        self.set_negative_flag(value & 0x80 != 0);
        4 // cycles
    }

    /// STA nnnn - Store Accumulator Absolute
    fn sta_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        memory.write_byte(addr, self.a);
        4 // cycles
    }

    /// LDI A,nn - Load Immediate (alias for LDA #nn)
    fn ldi_a(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.lda_immediate(memory)
    }

    /// LDB #nn - Load B Register Immediate
    fn ldb_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        self.b = value;
        self.set_zero_flag(value == 0);
        self.set_negative_flag(value & 0x80 != 0);
        3 // cycles
    }

    /// LDB nnnn - Load B Register Absolute
    fn ldb_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        let value = memory.read_byte(addr);
        self.b = value;
        self.set_zero_flag(value == 0);
        self.set_negative_flag(value & 0x80 != 0);
        4 // cycles
    }

    /// STB nnnn - Store B Register Absolute
    fn stb_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        memory.write_byte(addr, self.b);
        4 // cycles
    }

    /// LDX nnnn - Load X Index Register
    fn ldx_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        self.x = addr;
        4 // cycles
    }

    /// LDY nnnn - Load Y Index Register
    fn ldy_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        self.y = addr;
        4 // cycles
    }

    /// LDU nnnn - Load U Pointer Register
    fn ldu_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        self.u = addr;
        4 // cycles
    }

    // === ARITHMETIC INSTRUCTIONS ===

    /// ADC #nn - Add with Carry Immediate
    fn adc_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        self.add_with_carry(value);
        3 // cycles
    }

    /// ADC nnnn - Add with Carry Absolute
    fn adc_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        let value = memory.read_byte(addr);
        self.add_with_carry(value);
        4 // cycles
    }

    /// SBC #nn - Subtract with Carry Immediate
    fn sbc_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        self.subtract_with_carry(value);
        3 // cycles
    }

    /// SBC nnnn - Subtract with Carry Absolute
    fn sbc_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        let value = memory.read_byte(addr);
        self.subtract_with_carry(value);
        4 // cycles
    }

    /// ADD #nn - Add Immediate (without carry)
    fn add_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        let result = self.a as u16 + value as u16;

        self.set_carry_flag(result > 0xFF);
        self.set_overflow_flag((self.a ^ value) & 0x80 == 0 && (self.a ^ result as u8) & 0x80 != 0);

        self.a = result as u8;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        3 // cycles
    }

    /// SUB #nn - Subtract Immediate (without carry)
    fn sub_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        let result = self.a as i16 - value as i16;

        self.set_carry_flag(result >= 0);
        self.set_overflow_flag((self.a ^ value) & 0x80 != 0 && (self.a ^ result as u8) & 0x80 != 0);

        self.a = result as u8;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        3 // cycles
    }

    // === LOGICAL INSTRUCTIONS ===

    /// AND #nn - Logical AND Immediate
    fn and_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        self.a &= value;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        3 // cycles
    }

    /// OR #nn - Logical OR Immediate
    fn or_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        self.a |= value;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        3 // cycles
    }

    /// XOR #nn - Logical XOR Immediate
    fn xor_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        self.a ^= value;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        3 // cycles
    }

    /// CMP #nn - Compare Immediate
    fn cmp_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        let result = self.a as i16 - value as i16;

        self.set_carry_flag(result >= 0);
        self.set_zero_flag((result as u8) == 0);
        self.set_negative_flag((result as u8) & 0x80 != 0);
        3 // cycles
    }

    /// CMP nnnn - Compare Absolute
    fn cmp_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        let value = memory.read_byte(addr);
        let result = self.a as i16 - value as i16;

        self.set_carry_flag(result >= 0);
        self.set_zero_flag((result as u8) == 0);
        self.set_negative_flag((result as u8) & 0x80 != 0);
        4 // cycles
    }

    /// INC A - Increment Accumulator
    fn inc_a(&mut self) -> u8 {
        self.a = self.a.wrapping_add(1);
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        2 // cycles
    }

    /// DEC A - Decrement Accumulator
    fn dec_a(&mut self) -> u8 {
        self.a = self.a.wrapping_sub(1);
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        2 // cycles
    }

    /// INC B - Increment B Register
    fn inc_b(&mut self) -> u8 {
        self.b = self.b.wrapping_add(1);
        self.set_zero_flag(self.b == 0);
        self.set_negative_flag(self.b & 0x80 != 0);
        2 // cycles
    }

    /// DEC B - Decrement B Register
    fn dec_b(&mut self) -> u8 {
        self.b = self.b.wrapping_sub(1);
        self.set_zero_flag(self.b == 0);
        self.set_negative_flag(self.b & 0x80 != 0);
        2 // cycles
    }

    /// INC X - Increment X Register
    fn inc_x(&mut self) -> u8 {
        self.x = self.x.wrapping_add(1);
        // Note: 16-bit increment typically doesn't affect flags in many processors
        2 // cycles
    }

    /// DEC X - Decrement X Register
    fn dec_x(&mut self) -> u8 {
        self.x = self.x.wrapping_sub(1);
        // Note: 16-bit decrement typically doesn't affect flags in many processors
        2 // cycles
    }

    /// INC Y - Increment Y Register
    fn inc_y(&mut self) -> u8 {
        self.y = self.y.wrapping_add(1);
        // Note: 16-bit increment typically doesn't affect flags in many processors
        2 // cycles
    }

    /// DEC Y - Decrement Y Register
    fn dec_y(&mut self) -> u8 {
        self.y = self.y.wrapping_sub(1);
        // Note: 16-bit decrement typically doesn't affect flags in many processors
        2 // cycles
    }

    // === JUMP INSTRUCTIONS ===

    /// JMP nnnn - Jump Absolute
    fn jmp_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        self.p = addr;
        3 // cycles
    }

    /// JSR nnnn - Jump to Subroutine
    fn jsr_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        self.push_word(memory, self.p);
        self.p = addr;
        6 // cycles
    }

    /// RTS - Return from Subroutine
    fn rts(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.p = self.pop_word(memory);
        6 // cycles
    }

    /// JMP (nnnn) - Jump Indirect
    fn jmp_indirect(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let indirect_addr = self.read_word(memory);
        let low = memory.read_byte(indirect_addr);
        let high = memory.read_byte(indirect_addr.wrapping_add(1));
        let target_addr = u16::from_le_bytes([low, high]);
        self.p = target_addr;
        5 // cycles
    }

    // === BRANCH INSTRUCTIONS ===

    /// BRA nn - Branch Always
    fn bra_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        self.p = self.p.wrapping_add_signed(offset as i16);
        3 // cycles
    }

    /// BCS nn - Branch if Carry Set
    fn bcs_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        if self.get_carry_flag() {
            self.p = self.p.wrapping_add_signed(offset as i16);
            3 // taken
        } else {
            2 // not taken
        }
    }

    /// BCC nn - Branch if Carry Clear
    fn bcc_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        if !self.get_carry_flag() {
            self.p = self.p.wrapping_add_signed(offset as i16);
            3 // taken
        } else {
            2 // not taken
        }
    }

    /// BEQ nn - Branch if Equal (Zero flag set)
    fn beq_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        if self.get_zero_flag() {
            self.p = self.p.wrapping_add_signed(offset as i16);
            3 // taken
        } else {
            2 // not taken
        }
    }

    /// BNE nn - Branch if Not Equal (Zero flag clear)
    fn bne_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        if !self.get_zero_flag() {
            self.p = self.p.wrapping_add_signed(offset as i16);
            3 // taken
        } else {
            2 // not taken
        }
    }

    /// BMI nn - Branch if Minus (Negative flag set)
    fn bmi_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        if self.get_negative_flag() {
            self.p = self.p.wrapping_add_signed(offset as i16);
            3 // taken
        } else {
            2 // not taken
        }
    }

    /// BPL nn - Branch if Plus (Negative flag clear)
    fn bpl_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        if !self.get_negative_flag() {
            self.p = self.p.wrapping_add_signed(offset as i16);
            3 // taken
        } else {
            2 // not taken
        }
    }

    /// BVC nn - Branch if Overflow Clear
    fn bvc_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        if !self.get_overflow_flag() {
            self.p = self.p.wrapping_add_signed(offset as i16);
            3 // taken
        } else {
            2 // not taken
        }
    }

    /// BVS nn - Branch if Overflow Set
    fn bvs_relative(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let offset = memory.read_byte(self.p) as i8;
        self.p = self.p.wrapping_add(1);
        if self.get_overflow_flag() {
            self.p = self.p.wrapping_add_signed(offset as i16);
            3 // taken
        } else {
            2 // not taken
        }
    }

    // === STACK INSTRUCTIONS ===

    /// PHA - Push Accumulator
    fn pha(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.push_byte(memory, self.a);
        3 // cycles
    }

    /// PLA - Pull Accumulator
    fn pla(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.a = self.pop_byte(memory);
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        4 // cycles
    }

    /// PHB - Push B Register
    fn phb(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.push_byte(memory, self.b);
        3 // cycles
    }

    /// PLB - Pull B Register
    fn plb(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.b = self.pop_byte(memory);
        self.set_zero_flag(self.b == 0);
        self.set_negative_flag(self.b & 0x80 != 0);
        4 // cycles
    }

    /// PHP - Push Processor Status
    fn php(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.push_byte(memory, self.flags);
        3 // cycles
    }

    /// PLP - Pull Processor Status
    fn plp(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.flags = self.pop_byte(memory);
        4 // cycles
    }

    // === SYSTEM INSTRUCTIONS ===

    /// RTI - Return from Interrupt
    fn rti(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.flags = self.pop_byte(memory);
        self.p = self.pop_word(memory);
        self.interrupt_enabled = true;
        6 // cycles
    }

    /// SEI - Set Interrupt Enable
    fn sei(&mut self) -> u8 {
        self.interrupt_enabled = true;
        2 // cycles
    }

    /// CLI - Clear Interrupt Enable
    fn cli(&mut self) -> u8 {
        self.interrupt_enabled = false;
        2 // cycles
    }

    /// CLC - Clear Carry Flag
    fn clc(&mut self) -> u8 {
        self.set_carry_flag(false);
        2 // cycles
    }

    /// SEV - Set Overflow Flag
    fn sev(&mut self) -> u8 {
        self.set_overflow_flag(true);
        2 // cycles
    }

    /// CLV - Clear Overflow Flag
    fn clv(&mut self) -> u8 {
        self.set_overflow_flag(false);
        2 // cycles
    }

    // === BIT MANIPULATION INSTRUCTIONS ===

    /// BIT #nn - Bit Test Immediate
    fn bit_immediate(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let value = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        let result = self.a & value;
        self.set_zero_flag(result == 0);
        self.set_negative_flag(value & 0x80 != 0);
        self.set_overflow_flag(value & 0x40 != 0);
        3 // cycles
    }

    /// BIT nnnn - Bit Test Absolute
    fn bit_absolute(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        let addr = self.read_word(memory);
        let value = memory.read_byte(addr);
        let result = self.a & value;
        self.set_zero_flag(result == 0);
        self.set_negative_flag(value & 0x80 != 0);
        self.set_overflow_flag(value & 0x40 != 0);
        4 // cycles
    }

    // === SHIFT/ROTATE INSTRUCTIONS ===

    /// ASL A - Arithmetic Shift Left Accumulator
    fn asl_a(&mut self) -> u8 {
        self.set_carry_flag(self.a & 0x80 != 0);
        self.a <<= 1;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        2 // cycles
    }

    /// LSR A - Logical Shift Right Accumulator
    fn lsr_a(&mut self) -> u8 {
        self.set_carry_flag(self.a & 0x01 != 0);
        self.a >>= 1;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(false); // LSR always clears negative flag
        2 // cycles
    }

    /// ROL A - Rotate Left Accumulator
    fn rol_a(&mut self) -> u8 {
        let old_carry = if self.get_carry_flag() { 1 } else { 0 };
        self.set_carry_flag(self.a & 0x80 != 0);
        self.a = (self.a << 1) | old_carry;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        2 // cycles
    }

    /// ROR A - Rotate Right Accumulator
    fn ror_a(&mut self) -> u8 {
        let old_carry = if self.get_carry_flag() { 0x80 } else { 0 };
        self.set_carry_flag(self.a & 0x01 != 0);
        self.a = (self.a >> 1) | old_carry;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        2 // cycles
    }

    // === TRANSFER INSTRUCTIONS ===

    /// TAB - Transfer A to B
    fn tab(&mut self) -> u8 {
        self.b = self.a;
        self.set_zero_flag(self.b == 0);
        self.set_negative_flag(self.b & 0x80 != 0);
        2 // cycles
    }

    /// TBA - Transfer B to A
    fn tba(&mut self) -> u8 {
        self.a = self.b;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        2 // cycles
    }

    /// TAX - Transfer A to X (low byte)
    fn tax(&mut self) -> u8 {
        self.x = (self.x & 0xFF00) | (self.a as u16);
        // Flags typically not affected by 16-bit transfers in many processors
        2 // cycles
    }

    /// TXA - Transfer X to A (low byte)
    fn txa(&mut self) -> u8 {
        self.a = (self.x & 0xFF) as u8;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        2 // cycles
    }

    /// TAY - Transfer A to Y (low byte)
    fn tay(&mut self) -> u8 {
        self.y = (self.y & 0xFF00) | (self.a as u16);
        // Flags typically not affected by 16-bit transfers in many processors
        2 // cycles
    }

    /// TYA - Transfer Y to A (low byte)
    fn tya(&mut self) -> u8 {
        self.a = (self.y & 0xFF) as u8;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
        2 // cycles
    }

    /// TXS - Transfer X to Stack Pointer
    fn txs(&mut self) -> u8 {
        self.s = self.x;
        2 // cycles
    }

    /// TSX - Transfer Stack Pointer to X
    fn tsx(&mut self) -> u8 {
        self.x = self.s;
        2 // cycles
    }

    // === HELPER METHODS ===

    /// Read a 16-bit word from memory (little-endian)
    fn read_word(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u16 {
        let low = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        let high = memory.read_byte(self.p);
        self.p = self.p.wrapping_add(1);
        u16::from_le_bytes([low, high])
    }

    /// Push a byte onto the stack
    fn push_byte(&mut self, memory: &mut crate::pc1500::memory::MemoryBus, value: u8) {
        memory.write_byte(self.s, value);
        self.s = self.s.wrapping_sub(1);
    }

    /// Pop a byte from the stack
    fn pop_byte(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        self.s = self.s.wrapping_add(1);
        memory.read_byte(self.s)
    }

    /// Push a 16-bit word onto the stack (little-endian)
    fn push_word(&mut self, memory: &mut crate::pc1500::memory::MemoryBus, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.push_byte(memory, high);
        self.push_byte(memory, low);
    }

    /// Pop a 16-bit word from the stack (little-endian)
    fn pop_word(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u16 {
        let low = self.pop_byte(memory);
        let high = self.pop_byte(memory);
        u16::from_le_bytes([low, high])
    }

    /// Add with carry operation
    fn add_with_carry(&mut self, value: u8) {
        let carry = if self.get_carry_flag() { 1 } else { 0 };
        let result = self.a as u16 + value as u16 + carry;

        self.set_carry_flag(result > 0xFF);
        self.set_overflow_flag((self.a ^ value) & 0x80 == 0 && (self.a ^ result as u8) & 0x80 != 0);

        self.a = result as u8;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
    }

    /// Subtract with carry operation
    fn subtract_with_carry(&mut self, value: u8) {
        let carry = if self.get_carry_flag() { 0 } else { 1 };
        let result = self.a as i16 - value as i16 - carry;

        self.set_carry_flag(result >= 0);
        self.set_overflow_flag((self.a ^ value) & 0x80 != 0 && (self.a ^ result as u8) & 0x80 != 0);

        self.a = result as u8;
        self.set_zero_flag(self.a == 0);
        self.set_negative_flag(self.a & 0x80 != 0);
    }

    // === FLAG OPERATIONS ===

    fn get_zero_flag(&self) -> bool {
        self.flags & flags::ZERO != 0
    }

    pub(super) fn set_zero_flag(&mut self, value: bool) {
        if value {
            self.flags |= flags::ZERO;
        } else {
            self.flags &= !flags::ZERO;
        }
    }

    fn get_carry_flag(&self) -> bool {
        self.flags & flags::CARRY != 0
    }

    pub(super) fn set_carry_flag(&mut self, value: bool) {
        if value {
            self.flags |= flags::CARRY;
        } else {
            self.flags &= !flags::CARRY;
        }
    }

    fn set_overflow_flag(&mut self, value: bool) {
        if value {
            self.flags |= flags::OVERFLOW;
        } else {
            self.flags &= !flags::OVERFLOW;
        }
    }

    fn get_overflow_flag(&self) -> bool {
        self.flags & flags::OVERFLOW != 0
    }

    fn set_negative_flag(&mut self, value: bool) {
        if value {
            self.flags |= flags::NEGATIVE;
        } else {
            self.flags &= !flags::NEGATIVE;
        }
    }

    fn get_negative_flag(&self) -> bool {
        self.flags & flags::NEGATIVE != 0
    }
}

// CPU flag constants
#[allow(dead_code)]
pub mod flags {
    pub const ZERO: u8 = 0x01;
    pub const CARRY: u8 = 0x02;
    pub const OVERFLOW: u8 = 0x04;
    pub const NEGATIVE: u8 = 0x08;
}
