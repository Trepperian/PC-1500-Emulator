/// PC-1500 Timer Implementation
/// 
/// Based on PC-1500 Technical Manual Section 2-3-1:
/// - 9-bit polynomial counter (0x000-0x1FF)
/// - Set by AM0/AM1 CPU instructions
/// - Operates continuously, increments every φF cycle
/// - Issues interrupt request when reaching 1FFH
/// - With 4MHz crystal: φF = 31.25kHz, so timer increments every 32μsec

use core::time::Duration;

// PC-1500 specific timing constants
pub const FRAME_DURATION: Duration = Duration::new(0, 16_666_667); // ~60 Hz for PC-1500
pub const CYCLES_PER_FRAME: u32 = 128000; // Approximately for 4MHz / 60Hz

// Timer frequency: φF = 31.25kHz with 4MHz crystal (4MHz / 2 / 64 = 31.25kHz)
pub const TIMER_FREQUENCY_HZ: u32 = 31250; // 31.25 kHz
pub const CPU_FREQUENCY_HZ: u32 = 4_000_000; // 4 MHz
pub const TIMER_CYCLES_PER_INCREMENT: u32 = CPU_FREQUENCY_HZ / TIMER_FREQUENCY_HZ; // 128 cycles

#[derive(Default, Debug)]
pub struct Timer {
    /// 9-bit timer counter (0x000-0x1FF)
    counter: u16,
    /// Accumulated CPU cycles for timer increment
    cycle_accumulator: u32,
    /// Timer enabled state
    enabled: bool,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            counter: 0,
            cycle_accumulator: 0,
            enabled: false,
        }
    }

    /// Set timer value using AM0 instruction (TM8 = 0)
    pub fn set_am0(&mut self, value: u8) {
        self.counter = value as u16; // TM8 = 0, so only lower 8 bits
        self.enabled = true;
    }

    /// Set timer value using AM1 instruction (TM8 = 1) 
    pub fn set_am1(&mut self, value: u8) {
        self.counter = (value as u16) | 0x100; // TM8 = 1, so bit 8 is set
        self.enabled = true;
    }

    /// Get current timer counter value
    pub const fn counter(&self) -> u16 {
        self.counter
    }

    /// Disable timer (set to 000H when not used)
    pub fn disable(&mut self) {
        self.counter = 0;
        self.cycle_accumulator = 0;
        self.enabled = false;
    }

    /// Run timer for given number of CPU cycles
    /// Returns true if timer overflowed (reached 1FFH) and interrupt should be requested
    pub fn run_cycles(&mut self, cycles: u32) -> bool {
        if !self.enabled {
            return false;
        }

        self.cycle_accumulator += cycles;
        let mut interrupt_requested = false;

        // Each timer increment happens every TIMER_CYCLES_PER_INCREMENT CPU cycles
        while self.cycle_accumulator >= TIMER_CYCLES_PER_INCREMENT {
            self.cycle_accumulator -= TIMER_CYCLES_PER_INCREMENT;
            
            self.counter += 1;
            
            // Check for overflow at 1FFH (9-bit counter)
            if self.counter > 0x1FF {
                self.counter = 0; // Reset to 0 after overflow
                interrupt_requested = true;
            }
        }

        interrupt_requested
    }

    /// Check if timer is enabled
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// PC-1500 System implementation with timer support
pub struct Pc1500System {
    timer: Timer,
}

impl Pc1500System {
    pub fn new() -> Self {
        Self {
            timer: Timer::new(),
        }
    }

    /// Run system for given number of CPU cycles
    /// Returns true if timer interrupt should be processed
    pub fn run_cycles(&mut self, cycles: u32) -> bool {
        self.timer.run_cycles(cycles)
    }

    /// Handle AM0 instruction from CPU (Acc to Timer with TM8=0)
    pub fn cpu_am0_instruction(&mut self, accumulator: u8) {
        self.timer.set_am0(accumulator);
    }

    /// Handle AM1 instruction from CPU (Acc to Timer with TM8=1)  
    pub fn cpu_am1_instruction(&mut self, accumulator: u8) {
        self.timer.set_am1(accumulator);
    }

    /// Get timer counter for debugging
    pub const fn timer_counter(&self) -> u16 {
        self.timer.counter()
    }

    /// Check if timer is enabled
    pub const fn timer_enabled(&self) -> bool {
        self.timer.is_enabled()
    }

    /// Disable timer
    pub fn disable_timer(&mut self) {
        self.timer.disable();
    }
}

impl Default for Pc1500System {
    fn default() -> Self {
        Self::new()
    }
}
