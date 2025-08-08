/// PC-1500 Memory Writer with Visual Display
/// 
/// This program lets you write to memory and see the results immediately

mod video;

use ceres_core::{AudioCallback, Pc1500, Pc1500Model, Sample};
use ceres_std::{ShaderOption, wgpu_renderer::ScalingOption};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::KeyCode,
    window::{Window, WindowId},
};

/// Audio callback for PC-1500
struct Pc1500AudioCallback;

impl AudioCallback for Pc1500AudioCallback {
    fn audio_sample(&self, _l: Sample, _r: Sample) {
        // PC-1500 beeper would go here
    }
}

struct Windows {
    main: video::State<'static, 156, 7>, // PC-1500 display is 156x7 pixels (CORRECTED)
}

/// PC-1500 Memory Writer Application
pub struct Pc1500MemoryWriter {
    pc1500: Option<Pc1500<Pc1500AudioCallback>>,
    windows: Option<Windows>,
    current_address: u16,
    writing_mode: bool,
    hex_input_mode: bool,
    hex_input_buffer: String,
}

impl Pc1500MemoryWriter {
    pub fn new() -> Self {
        let audio_callback = Pc1500AudioCallback;
        let mut pc1500 = Pc1500::new(Pc1500Model::Pc1500, audio_callback);
        
        // Initialize and show READY message (Test Mode by default)
        pc1500.init_test_mode();
        pc1500.display_message("READY");
        
        Self {
            pc1500: Some(pc1500),
            windows: None,
            current_address: 0x7600,
            writing_mode: false,
            hex_input_mode: false,
            hex_input_buffer: String::new(),
        }
    }
    
    /// Create new instance with ROM loaded
    pub fn new_with_rom(rom_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let audio_callback = Pc1500AudioCallback;
        let mut pc1500 = Pc1500::new(Pc1500Model::Pc1500, audio_callback);
        
        // Load ROM instead of test mode
        pc1500.load_rom(rom_path)?;
        
        Ok(Self {
            pc1500: Some(pc1500),
            windows: None,
            current_address: 0x7600,
            writing_mode: false,
            hex_input_mode: false,
            hex_input_buffer: String::new(),
        })
    }
    
    fn step_frame(&mut self) {
        if let Some(pc1500) = &mut self.pc1500 {
            pc1500.step_frame();
        }
    }
    
    fn update_display(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let (Some(pc1500), Some(windows)) = (&mut self.pc1500, &mut self.windows) {
            let pixel_data = pc1500.display().rgba_buffer();
            windows.main.update_texture(pixel_data);
        }
        Ok(())
    }
    
    fn show_memory_interface(&mut self) {
        if let Some(pc1500) = &mut self.pc1500 {
            // Just read the current value - no display interference
            let (section_name, col) = if self.current_address >= 0x7700 {
                ("Second section", self.current_address - 0x7700)
            } else {
                ("First section", self.current_address - 0x7600)
            };
            
            let current_value = pc1500.read_memory(self.current_address);
            
            // Show PC-1500 specific information in console only
            let first_char = current_value & 0x0F;          // bits 0,1,2,3 (4 DOTS)
            let second_char = (current_value >> 4) & 0x07;  // bits 4,5,6 (3 DOTS)
            
            println!("📍 Current Position → {} Column {}: 0x{:02X} = {:08b} (Address 0x{:04X})", 
                    section_name, col, current_value, current_value, self.current_address);
            println!("  PC-1500 7-dot display (two hex chars):");
            println!("  First char (4 DOTS): 0x{:X} (bits 0,1,2,3)", first_char);
            println!("  Second char (3 DOTS): 0x{:X} (bits 4,5,6)", second_char);
            println!("  Visual pattern (top to bottom):");
            println!("    4 DOTS (first char 0x{:X}):", first_char);
            for bit in 0..4 {
                let bit_set = (current_value >> bit) & 1 != 0;
                println!("      Bit {}: {}", bit, if bit_set { "█" } else { "·" });
            }
            println!("    ----");
            println!("    3 DOTS (second char 0x{:X}):", second_char);
            for bit in 4..7 {
                let bit_set = (current_value >> bit) & 1 != 0;
                println!("      Bit {}: {}", bit, if bit_set { "█" } else { "·" });
            }
        }
    }
    
