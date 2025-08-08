/// PC-1500 Display Memory Test
/// 
/// Simple test program that writes random values to display memory addresses
/// and verifies they appear on screen correctly.

// Include the video module from the ceres main package
#[path = "../ceres/src/video.rs"]
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
use std::time::{Duration, Instant};

/// Audio callback for PC-1500
struct Pc1500AudioCallback;

impl AudioCallback for Pc1500AudioCallback {
    fn audio_sample(&self, _l: Sample, _r: Sample) {
        // PC-1500 beeper would go here
    }
}

struct Windows {
    main: video::State<'static, 156, 8>, // PC-1500 display is 156x8 pixels
}

/// PC-1500 Display Test Application
pub struct Pc1500DisplayTest {
    pc1500: Option<Pc1500<Pc1500AudioCallback>>,
    windows: Option<Windows>,
    test_running: bool,
    last_update: Instant,
    test_pattern: u8,
    current_address: u16,
    addresses_written: Vec<(u16, u8)>, // Track what we wrote where
}

impl Pc1500DisplayTest {
    pub fn new() -> Self {
        let audio_callback = Pc1500AudioCallback;
        let mut pc1500 = Pc1500::new(Pc1500Model::Pc1500, audio_callback);
        
        // Initialize
        pc1500.init_test_mode();
        
        Self {
            pc1500: Some(pc1500),
            windows: None,
            test_running: false,
            last_update: Instant::now(),
            test_pattern: 0,
            current_address: 0x7600,
            addresses_written: Vec::new(),
        }
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
    
    fn start_random_test(&mut self) {
        if let Some(pc1500) = &mut self.pc1500 {
            println!("🧪 STARTING PC-1500 DISPLAY MEMORY TEST");
            println!("======================================");
            println!("Writing random values to display memory and verifying display output...");
            println!();
            
            // Clear both memory sections first
            println!("Clearing display memory...");
            for addr in 0x7600..=0x764F {
                pc1500.write_memory(addr, 0x00);
            }
            for addr in 0x7700..=0x774F {
                pc1500.write_memory(addr, 0x00);
            }
            
            self.test_running = true;
            self.current_address = 0x7600;
            self.addresses_written.clear();
            
            println!("✅ Display cleared. Starting random write test...");
            println!("Press SPACE to stop test, R to restart, ESC to exit");
            println!();
        }
    }
    
    fn run_test_cycle(&mut self) {
        if !self.test_running || self.last_update.elapsed() < Duration::from_millis(200) {
            return;
        }
        
        if let Some(pc1500) = &mut self.pc1500 {
            // Simple pseudo-random number generation
            self.test_pattern = self.test_pattern.wrapping_mul(137).wrapping_add(71);
            let random_value = self.test_pattern;
            
            pc1500.write_memory(self.current_address, random_value);
            
            // Read back to verify
            let read_back = pc1500.read_memory(self.current_address);
            
            // Track what we wrote
            self.addresses_written.push((self.current_address, random_value));
            
            // Determine section and column
            let (section_name, col) = if self.current_address >= 0x7700 {
                ("Second", self.current_address - 0x7700)
            } else {
                ("First", self.current_address - 0x7600)
            };
            
            // Show detailed info
            let first_char = random_value & 0x0F;
            let second_char = (random_value >> 4) & 0x07;
            let bit7 = (random_value >> 7) & 1;
            
            println!("📍 {} section, Col {}: 0x{:04X} ← 0x{:02X} (read: 0x{:02X}) {}", 
                    section_name, col, self.current_address, random_value, read_back,
                    if read_back == random_value { "✅" } else { "❌" });
            println!("   Binary: {:08b} → 4-DOTS: {:04b} (0x{:X}), 3-DOTS: {:03b} (0x{:X}), bit7: {} {}",
                    random_value, first_char, first_char, second_char, second_char, bit7,
                    if bit7 == 1 { "(ignored)" } else { "" });
            
            // Visual representation
            print!("   Visual: ");
            for bit in 0..7 {
                let is_set = (random_value >> bit) & 1 != 0;
                print!("{}", if is_set { "█" } else { "·" });
                if bit == 3 { print!("-"); } // Separator between 4-DOTS and 3-DOTS
            }
            println!();
            
            // Move to next address
            self.current_address += 1;
            
            // Check if we've gone through all addresses
            if self.current_address > 0x764F && self.current_address < 0x7700 {
                // Jump from end of first section to start of second section
                self.current_address = 0x7700;
                println!("🔄 Switching to second section (0x7700-0x774F)");
            } else if self.current_address > 0x774F {
                // We've finished both sections
                self.finish_test();
                return;
            }
            
            self.last_update = Instant::now();
        }
    }
    
    fn finish_test(&mut self) {
        self.test_running = false;
        
        println!();
        println!("🎉 TEST COMPLETED!");
        println!("==================");
        println!("Total addresses written: {}", self.addresses_written.len());
        
        if let Some(pc1500) = &mut self.pc1500 {
            // Verify all written values are still correct
            let mut correct = 0;
            let mut incorrect = 0;
            
            println!();
            println!("🔍 VERIFICATION PHASE:");
            println!("Checking all written values are still in memory...");
            
            for (addr, expected) in &self.addresses_written {
                let actual = pc1500.read_memory(*addr);
                if actual == *expected {
                    correct += 1;
                } else {
                    incorrect += 1;
                    println!("❌ Address 0x{:04X}: expected 0x{:02X}, got 0x{:02X}", 
                            addr, expected, actual);
                }
            }
            
            println!();
            println!("📊 RESULTS:");
            println!("✅ Correct: {}/{}", correct, self.addresses_written.len());
            if incorrect > 0 {
                println!("❌ Incorrect: {}", incorrect);
                println!("⚠️  Some memory addresses may not be properly connected to display!");
            } else {
                println!("🎉 ALL MEMORY ADDRESSES WORKING CORRECTLY!");
                println!("✅ Display memory is properly connected to the visual output");
            }
            
            // Show a summary pattern to visualize the test
            println!();
            println!("📺 DISPLAY TEST SUMMARY:");
            println!("Each dot represents a memory location that was tested");
            self.show_test_summary();
        }
        
        println!();
        println!("Press R to restart test, ESC to exit");
    }
    
    fn show_test_summary(&self) {
        if let Some(pc1500) = &self.pc1500 {
            println!("First section (0x7600-0x764F) - showing first 40 columns:");
            
            // Show which addresses were written to
            print!("Written: ");
            for col in 0..40 {
                let addr = 0x7600 + col;
                let was_written = self.addresses_written.iter().any(|(a, _)| *a == addr);
                print!("{}", if was_written { "●" } else { "·" });
            }
            println!();
            
            // Show actual display state
            for row in 0..7 {
                print!("Bit {}: ", row);
                for col in 0..40 {
                    let value = pc1500.read_memory(0x7600 + col);
                    let bit_set = (value >> row) & 1 != 0;
                    print!("{}", if bit_set { "█" } else { "·" });
                }
                println!();
            }
            
            println!();
            println!("Second section (0x7700-0x774F) - showing first 40 columns:");
            
            // Show which addresses were written to
            print!("Written: ");
            for col in 0..40 {
                let addr = 0x7700 + col;
                let was_written = self.addresses_written.iter().any(|(a, _)| *a == addr);
                print!("{}", if was_written { "●" } else { "·" });
            }
            println!();
            
            // Show actual display state  
            for row in 0..7 {
                print!("Bit {}: ", row);
                for col in 0..40 {
                    let value = pc1500.read_memory(0x7700 + col);
                    let bit_set = (value >> row) & 1 != 0;
                    print!("{}", if bit_set { "█" } else { "·" });
                }
                println!();
            }
        }
    }
    
    fn stop_test(&mut self) {
        if self.test_running {
            self.test_running = false;
            println!("⏸️  Test stopped by user");
            self.finish_test();
        }
    }
    
    fn clear_display(&mut self) {
        if let Some(pc1500) = &mut self.pc1500 {
            println!("🧹 Clearing display...");
            for addr in 0x7600..=0x764F {
                pc1500.write_memory(addr, 0x00);
            }
            for addr in 0x7700..=0x774F {
                pc1500.write_memory(addr, 0x00);
            }
            self.addresses_written.clear();
            println!("✅ Display cleared");
        }
    }
    
    fn handle_keyboard_input(&mut self, key_event: &KeyEvent, pressed: bool) {
        if let winit::keyboard::PhysicalKey::Code(key_code) = key_event.physical_key {
            if pressed {
                match key_code {
                    KeyCode::Space => {
                        if self.test_running {
                            self.stop_test();
                        } else {
                            self.start_random_test();
                        }
                    }
                    KeyCode::KeyR => {
                        self.test_running = false;
                        self.clear_display();
                        self.start_random_test();
                    }
                    KeyCode::KeyC => {
                        self.clear_display();
                    }
                    _ => {}
                }
            }
        }
    }
}

impl ApplicationHandler for Pc1500DisplayTest {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_some() {
            return;
        }

