/// PC-1500 Sharp Pocket Computer Emulation
///
/// This module contains the complete PC-1500 system implementation,
/// following the same modular structure as the GameBoy emulation.
pub mod cpu;
pub mod display;
pub mod joypad;
pub mod keyboard;
pub mod memory;

use crate::AudioCallback;
pub use cpu::Lh5801Cpu;
use display::DisplayController;
use keyboard::KeyboardController;
use memory::MemoryBus;

/// PC-1500 model variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pc1500Model {
    #[default]
    Pc1500, // Original PC-1500
    Pc1500A, // PC-1500A with more RAM
}

impl Pc1500Model {
    pub fn ram_size(&self) -> usize {
        match self {
            Pc1500Model::Pc1500 => 0x2000,  // 8KB
            Pc1500Model::Pc1500A => 0x2000, // Also 8KB but different layout
        }
    }
}

/// Main PC-1500 system - following the GameBoy structure pattern
pub struct Pc1500<A: AudioCallback> {
    // Core components (similar to GameBoy's structure)
    model: Pc1500Model,
    cpu: Lh5801Cpu,
    memory: MemoryBus,
    keyboard: KeyboardController,
    audio_callback: A,

    // Timing (following GameBoy pattern)
    cycles_run: u64,
    target_cycles_per_frame: u64,
}

impl<A: AudioCallback> Pc1500<A> {
    #[must_use]
    pub fn new(model: Pc1500Model, audio_callback: A) -> Self {
        Self {
            model,
            cpu: Lh5801Cpu::new(),
            memory: MemoryBus::new(),
            keyboard: KeyboardController::new(),
            audio_callback,
            cycles_run: 0,
            target_cycles_per_frame: 1000, // Placeholder - adjust for actual timing
        }
    }