    fn show_display_map(&mut self) {
        if let Some(pc1500) = &mut self.pc1500 {
            println!("\n📺 === PC-1500 DISPLAY MEMORY MAP ===");
            println!("Each column has 7 dots: 4 DOTS (upper) + 3 DOTS (lower)");
            println!("4 DOTS: bits 0,1,2,3 (first hex char 0-F)");
            println!("3 DOTS: bits 4,5,6 (second hex char 0-7)");
            println!("Bit order: 0 (top) → 6 (bottom)");
            println!("Each column = two hex chars (00-FF)");
            println!("First section (0x7600-0x764F): 80 columns");
            println!("Second section (0x7700-0x774F): 80 columns");
            println!("Showing first 40 columns of each section:");
            println!();
            
            // First section (0x7600-0x764F)
            println!("=== FIRST SECTION (0x7600-0x764F) ===");
            
            // Show header with column numbers
            print!("COL: ");
            for col in 0..40 {
                print!("{:2} ", col);
            }
            println!();
            
            // Show 4 DOTS section (bits 0,1,2,3) - top to bottom
            println!("4 DOTS (upper section - bits 0,1,2,3):");
            for row in 0..4 { // bits 0,1,2,3 from top to bottom
                print!("B{}: ", row);
                for col in 0..40 {
                    let value = pc1500.read_memory(0x7600 + col);
                    let bit_set = (value >> row) & 1 != 0;
                    print!(" {} ", if bit_set { "█" } else { "·" });
                }
                println!();
            }
            
            // Show separator
            print!("    ");
            for _ in 0..40 {
                print!(" - ");
            }
            println!();
            
            // Show 3 DOTS section (bits 4,5,6) - top to bottom
            println!("3 DOTS (lower section - bits 4,5,6):");
            for row in 4..7 { // bits 4,5,6 from top to bottom
                print!("B{}: ", row);
                for col in 0..40 {
                    let value = pc1500.read_memory(0x7600 + col);
                    let bit_set = (value >> row) & 1 != 0;
                    print!(" {} ", if bit_set { "█" } else { "·" });
                }
                println!();
            }
            
            // Show hex values and interpretation
            print!("HEX: ");
            for col in 0..40 {
                let value = pc1500.read_memory(0x7600 + col);
                print!("{:02X} ", value);
            }
            println!();
            
            print!("1ST: ");
            for col in 0..40 {
                let value = pc1500.read_memory(0x7600 + col);
                let first_char = value & 0x0F; // bits 0,1,2,3
                print!("{:X}  ", first_char);
            }
            println!();
            
            print!("2ND: ");
            for col in 0..40 {
                let value = pc1500.read_memory(0x7600 + col);
                let second_char = (value >> 4) & 0x07; // bits 4,5,6
                print!("{:X}  ", second_char);
            }
            println!();
            
            // Second section (0x7700-0x774F)
            println!("\n=== SECOND SECTION (0x7700-0x774F) ===");
            
            // Show header with column numbers  
            print!("COL: ");
            for col in 0..40 {
                print!("{:2} ", col);
            }
            println!();
            
            // Show 4 DOTS section (bits 0,1,2,3) - top to bottom
            println!("4 DOTS (upper section - bits 0,1,2,3):");
            for row in 0..4 { // bits 0,1,2,3 from top to bottom
                print!("B{}: ", row);
                for col in 0..40 {
                    let value = pc1500.read_memory(0x7700 + col);
                    let bit_set = (value >> row) & 1 != 0;
                    print!(" {} ", if bit_set { "█" } else { "·" });
                }
                println!();
            }
            
            // Show separator
            print!("    ");
            for _ in 0..40 {
                print!(" - ");
            }
            println!();
            
            // Show 3 DOTS section (bits 4,5,6) - top to bottom
            println!("3 DOTS (lower section - bits 4,5,6):");
            for row in 4..7 { // bits 4,5,6 from top to bottom
                print!("B{}: ", row);
                for col in 0..40 {
                    let value = pc1500.read_memory(0x7700 + col);
                    let bit_set = (value >> row) & 1 != 0;
                    print!(" {} ", if bit_set { "█" } else { "·" });
                }
                println!();
            }
            
            // Show hex values and interpretation
            print!("HEX: ");
            for col in 0..40 {
                let value = pc1500.read_memory(0x7700 + col);
                print!("{:02X} ", value);
            }
            println!();
            
            print!("1ST: ");
            for col in 0..40 {
                let value = pc1500.read_memory(0x7700 + col);
                let first_char = value & 0x0F; // bits 0,1,2,3
                print!("{:X}  ", first_char);
            }
            println!();
            
            print!("2ND: ");
            for col in 0..40 {
                let value = pc1500.read_memory(0x7700 + col);
                let second_char = (value >> 4) & 0x07; // bits 4,5,6
                print!("{:X}  ", second_char);
            }
            println!();
            
            println!("===============================\n");
        }
    }
    
