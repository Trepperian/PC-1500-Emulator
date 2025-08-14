/// PC-1500 Interrupt System Implementation
/// 
/// Based on PC-1500 Technical Manual interrupt processing diagrams:
/// - Timer interrupt processing sequence
/// - Maskable interrupt processing sequence  
/// - RTI (Return from interrupt) instruction support
/// - Interrupt enable/disable via IE flag

// PC-1500 interrupt types
const MASKABLE: u8 = 1;   // MI (Maskable Interrupt)
const TIMER: u8 = 2;      // Timer interrupt

/// Interrupt request flags
const IR1_ACTIVE: u8 = 1; // IR1 active (maskable interrupt)  
const IR2_ACTIVE: u8 = 2; // IR2 active (timer interrupt)

/// PC-1500 Interrupt Controller
/// Handles timer interrupts and maskable interrupts according to PC-1500 specifications
#[derive(Default, Debug)]
pub struct InterruptController {
    /// IE flag - Interrupt Enable (controlled by SIE/RIE instructions)
    ie_flag: bool,
    
    /// IR1 - Maskable interrupt request flag
    ir1_active: bool,
    
    /// IR2 - Timer interrupt request flag  
    ir2_active: bool,
    
    /// Currently processing interrupt (prevents nested interrupts)
    processing_interrupt: bool,
}

impl InterruptController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set IE flag (via SIE instruction)
    pub fn set_ie(&mut self) {
        self.ie_flag = true;
    }

    /// Reset IE flag (via RIE instruction)  
    pub fn reset_ie(&mut self) {
        self.ie_flag = false;
    }

    /// Get IE flag status
    pub const fn ie_enabled(&self) -> bool {
        self.ie_flag
    }

    /// Request timer interrupt (when timer overflows to 1FFH)
    pub fn request_timer_interrupt(&mut self) {
        self.ir2_active = true;
    }

    /// Request maskable interrupt (MI input sampled)
    pub fn request_maskable_interrupt(&mut self) {
        self.ir1_active = true;
    }

    /// Clear timer interrupt request
    pub fn clear_timer_interrupt(&mut self) {
        self.ir2_active = false;
    }

    /// Clear maskable interrupt request
    pub fn clear_maskable_interrupt(&mut self) {
        self.ir1_active = false;
    }

    /// Check if any interrupt is pending and enabled
    /// Returns the interrupt vector address if an interrupt should be processed
    pub fn check_pending_interrupt(&mut self) -> Option<u16> {
        // If IE flag is not set, no interrupts are processed
        if !self.ie_flag {
            return None;
        }

        // If already processing an interrupt, don't process another
        if self.processing_interrupt {
            return None;
        }

        // Timer interrupt has higher priority (IR2)
        if self.ir2_active {
            self.processing_interrupt = true;
            self.clear_timer_interrupt();
            // Timer interrupt vector from FFFAH/FFFBH addresses
            return Some(0xFFFB);
        }

        // Maskable interrupt (IR1) 
        if self.ir1_active {
            self.processing_interrupt = true;
            self.clear_maskable_interrupt();
            // Maskable interrupt vector (generic address for now)
            return Some(0xFFFA);
        }

        None
    }

    /// Start interrupt processing sequence
    /// Called when CPU begins executing interrupt
    pub fn start_interrupt_processing(&mut self) {
        self.processing_interrupt = true;
        // IE flag is automatically reset when interrupt processing starts
        self.ie_flag = false;
    }

    /// Complete interrupt processing (called by RTI instruction)
    /// This restores the IE flag and allows new interrupts
    pub fn complete_interrupt_processing(&mut self) {
        self.processing_interrupt = false;
        // Note: IE flag restoration is handled by RTI instruction
        // which restores the previous IE state from the stack
    }

    /// Check if timer interrupt is requested
    pub const fn timer_interrupt_requested(&self) -> bool {
        self.ir2_active
    }

    /// Check if maskable interrupt is requested  
    pub const fn maskable_interrupt_requested(&self) -> bool {
        self.ir1_active
    }

    /// Check if currently processing an interrupt
    pub const fn is_processing_interrupt(&self) -> bool {
        self.processing_interrupt
    }

    /// Get interrupt status for debugging
    pub fn get_status(&self) -> InterruptStatus {
        InterruptStatus {
            ie_flag: self.ie_flag,
            ir1_active: self.ir1_active,
            ir2_active: self.ir2_active,
            processing_interrupt: self.processing_interrupt,
        }
    }
}

/// Interrupt status for debugging and inspection
#[derive(Debug, Clone)]
pub struct InterruptStatus {
    pub ie_flag: bool,
    pub ir1_active: bool,
    pub ir2_active: bool,
    pub processing_interrupt: bool,
}