    /// Load BIOS/ROM data into the system
    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), &'static str> {
        self.memory.load_rom(bios_data)
    }

    /// Soft reset the system (following GameBoy pattern)
    pub fn soft_reset(&mut self) {
        self.cpu.reset();
        self.keyboard.clear_all();
        self.memory.display_mut().clear();
        self.cycles_run = 0;
    }

    /// Change model and soft reset (following GameBoy pattern)
    pub fn change_model_and_soft_reset(&mut self, model: Pc1500Model) {
        self.model = model;
        self.soft_reset();
    }

    /// Run one CPU instruction (following GameBoy's run_cpu pattern)
    pub fn run_cpu(&mut self) {
        // Execute one CPU instruction
        let cycles = self.cpu.step(&mut self.memory);
        self.cycles_run += cycles as u64;

        // Handle interrupts if needed
        if self.cpu.should_handle_interrupt() {
            // TODO: Check for pending interrupts
            // For now, just keyboard interrupts
            self.cpu.handle_interrupt(0x0000);
        }
    }

    /// Execute one frame of emulation (following GameBoy's run_frame pattern)
    pub fn step_frame(&mut self) {
        let start_cycles = self.cycles_run;

        while (self.cycles_run - start_cycles) < self.target_cycles_per_frame {
            self.run_cpu();
        }

        // Audio callback (placeholder - PC-1500 has simple beeper)
        self.audio_callback.audio_sample(0i16, 0i16);
    }

    /// Get display for rendering
    pub fn display(&mut self) -> &mut DisplayController {
        self.memory.display_mut()
    }

    /// Get pixel data (following GameBoy's pixel_data_rgba pattern)
    #[must_use]
    pub fn pixel_data_rgba(&mut self) -> &[u8] {
        self.memory.display_mut().rgba_buffer()
    }

    /// Handle keyboard input (following GameBoy's press/release pattern)
    pub fn handle_key_input(&mut self, scancode: u32, pressed: bool) {
        self.keyboard.handle_pc_key(scancode, pressed);
    }

    /// Press a key (following GameBoy pattern - for compatibility)
    pub fn press_key(&mut self, scancode: u32) {
        self.handle_key_input(scancode, true);
    }

    /// Release a key (following GameBoy pattern - for compatibility)
    pub fn release_key(&mut self, scancode: u32) {
        self.handle_key_input(scancode, false);
    }

    /// Get model information (following GameBoy pattern)
    #[must_use]
    pub const fn model(&self) -> Pc1500Model {
        self.model
    }

    /// Get current CPU state (for debugging - following GameBoy pattern)
    #[must_use]
    pub const fn cpu(&self) -> &Lh5801Cpu {
        &self.cpu
    }

    /// Get cycles run (for debugging - following GameBoy pattern)
    #[must_use]
    pub const fn cycles_run(&self) -> u64 {
        self.cycles_run
    }

    /// Display a message on the LCD screen
    pub fn display_message(&mut self, message: &str) {
        self.memory.display_mut().show_status(message);
    }

    /// Display text centered on screen
    pub fn display_text_centered(&mut self, y: usize, text: &str) {
        self.memory.display_mut().draw_string_centered(y, text);
    }

    /// Display text at specific position
    pub fn display_text_at(&mut self, x: usize, y: usize, text: &str) {
        self.memory.display_mut().draw_string(x, y, text);
    }

    /// Clear the display
    pub fn clear_display(&mut self) {
        self.memory.display_mut().clear();
    }

    /// Show a test pattern with PC-1500 label
    pub fn show_test_pattern(&mut self) {
        self.memory.display_mut().test_pattern_with_text();
    }

    /// Write directly to memory (for testing memory mapping)
    pub fn write_memory(&mut self, address: u16, value: u8) {
        self.memory.write_byte(address, value);
    }

    /// Read directly from memory (for testing memory mapping)
    pub fn read_memory(&self, address: u16) -> u8 {
        self.memory.read_byte(address)
    }

    /// Write pattern to display memory for testing
    pub fn write_display_memory_pattern(&mut self) {
        // Write a test pattern to display memory (PC-1500 addresses: 0x7600-0x764F and 0x7700-0x774F)
        // First section (0x7600-0x764F)
        for i in 0..20u16 {
            let address = 0x7600 + i;
            let pattern = if i % 2 == 0 { 0xFF } else { 0x00 };
            self.write_memory(address, pattern);
        }
        // Second section (0x7700-0x774F)
        for i in 0..20u16 {
            let address = 0x7700 + i;
            let pattern = if i % 2 == 0 { 0xAA } else { 0x55 };
            self.write_memory(address, pattern);
        }
    }

    /// Get display memory range info (PC-1500 uses two sections)
    pub fn get_display_memory_info(&self) -> (u16, u16, usize) {
        (0x7600, 0x774F, 160) // (start_first_section, end_second_section, total_size)
    }

    /// Get ROM information
    pub fn get_rom_info(&self) -> (usize, &'static str) {
        self.memory.rom_info()
    }

    /// Read ROM byte at specific address (for inspection)
    pub fn read_rom_byte(&self, address: u16) -> u8 {
        if address <= 0x3FFF {
            self.memory.read_byte(address)
        } else {
            0xFF
        }
    }

    /// TEST MODE: Initialize system for testing without BIOS
    pub fn init_test_mode(&mut self) {
        // Reset the system
        self.soft_reset();

        // Set up a basic test program in memory
        // This will test our CPU instructions and display
        let test_program = [
            0x05, 0x55, // LDA #0x55 - Load 0x55 into accumulator
            0x0D, 0x00, 0x76, // STA 0x7600 - Store A into display memory start (CORRECTED)
            0x05, 0xAA, // LDA #0xAA - Load 0xAA into accumulator
            0x0D, 0x01, 0x76, // STA 0x7601 - Store A into display memory+1 (CORRECTED)
            0x05, 0xFF, // LDA #0xFF - Load 0xFF into accumulator
            0x0D, 0x02, 0x76, // STA 0x7602 - Store A into display memory+2 (CORRECTED)
            0x81, 0xF0, // BRA -16 (0xF0) - Branch backwards to create loop
        ];

        // Load test program at address 0x0000
        for (i, &byte) in test_program.iter().enumerate() {
            self.memory.write_byte(i as u16, byte);
        }

        // Fill some display memory for immediate visual feedback
        // First section (0x7600-0x764F)
        for i in 0..20 {
            let pattern = if i % 2 == 0 { 0xAA } else { 0x55 }; // Alternating pattern
            self.memory.write_byte(0x7600 + i, pattern);
        }

        // Set PC to start of our test program
        self.cpu.set_pc(0x0000);

        println!("PC-1500 Test Mode Initialized!");
        println!("- Test program loaded at 0x0000");
        println!("- Display pattern written to 0x7600-0x7613 (CORRECTED)");
        println!("- CPU PC set to 0x0000");
        println!("- Ready to execute and display!");
    }

    /// ROM MODE: Load real PC-1500 ROM and initialize for authentic execution
    pub fn load_rom(&mut self, rom_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔄 Loading PC-1500 ROM: {}", rom_path);
        
        // Read ROM file
        let rom_data = std::fs::read(rom_path)?;
        println!("✅ ROM file loaded: {} bytes", rom_data.len());
        
        // Validate ROM size (PC-1500 ROMs are typically 8KB, 16KB, or 32KB)
        match rom_data.len() {
            8192 => println!("📦 ROM Type: 8KB (PC-1500 basic)"),
            16384 => println!("📦 ROM Type: 16KB (PC-1500 extended)"),
            32768 => println!("📦 ROM Type: 32KB (PC-1500A full)"),
            _ => println!("⚠️  ROM Size: {} bytes (unusual size)", rom_data.len()),
        }
        
        // Reset system but DO NOT run init_test_mode
        self.soft_reset();
        
        // Load ROM data into memory starting at 0x0000
        for (i, &byte) in rom_data.iter().enumerate() {
            let address = i as u16;
            // Only load into ROM space (0x0000-0x7FFF)
            if address <= 0x7FFF {
                self.memory.write_byte(address, byte);
            }
        }
        
        // Clear display memory to ensure clean state
        // First section (0x7600-0x764F)
        for i in 0..80 {
            self.memory.write_byte(0x7600 + i, 0x00);
        }
        // Second section (0x7700-0x774F)  
        for i in 0..80 {
            self.memory.write_byte(0x7700 + i, 0x00);
        }
        
        // Set CPU to ROM entry point (PC-1500 ROMs typically start at 0x0000)
        self.cpu.set_pc(0x0000);
        
        println!("✅ ROM MODE Initialized:");
        println!("   - ROM loaded into memory (0x0000-0x{:04X})", 
                 std::cmp::min(rom_data.len() - 1, 0x7FFF));
        println!("   - Display memory cleared (0x7600-0x764F, 0x7700-0x774F)");
        println!("   - CPU PC set to 0x0000 (ROM entry point)");
        println!("   - System ready for authentic PC-1500 execution");
        println!("🚀 PC-1500 now running REAL ROM!");
        
        Ok(())
    }

    /// Check if system is running in ROM mode (has ROM loaded)
    pub fn is_rom_mode(&self) -> bool {
        // Check if there's actual ROM code at typical PC-1500 entry points
        // PC-1500 ROMs typically have specific patterns at start
        let first_bytes = [
            self.memory.read_byte(0x0000),
            self.memory.read_byte(0x0001),
            self.memory.read_byte(0x0002),
        ];
        
        // Our test mode starts with 0x05, 0x55 (LDA #0x55)
        // Real ROMs will have different patterns
        !(first_bytes[0] == 0x05 && first_bytes[1] == 0x55)
    }

    /// TEST MODE: Run a few CPU instructions and show state
    pub fn run_test_instructions(&mut self, count: usize) {
        println!("\n=== Running {} test instructions ===", count);

        for i in 0..count {
            let pc_before = self.cpu.p();
            let a_before = self.cpu.a();

            // Execute one instruction
            self.run_cpu();

            let pc_after = self.cpu.p();
            let a_after = self.cpu.a();

            println!(
                "Instruction {}: PC 0x{:04X} -> 0x{:04X}, A 0x{:02X} -> 0x{:02X}",
                i + 1,
                pc_before,
                pc_after,
                a_before,
                a_after
            );
        }

        self.print_test_state();
    }

    /// TEST MODE: Print current system state
    pub fn print_test_state(&self) {
        println!("\n=== PC-1500 System State ===");
        println!("CPU State:");
        println!("  A: 0x{:02X}  B: 0x{:02X}", self.cpu.a(), self.cpu.b());
        println!("  P: 0x{:04X}  S: 0x{:04X}", self.cpu.p(), self.cpu.s());
        println!(
            "  U: 0x{:04X}  X: 0x{:04X}  Y: 0x{:04X}",
            self.cpu.u(),
            self.cpu.x(),
            self.cpu.y()
        );
        println!("  Flags: 0x{:02X}", self.cpu.flags());
        println!("  Cycles: {}", self.cycles_run);

        println!("\nDisplay Memory (first 20 bytes - PC-1500 addresses):");
        print!("  0x7600: ");
        for i in 0..20 {
            let value = self.memory.read_byte(0x7600 + i);
            print!("{:02X} ", value);
            if (i + 1) % 8 == 0 {
                print!("\n  0x{:04X}: ", 0x7600 + i + 1);
            }
        }
        println!();
    }

    /// Reset the system
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.keyboard.clear_all();
        self.memory.display_mut().clear();
        self.cycles_run = 0;
    }

    /// Get CPU state information
    pub fn cpu_state(&self) -> CpuState {
        CpuState {
            pc: self.cpu.p(),
            a: self.cpu.a(),
            b: self.cpu.b(),
            x: self.cpu.x(),
            y: self.cpu.y(),
            u: self.cpu.u(),
            s: self.cpu.s(),
            flags: self.cpu.flags(),
        }
    }
}