    fn write_memory_value(&mut self, value: u8) {
        if let Some(pc1500) = &mut self.pc1500 {
            // Write the value to display memory
            pc1500.write_memory(self.current_address, value);
            
            // Verify it was written correctly
            let read_back = pc1500.read_memory(self.current_address);
            println!("✅ Written 0x{:02X} to 0x{:04X} - Read back: 0x{:02X}", 
                    value, self.current_address, read_back);
            
            // Force display refresh to show the new pattern immediately
            if let Some(windows) = &mut self.windows {
                let pixel_data = pc1500.display().rgba_buffer();
                windows.main.update_texture(pixel_data);
                windows.main.window().request_redraw();
            }
            
            // Show PC-1500 specific bit pattern info
            let (section_name, col) = if self.current_address >= 0x7700 {
                ("Second section", self.current_address - 0x7700)
            } else {
                ("First section", self.current_address - 0x7600)
            };
            
            let first_char = value & 0x0F;          // bits 0,1,2,3 (4 DOTS upper)
            let second_char = (value >> 4) & 0x07;  // bits 4,5,6 (3 DOTS lower)
            let bit7_ignored = (value >> 7) & 1;    // bit 7 (ignored by PC-1500)
            
            println!("{} Column {}: 0x{:02X} = {:08b} (Address 0x{:04X})", 
                    section_name, col, value, value, self.current_address);
            println!("  PC-1500 7-dot display interpretation:");
            println!("  First char (4 DOTS): 0x{:X} = {:04b} (bits 0,1,2,3)", first_char, first_char);
            println!("  Second char (3 DOTS): 0x{:X} = {:03b} (bits 4,5,6)", second_char, second_char);
            if bit7_ignored != 0 {
                println!("  ⚠️  Bit 7 = {} (IGNORED by PC-1500 display)", bit7_ignored);
            }
            
            // Visual representation of the 7 dots (top to bottom: 0→6)
            println!("  Visual pattern (7 dots, top to bottom):");
            println!("    4 DOTS (first char 0x{:X}):", first_char);
            for bit in 0..4 { // bits 0,1,2,3 from top to bottom
                let bit_set = (value >> bit) & 1 != 0;
                println!("      Bit {}: {}", bit, if bit_set { "█" } else { "·" });
            }
            println!("    ----");
            println!("    3 DOTS (second char 0x{:X}):", second_char);
            for bit in 4..7 { // bits 4,5,6 from top to bottom
                let bit_set = (value >> bit) & 1 != 0;
                println!("      Bit {}: {}", bit, if bit_set { "█" } else { "·" });
            }
        }
    }
    
