/// PC-1500 Control Instructions Implementation
/// 
/// This module contains the implementation of all CPU control instructions
/// for the Sharp PC-1500 pocket computer using the LH5801 microprocessor.
/// 
/// Based on official LH5801 documentation and PC-1500 technical specifications.

use super::cpu::Lh5801Cpu;

impl Lh5801Cpu {
    
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
    /// Cycles: 2
    /// Notes: Contents of accumulator sent on data bus, may be used for latch IC clock
    pub(super) fn atp(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        // Send accumulator value to output port
        // In PC-1500, this is used for controlling external devices
        memory.write_output_port(self.a);
        2 // 2 cycles
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
    /// Cycles: 1
    /// Notes: No change takes place in flags
    pub(super) fn spv(&mut self) -> u8 {
        // Set PV flipflop (general purpose control bit)
        self.pv_flipflop = true;
        1 // 1 cycle
    }
    
    /// RPV (Reset PV) - Resets the general purpose flipflop PV
    /// Format: RPV
    /// Operation: 0 → PV
    /// Cycles: 1
    /// Notes: No change takes place in flags
    pub(super) fn rpv(&mut self) -> u8 {
        // Reset PV flipflop
        self.pv_flipflop = false;
        1 // 1 cycle
    }
    
    // ========================================================================
    // DISPLAY CONTROL INSTRUCTIONS
    // ========================================================================
    
    /// SDP (Set DisP) - Sets the LCD on/off control flipflop DISP
    /// Format: SDP
    /// Operation: 1 → DISP
    /// Cycles: 2
    /// Notes: On pattern signal generated from CPU internal LCD backplate signal lines (H0-H7)
    pub(super) fn sdp(&mut self) -> u8 {
        // Enable display
        self.display_enabled = true;
        2 // 2 cycles
    }
    
    /// RDP (Reset DisP) - Resets the LCD on/off control flipflop DISP  
    /// Format: RDP
    /// Operation: 0 → DISP
    /// Cycles: 2
    /// Notes: Off pattern signal generated from CPU internal LCD backplate signal lines (H0-H7)
    pub(super) fn rdp(&mut self) -> u8 {
        // Disable display
        self.display_enabled = false;
        2 // 2 cycles
    }
    
    // ========================================================================
    // INPUT/OUTPUT INSTRUCTIONS
    // ========================================================================
    
    /// ITA (In To Acc) - Transfers input port contents to accumulator
    /// Format: ITA
    /// Operation: IN0-7 → Accumulator
    /// Cycles: 2
    /// Notes: Only the flag Z changes based on result
    pub(super) fn ita(&mut self, memory: &mut crate::pc1500::memory::MemoryBus) -> u8 {
        // Read keyboard state from the keyboard controller
        let keyboard_value = memory.read_keyboard_input();
        
        // Load the keyboard input into the accumulator
        self.a = keyboard_value;
        
        // Set flags based on the loaded value (only Z flag changes per documentation)
        self.set_zero_flag(keyboard_value == 0);
        
        2 // 2 cycles per official documentation
    }
    
    // ========================================================================
    // INTERRUPT CONTROL INSTRUCTIONS
    // ========================================================================
    
    /// SIE (Set IE) - Sets the interrupt enable flag IE
    /// Format: SIE
    /// Operation: 1 → IE
    /// Cycles: 1
    /// Notes: After this, it becomes ready for maskable interrupt and timer interrupt acknowledge
    pub(super) fn sie(&mut self) -> u8 {
        self.interrupt_enabled = true;
        1 // 1 cycle
    }
    
    /// RIE (Reset IE) - Resets the interrupt enable flag IE
    /// Format: RIE
    /// Operation: 0 → IE
    /// Cycles: 1
    /// Notes: After this, maskable interrupt and timer interrupt are disabled
    pub(super) fn rie(&mut self) -> u8 {
        self.interrupt_enabled = false;
        1 // 1 cycle
    }
    
    // ========================================================================
    // TIMER CONTROL INSTRUCTIONS
    // ========================================================================
    
    /// AM0 (Acc to Tm and 0) - Transfers accumulator to timer register and sets TM8 to 0
    /// Format: AM0
    /// Operation: A → TM (TM0~TM7), 0 → TM8
    /// Cycles: 2
    /// Notes: No change takes place in other flags
    pub(super) fn am0(&mut self) -> u8 {
        // Transfer accumulator to timer register (low 8 bits)
        self.timer_register = (self.timer_register & 0xFF00) | (self.a as u16);
        // Clear TM8 (bit 8 of timer register)
        self.timer_register &= 0x00FF;
        2 // 2 cycles
    }
    
    /// AM1 (Acc to Tm and 1) - Transfers accumulator to timer register and sets TM8 to 1
    /// Format: AM1
    /// Operation: A → TM (TM0~TM7), 1 → TM8
    /// Cycles: 2
    /// Notes: Same as AM0, but "1" is entered in the highest order bit
    pub(super) fn am1(&mut self) -> u8 {
        // Transfer accumulator to timer register (low 8 bits)
        self.timer_register = (self.timer_register & 0xFF00) | (self.a as u16);
        // Set TM8 (bit 8 of timer register)
        self.timer_register |= 0x0100;
        2 // 2 cycles
    }
    
    // ========================================================================
    // SYSTEM OPERATION INSTRUCTIONS
    // ========================================================================
    
    /// NOP (No Operation) - Does nothing for one cycle
    /// Format: NOP
    /// Operation: No operation
    /// Cycles: 1
    pub(super) fn nop(&mut self) -> u8 {
        // Do nothing
        1 // 1 cycle
    }
    
    /// HLT (Halt) - Stops CPU operation
    /// Format: HLT
    /// Operation: Stops CPU operation (only divider remains in operation)
    /// Cycles: 2
    /// Notes: Released from stop by interrupt. No change takes place in flags.
    pub(super) fn hlt(&mut self) -> u8 {
        // Set CPU to halted state
        self.halted = true;
        2 // 2 cycles
    }
    
    /// OFF - BF flipflop reset instruction
    /// Format: OFF
    /// Operation: BF flipflop reset
    /// Cycles: 2
    /// Notes: No change takes place in flags
    pub(super) fn off(&mut self) -> u8 {
        // Reset BF flipflop (power management)
        self.bf_flipflop = false;
        2 // 2 cycles
    }
}