/// CPU state information structure
#[derive(Debug, Clone)]
pub struct CpuState {
    pub pc: u16,    // Program counter
    pub a: u8,      // Accumulator
    pub b: u8,      // B register
    pub x: u16,     // X index register  
    pub y: u16,     // Y index register
    pub u: u16,     // U pointer register
    pub s: u16,     // Stack pointer
    pub flags: u8,  // Processor flags
}

/// Builder for PC-1500 system
pub struct Pc1500Builder {
    model: Pc1500Model,
    bios_data: Option<Vec<u8>>,
}

impl Pc1500Builder {
    pub fn new() -> Self {
        Self {
            model: Pc1500Model::default(),
            bios_data: None,
        }
    }

    pub fn model(mut self, model: Pc1500Model) -> Self {
        self.model = model;
        self
    }

    pub fn bios(mut self, bios_data: Vec<u8>) -> Self {
        self.bios_data = Some(bios_data);
        self
    }

    pub fn build<A: AudioCallback>(self, audio_callback: A) -> Result<Pc1500<A>, &'static str> {
        let mut system = Pc1500::new(self.model, audio_callback);

        if let Some(bios_data) = self.bios_data {
            system.load_bios(&bios_data)?;
        }

        Ok(system)
    }
}

impl Default for Pc1500Builder {
    fn default() -> Self {
        Self::new()
    }
}