    fn start_hex_input(&mut self) {
        self.hex_input_mode = true;
        self.hex_input_buffer.clear();
        println!("🔤 HEX INPUT MODE: Enter two hex chars (AB format)");
        println!("   First char (A) → 4 DOTS (bits 0,1,2,3)");
        println!("   Second char (B) → 3 DOTS (bits 4,5,6)");
        println!("   Examples: AB, 3F, 72, FF, etc.");
        println!("   Press ENTER to confirm, ESC to cancel");
    }
    
    fn handle_hex_input(&mut self, key_code: KeyCode) -> bool {
        match key_code {
            // Hex digits
            KeyCode::Digit0 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('0'); } true }
            KeyCode::Digit1 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('1'); } true }
            KeyCode::Digit2 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('2'); } true }
            KeyCode::Digit3 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('3'); } true }
            KeyCode::Digit4 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('4'); } true }
            KeyCode::Digit5 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('5'); } true }
            KeyCode::Digit6 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('6'); } true }
            KeyCode::Digit7 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('7'); } true }
            KeyCode::Digit8 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('8'); } true }
            KeyCode::Digit9 => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('9'); } true }
            KeyCode::KeyA => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('A'); } true }
            KeyCode::KeyB => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('B'); } true }
            KeyCode::KeyC => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('C'); } true }
            KeyCode::KeyD => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('D'); } true }
            KeyCode::KeyE => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('E'); } true }
            KeyCode::KeyF => { if self.hex_input_buffer.len() < 2 { self.hex_input_buffer.push('F'); } true }
            
            // Backspace
            KeyCode::Backspace => {
                self.hex_input_buffer.pop();
                true
            }
            
            // Enter - confirm input
            KeyCode::Enter => {
                if self.hex_input_buffer.len() == 2 {
                    // Parse AB where A=first char, B=second char
                    let chars: Vec<char> = self.hex_input_buffer.chars().collect();
                    if let (Some(first_char), Some(second_char)) = (
                        chars[0].to_digit(16),
                        chars[1].to_digit(16)
                    ) {
                        // PC-1500 format: first char in bits 0,1,2,3 and second char in bits 4,5,6
                        let pc1500_value = (first_char & 0x0F) | ((second_char & 0x07) << 4);
                        
                        println!("✅ Hex input: {} → First char (4 DOTS): 0x{:X}, Second char (3 DOTS): 0x{:X}", 
                                self.hex_input_buffer, first_char, second_char & 0x07);
                        println!("   PC-1500 value: 0x{:02X}", pc1500_value);
                        self.write_memory_value(pc1500_value as u8);
                        self.hex_input_mode = false;
                        self.hex_input_buffer.clear();
                        return false; // Don't show status after processing
                    } else {
                        println!("❌ Invalid hex input: {}", self.hex_input_buffer);
                    }
                } else if self.hex_input_buffer.len() == 1 {
                    // Auto-pad with 0 if only one digit - treat as first char only
                    if let Some(first_char) = self.hex_input_buffer.chars().next().unwrap().to_digit(16) {
                        let pc1500_value = first_char & 0x0F; // Only in bits 0,1,2,3 (4 DOTS)
                        println!("✅ Hex input: {} → First char (4 DOTS): 0x{:X}, Second char (3 DOTS): 0x0", 
                                self.hex_input_buffer, first_char);
                        println!("   PC-1500 value: 0x{:02X}", pc1500_value);
                        self.write_memory_value(pc1500_value as u8);
                        self.hex_input_mode = false;
                        self.hex_input_buffer.clear();
                        return false; // Don't show status after processing
                    }
                } else {
                    println!("❌ Please enter 1 or 2 hex digits");
                }
                true
            }
            
            // Escape - cancel
            KeyCode::Escape => {
                println!("❌ Hex input cancelled");
                self.hex_input_mode = false;
                self.hex_input_buffer.clear();
                false // Don't show status after cancelling
            }
            
            // Invalid hex characters - provide feedback
            _ => {
                println!("❌ Invalid hex character! Use only 0-9 and A-F");
                false
            }
        }
    }
    
    fn show_hex_input_status(&self) {
        if self.hex_input_mode {
            print!("🔤 HEX INPUT: {} ", self.hex_input_buffer);
            for _ in self.hex_input_buffer.len()..2 {
                print!("_");
            }
            println!(" ({})", if self.hex_input_buffer.len() < 2 { "type more digits" } else { "press ENTER" });
        }
    }
    
    fn handle_keyboard_input(&mut self, key_event: &KeyEvent, pressed: bool) {
        if let Some(pc1500) = &mut self.pc1500 {
            if let winit::keyboard::PhysicalKey::Code(key_code) = key_event.physical_key {
                if pressed {
                    // Handle hex input mode first
                    if self.hex_input_mode {
                        if self.handle_hex_input(key_code) {
                            self.show_hex_input_status();
                        }
                        // ALWAYS return when in hex input mode to prevent other key processing
                        return;
                    }
                    
                    match key_code {
                        // Mode switching
                        KeyCode::Space => {
                            if !self.writing_mode {
                                self.writing_mode = true;
                                // Clear both memory sections when entering writing mode
                                for i in 0..80 { // First section: 0x7600-0x764F
                                    pc1500.write_memory(0x7600 + i, 0x00);
                                }
                                for i in 0..80 { // Second section: 0x7700-0x774F
                                    pc1500.write_memory(0x7700 + i, 0x00);
                                }
                                println!("✏️  WRITING MODE ACTIVATED");
                                println!("🎯 You can now write to memory and see immediate visual feedback");
                                self.show_memory_interface();
                            }
                        }
                        KeyCode::KeyR => {
                            self.writing_mode = false;
                            self.hex_input_mode = false;
                            self.hex_input_buffer.clear();
                            // Clear both memory sections and show READY message
                            for i in 0..80 { // First section: 0x7600-0x764F
                                pc1500.write_memory(0x7600 + i, 0x00);
                            }
                            for i in 0..80 { // Second section: 0x7700-0x774F
                                pc1500.write_memory(0x7700 + i, 0x00);
                            }
                            pc1500.display_message("READY");
                            println!("🔄 Reset to READY mode");
                        }
                        KeyCode::KeyL => {
                            // Load ROM mode
                            println!("🔄 Attempting to load PC-1500 ROM...");
                            match pc1500.load_rom("PC-1500_A04.ROM") {
                                Ok(()) => {
                                    println!("✅ ROM loaded successfully! PC-1500 now running authentic firmware.");
                                    println!("💡 The system is now executing real PC-1500 code.");
                                    println!("   Display updates will come from ROM execution, not manual writing.");
                                    println!("   Press M to see memory map, W to enter writing mode for testing.");
                                }
                                Err(e) => {
                                    println!("❌ Failed to load ROM: {}", e);
                                    println!("💡 Make sure PC-1500_A04.ROM is in the current directory");
                                    println!("   Continuing in test mode...");
                                }
                            }
                        }
                        KeyCode::KeyI => {
                            // Show system information
                            if pc1500.is_rom_mode() {
                                println!("📋 SYSTEM INFO - ROM MODE ACTIVE");
                                println!("✅ Running authentic PC-1500 firmware");
                                println!("   ROM loaded and executing from address 0x0000");
                                println!("   Display controlled by ROM code");
                                println!("   CPU executing real PC-1500 instructions");
                            } else {
                                println!("📋 SYSTEM INFO - TEST MODE ACTIVE");
                                println!("🧪 Running test program (not authentic ROM)");
                                println!("   Test code loaded at address 0x0000");
                                println!("   Display available for manual writing");
                                println!("   Press L to load real ROM");
                            }
                            
                            // Show current CPU state
                            println!("📊 CPU STATE:");
                            println!("   PC: 0x{:04X}", pc1500.cpu_state().pc);
                            println!("   A:  0x{:02X}", pc1500.cpu_state().a);
                            println!("   X:  0x{:04X}", pc1500.cpu_state().x);
                            println!("   Y:  0x{:04X}", pc1500.cpu_state().y);
                            println!("   U:  0x{:04X}", pc1500.cpu_state().u);
                            println!("   S:  0x{:04X}", pc1500.cpu_state().s);
                        }
                        KeyCode::KeyM => {
                            // Show memory map
                            self.show_display_map();
                        }
                        KeyCode::KeyH => {
                            // Start hex input mode
                            if self.writing_mode {
                                self.start_hex_input();
                            }
                        }
                        KeyCode::KeyT => {
                            // Test pattern to verify PC-1500 two-char system
                            if self.writing_mode {
                                println!("🧪 Testing PC-1500 two hex char system...");
                                
                                // Clear both sections first
                                for i in 0..80 {
                                    pc1500.write_memory(0x7600 + i, 0x00);
                                    pc1500.write_memory(0x7700 + i, 0x00);
                                }
                                
                                // Test first char (bits 0,1,2,3) - 4 DOTS in first section
                                println!("First section tests:");
                                for i in 0..16u16 {
                                    let pattern = i as u8; // First char 0-F in bits 0,1,2,3
                                    pc1500.write_memory(0x7600 + 20 + i, pattern);
                                    println!("  Col {}: First=0x{:X}, Second=0x0 (pattern 0x{:02X})", 20 + i, i, pattern);
                                }
                                // Test second char (bits 4,5,6) - 3 DOTS in first section
                                for i in 0..8u16 {
                                    let pattern = (i << 4) as u8; // Second char 0-7 in bits 4,5,6
                                    pc1500.write_memory(0x7600 + 40 + i, pattern);
                                    println!("  Col {}: First=0x0, Second=0x{:X} (pattern 0x{:02X})", 40 + i, i, pattern);
                                }
                                
                                // Test patterns in second section
                                println!("Second section tests:");
                                // Test combined patterns
                                for i in 0..8u16 {
                                    let first_char = i + 8;  // 8-F
                                    let second_char = i;     // 0-7
                                    let pattern = (first_char | (second_char << 4)) as u8;
                                    pc1500.write_memory(0x7700 + i, pattern);
                                    println!("  Col {}: First=0x{:X}, Second=0x{:X} (pattern 0x{:02X})", 
                                            i, first_char, second_char, pattern);
                                }
                                // Test full patterns in second section
                                for i in 0..16u16 {
                                    let pattern = (0x70 | i) as u8; // Mixed pattern
                                    pc1500.write_memory(0x7700 + 20 + i, pattern);
                                    let first_char = pattern & 0x0F;
                                    let second_char = (pattern >> 4) & 0x07;
                                    println!("  Col {}: First=0x{:X}, Second=0x{:X} (pattern 0x{:02X})", 
                                            20 + i, first_char, second_char, pattern);
                                }
                                
                                println!("Check first section - columns 20-35: First char test (0-F)");
                                println!("Check first section - columns 40-47: Second char test (0-7)");  
                                println!("Check second section - columns 0-7: Combined patterns");
                                println!("Check second section - columns 20-35: Full patterns");
                            }
                        }
                        
                        // Memory writing (only in writing mode)
                        _ if self.writing_mode => match key_code {
                            // Valid hexadecimal values only (0-9, A-F)
                            // Single hex character input (first char only, second char = 0)
                            KeyCode::Digit0 => self.write_memory_value(0x00), // First char: 0, Second char: 0
                            KeyCode::Digit1 => self.write_memory_value(0x01), // First char: 1, Second char: 0 
                            KeyCode::Digit2 => self.write_memory_value(0x02), // First char: 2, Second char: 0
                            KeyCode::Digit3 => self.write_memory_value(0x03), // First char: 3, Second char: 0
                            KeyCode::Digit4 => self.write_memory_value(0x04), // First char: 4, Second char: 0
                            KeyCode::Digit5 => self.write_memory_value(0x05), // First char: 5, Second char: 0
                            KeyCode::Digit6 => self.write_memory_value(0x06), // First char: 6, Second char: 0
                            KeyCode::Digit7 => self.write_memory_value(0x07), // First char: 7, Second char: 0
                            KeyCode::Digit8 => self.write_memory_value(0x08), // First char: 8, Second char: 0
                            KeyCode::Digit9 => self.write_memory_value(0x09), // First char: 9, Second char: 0
                            
                            // Valid hex letters A-F (first char only, second char = 0)
                            KeyCode::KeyA => self.write_memory_value(0x0A), // First char: A, Second char: 0
                            KeyCode::KeyB => self.write_memory_value(0x0B), // First char: B, Second char: 0  
                            KeyCode::KeyC => self.write_memory_value(0x0C), // First char: C, Second char: 0
                            KeyCode::KeyD => self.write_memory_value(0x0D), // First char: D, Second char: 0
                            KeyCode::KeyE => self.write_memory_value(0x0E), // First char: E, Second char: 0
                            KeyCode::KeyF => self.write_memory_value(0x0F), // First char: F, Second char: 0
                            
                            // Navigation
                            KeyCode::ArrowLeft => {
                                if self.current_address > 0x7600 {
                                    if self.current_address == 0x7700 {
                                        // Jump from start of second section to end of first section
                                        self.current_address = 0x764F;
                                    } else {
                                        self.current_address -= 1;
                                    }
                                    self.show_memory_interface();
                                }
                            }
                            KeyCode::ArrowRight => {
                                if self.current_address < 0x774F {
                                    if self.current_address == 0x764F {
                                        // Jump from end of first section to start of second section
                                        self.current_address = 0x7700;
                                    } else {
                                        self.current_address += 1;
                                    }
                                    self.show_memory_interface();
                                }
                            }
                            KeyCode::ArrowUp => {
                                // Jump to previous section
                                if self.current_address >= 0x7700 {
                                    // From second section to first section (same column)
                                    let col = self.current_address - 0x7700;
                                    self.current_address = 0x7600 + col;
                                    self.show_memory_interface();
                                }
                            }
                            KeyCode::ArrowDown => {
                                // Jump to next section
                                if self.current_address >= 0x7600 && self.current_address <= 0x764F {
                                    // From first section to second section (same column)
                                    let col = self.current_address - 0x7600;
                                    self.current_address = 0x7700 + col;
                                    self.show_memory_interface();
                                }
                            }
                            KeyCode::Home => {
                                self.current_address = 0x7600;
                                self.show_memory_interface();
                            }
                            KeyCode::End => {
                                self.current_address = 0x774F; // End of second section
                                self.show_memory_interface();
                            }
                            
                            // Invalid characters in writing mode - provide feedback
                            _ => {
                                println!("❌ Invalid hex character! Use only 0-9 and A-F, or H for hex input mode");
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

impl ApplicationHandler for Pc1500MemoryWriter {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_some() {
            return;
        }

        // Create window exactly like pc1500_window.rs
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Ceres - PC-1500 Memory Writer")
                    .with_inner_size(LogicalSize::new(
                        156 * 4, // Scale 4x for visibility  
                        7 * 4,   // CORRECTED: PC-1500 is 156x7, not 156x8
                    ))
                    .with_min_inner_size(LogicalSize::new(156, 7))
                    .with_resizable(true),
            )
            .expect("Failed to create window");

        // Initialize graphics
        let state = pollster::block_on(video::State::<156, 7>::new(
            window,
            ShaderOption::Nearest,
            ScalingOption::PixelPerfect,
        ))
        .expect("Failed to create graphics state");

        self.windows = Some(Windows { main: state });
        
        println!("🎮 PC-1500 Memory Writer Started!");
        println!("📋 Controls:");
        println!("   SPACE - Enter memory writing mode");
        println!("   R     - Return to READY (Test Mode)");
        println!("   L     - Load ROM (Switch to ROM Mode)");
        println!("   I     - Show system information (Test/ROM mode, CPU state)");
        println!("   M     - Show display memory map (PC-1500 format)");
        println!("   H     - Hex input mode (type any 00-FF combination)");
        println!("   T     - Test PC-1500 two hex char system");
        println!("📋 Single Character Input (in writing mode):");
        println!("   Valid hex characters only: 0-9, A-F");
        println!("   Each character writes to first 4 DOTS (bits 0,1,2,3)");
        println!("   Second char (3 DOTS) automatically set to 0");
        println!("📋 Hex Input Mode (H key):");
        println!("   Format: AB where A→4 DOTS (bits 0,1,2,3), B→3 DOTS (bits 4,5,6)");
        println!("   Examples: AB, 3F, 72, FF, etc.");
        println!("   Press ENTER to confirm, ESC to cancel");
        println!("📋 Navigation:");
        println!("   ←→    - Navigate within sections");
        println!("   ↑↓    - Jump between sections (same column)");
        println!("   HOME  - Go to start (0x7600)");
        println!("   END   - Go to end (0x774F)");
        println!("   ESC   - Exit");
        println!("📋 Memory Layout:");
        println!("   First section:  0x7600-0x764F (80 bytes)");
        println!("   Second section: 0x7700-0x774F (80 bytes)");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("PC-1500 Memory Writer closing...");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use winit::event::ElementState;
                match event.state {
                    ElementState::Pressed => {
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                            // If in hex input mode, just cancel it
                            if self.hex_input_mode {
                                self.hex_input_mode = false;
                                self.hex_input_buffer.clear();
                                println!("❌ Hex input cancelled");
                                return;
                            }
                            // Otherwise exit the application
                            event_loop.exit();
                            return;
                        }
                        self.handle_keyboard_input(&event, true);
                    }
                    ElementState::Released => {
                        self.handle_keyboard_input(&event, false);
                    }
                }
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(windows) = &mut self.windows {
                    windows.main.resize(physical_size);
                }
            }
            WindowEvent::RedrawRequested => {
                // Step emulation frame (CRITICAL!)
                self.step_frame();

                // Update display
                if let Err(e) = self.update_display() {
                    eprintln!("Display update error: {e}");
                }

                // Render frame
                if let Some(windows) = &mut self.windows {
                    if let Err(e) = windows.main.render() {
                        eprintln!("Render error: {e}");
                    }
                }

                // Request next frame
                if let Some(windows) = &self.windows {
                    windows.main.window().request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() -> anyhow::Result<()> {
    println!("🎮 PC-1500 Memory Writer");
    println!("========================");
    
    let args: Vec<String> = std::env::args().collect();
    let event_loop = EventLoop::new()?;
    
    // Check if ROM file argument provided
    let mut app = if args.len() > 1 {
        let rom_path = &args[1];
        println!("🔄 Loading ROM: {}", rom_path);
        match Pc1500MemoryWriter::new_with_rom(rom_path) {
            Ok(app) => {
                println!("✅ ROM loaded successfully! Starting in ROM mode...");
                app
            }
            Err(e) => {
                println!("❌ Failed to load ROM: {}", e);
                println!("🔄 Falling back to test mode...");
                Pc1500MemoryWriter::new()
            }
        }
    } else {
        println!("🧪 Starting in test mode. Use 'L' key to load ROM, or pass ROM file as argument:");
        println!("   cargo run --bin pc1500_memory_writer PC-1500_A04.ROM");
        Pc1500MemoryWriter::new()
    };
    
    event_loop.run_app(&mut app)?;
    
    Ok(())
}