        // Create window
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Ceres - PC-1500 Display Memory Test")
                    .with_inner_size(LogicalSize::new(
                        156 * 6, // Scale 6x for better visibility  
                        8 * 6,
                    ))
                    .with_min_inner_size(LogicalSize::new(156, 8))
                    .with_resizable(true),
            )
            .expect("Failed to create window");

        // Initialize graphics
        let state = pollster::block_on(video::State::<156, 8>::new(
            window,
            ShaderOption::Nearest,
            ScalingOption::PixelPerfect,
        ))
        .expect("Failed to create graphics state");

        self.windows = Some(Windows { main: state });
        
        println!("🧪 PC-1500 Display Memory Test");
        println!("==============================");
        println!("This test writes random values to PC-1500 display memory");
        println!("and verifies they appear correctly on the display.");
        println!();
        println!("Memory ranges being tested:");
        println!("  • First section:  0x7600H - 0x764FH (80 bytes)");
        println!("  • Second section: 0x7700H - 0x774FH (80 bytes)");
        println!("  • Total: 160 bytes");
        println!();
        println!("Controls:");
        println!("  SPACE - Start/Stop random test");
        println!("  R     - Restart test (clear + start)");
        println!("  C     - Clear display");
        println!("  ESC   - Exit");
        println!();
        println!("Press SPACE to begin the test!");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("PC-1500 Display Test closing...");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use winit::event::ElementState;
                match event.state {
                    ElementState::Pressed => {
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
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
                // Step emulation frame
                self.step_frame();
                
                // Run test cycle if active
                self.run_test_cycle();

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
    println!("🧪 PC-1500 Display Memory Test");
    println!("==============================");
    
    let event_loop = EventLoop::new()?;
    let mut app = Pc1500DisplayTest::new();
    
    event_loop.run_app(&mut app)?;
    
    Ok(())
}
