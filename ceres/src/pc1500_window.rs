/// PC-1500 Graphical Application
/// 
/// This creates a window that displays the PC-1500 emulator screen graphically

mod video;

use ceres_core::{AudioCallback, Pc1500, Pc1500Model, Sample};
use ceres_std::{ShaderOption, wgpu_renderer::ScalingOption};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
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

/// Main PC-1500 application
pub struct Pc1500App {
    pc1500: Option<Pc1500<Pc1500AudioCallback>>,
    windows: Option<Windows>,
}

impl Pc1500App {
    pub fn new() -> Self {
        let audio_callback = Pc1500AudioCallback;
        let mut pc1500 = Pc1500::new(Pc1500Model::Pc1500, audio_callback);
        
        // Initialize test mode and show welcome message
        pc1500.init_test_mode();
        pc1500.display_message("READY");
        
        Self {
            pc1500: Some(pc1500),
            windows: None,
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
    
    fn handle_keyboard_input(&mut self, key_event: &KeyEvent, pressed: bool) {
        if let Some(pc1500) = &mut self.pc1500 {
            if let PhysicalKey::Code(key_code) = key_event.physical_key {
                if pressed { // Only handle key press, not release for these commands
                    match key_code {
                        // Test messages
                        KeyCode::F1 => {
                            pc1500.display_message("HELLO PC-1500");
                            println!("Displayed: HELLO PC-1500");
                        }
                        KeyCode::F2 => {
                            pc1500.clear_display();
                            pc1500.display_text_centered(0, "CERES");
                            pc1500.display_text_centered(1, "EMULATOR");
                            println!("Displayed: CERES EMULATOR");
                        }
                        KeyCode::F3 => {
                            pc1500.show_test_pattern();
                            println!("Displayed: PC-1500 test pattern");
                        }
                        KeyCode::F4 => {
                            pc1500.clear_display();
                            pc1500.display_text_at(0, 0, "TEST");
                            pc1500.display_text_at(0, 1, "1234567890");
                            println!("Displayed: TEST text and numbers");
                        }
                        KeyCode::F5 => {
                            pc1500.clear_display();
                            pc1500.display_text_at(0, 0, "ABCDEFGHIJKLM");
                            pc1500.display_text_at(0, 1, "NOPQRSTUVWXYZ");
                            println!("Displayed: Alphabet test");
                        }
                        KeyCode::KeyC => {
                            pc1500.clear_display();
                            println!("Display cleared");
                        }
                        
                        // FUNCIONES DE ESCRITURA DIRECTA EN MEMORIA DEL DISPLAY
                        KeyCode::F6 => {
                            // Escribir patrón sólido (todas las columnas encendidas)
                            println!("✏️ Writing solid pattern to display memory...");
                            for col in 0..20 { // Primeras 20 columnas (primera sección)
                                pc1500.write_memory(0x7600 + col, 0xFF);
                            }
                            println!("✅ Written solid pattern (0xFF) to columns 0-19 (0x7600-0x7613)");
                        }
                        KeyCode::F7 => {
                            // Escribir patrón de tablero de ajedrez
                            println!("✏️ Writing checkerboard pattern...");
                            for col in 0..40 {
                                let pattern = if col % 2 == 0 { 0xAA } else { 0x55 };
                                pc1500.write_memory(0x7600 + col, pattern);
                            }
                            println!("✅ Written checkerboard pattern to columns 0-39 (0x7600-0x7627)");
                        }
                        KeyCode::F8 => {
                            // Escribir líneas horizontales
                            println!("✏️ Writing horizontal lines...");
                            for col in 0..60 {
                                pc1500.write_memory(0x7600 + col, 0x18); // Bits 3 y 4 = líneas centrales
                            }
                            println!("✅ Written horizontal lines to columns 0-59 (0x7600-0x763B)");
                        }
                        KeyCode::F9 => {
                            // Dump de memoria más compacto y útil
                            println!("\n📋 === DISPLAY MEMORY DUMP (PC-1500 addresses) ===");
                            for row in 0..4 { // Mostrar en filas de 10 columnas
                                let start = row * 10;
                                let end = (start + 10).min(40);
                                print!("Row {}: ", row);
                                for col in start..end {
                                    let value = pc1500.read_memory(0x7600 + col);
                                    print!("{:02X} ", value);
                                }
                                println!();
                            }
                            println!("=============================\n");
                        }
                        KeyCode::F10 => {
                            // Limpiar memoria de display completamente
                            println!("🧹 Clearing entire display memory...");
                            // First section (0x7600-0x764F)
                            for col in 0..80 {
                                pc1500.write_memory(0x7600 + col, 0x00);
                            }
                            // Second section (0x7700-0x774F)
                            for col in 0..80 {
                                pc1500.write_memory(0x7700 + col, 0x00);
                            }
                            println!("✅ Display memory cleared (0x7600-0x764F and 0x7700-0x774F)");
                        }
                        KeyCode::F11 => {
                            // Escribir texto "TEST" directamente en memoria
                            println!("✏️ Writing TEST pattern directly to memory...");
                            // Patrón simple para "TEST" (simplificado)
                            let test_pattern = [0x7E, 0x18, 0x18, 0x18, 0x00, // T
                                              0x7E, 0x60, 0x7C, 0x60, 0x7E, // E  
                                              0x3E, 0x60, 0x3C, 0x06, 0x7C, // S
                                              0x7E, 0x18, 0x18, 0x18, 0x00]; // T
                            for (i, &pattern) in test_pattern.iter().enumerate() {
                                pc1500.write_memory(0x7600 + i as u16, pattern);
                            }
                            println!("✅ Written TEST pattern to memory (0x7600-0x7613)");
                        }
                        KeyCode::F12 => {
                            // Escribir patrón de barras verticales
                            println!("✏️ Writing vertical bars...");
                            for col in 0..80 {
                                let pattern = if col % 4 < 2 { 0xFF } else { 0x00 };
                                pc1500.write_memory(0x7600 + col, pattern);
                            }
                            println!("✅ Written vertical bars pattern (0x7600-0x764F)");
                        }
                        
                        // Numbers 0-9 (original PC-1500 key mapping)
                        KeyCode::Digit0 => pc1500.handle_key_input(0x02, pressed),
                        KeyCode::Digit1 => pc1500.handle_key_input(0x03, pressed),
                        KeyCode::Digit2 => pc1500.handle_key_input(0x04, pressed),
                        KeyCode::Digit3 => pc1500.handle_key_input(0x05, pressed),
                        KeyCode::Digit4 => pc1500.handle_key_input(0x06, pressed),
                        KeyCode::Digit5 => pc1500.handle_key_input(0x07, pressed),
                        KeyCode::Digit6 => pc1500.handle_key_input(0x08, pressed),
                        KeyCode::Digit7 => pc1500.handle_key_input(0x09, pressed),
                        KeyCode::Digit8 => pc1500.handle_key_input(0x0A, pressed),
                        KeyCode::Digit9 => pc1500.handle_key_input(0x0B, pressed),
                        
                        // Letters (simplified mapping)
                        KeyCode::KeyA => pc1500.handle_key_input(0x1E, pressed),
                        KeyCode::KeyB => pc1500.handle_key_input(0x30, pressed),
                        
                        // Special keys
                        KeyCode::Enter => pc1500.handle_key_input(0x1C, pressed),
                        KeyCode::Space => pc1500.handle_key_input(0x39, pressed),
                        KeyCode::Escape => pc1500.handle_key_input(0x01, pressed),
                        
                        _ => {} // Ignore other keys
                    }
                } else {
                    // Handle key release for original PC-1500 keys
                    match key_code {
                        KeyCode::Digit0 => pc1500.handle_key_input(0x02, pressed),
                        KeyCode::Digit1 => pc1500.handle_key_input(0x03, pressed),
                        KeyCode::Digit2 => pc1500.handle_key_input(0x04, pressed),
                        KeyCode::Digit3 => pc1500.handle_key_input(0x05, pressed),
                        KeyCode::Digit4 => pc1500.handle_key_input(0x06, pressed),
                        KeyCode::Digit5 => pc1500.handle_key_input(0x07, pressed),
                        KeyCode::Digit6 => pc1500.handle_key_input(0x08, pressed),
                        KeyCode::Digit7 => pc1500.handle_key_input(0x09, pressed),
                        KeyCode::Digit8 => pc1500.handle_key_input(0x0A, pressed),
                        KeyCode::Digit9 => pc1500.handle_key_input(0x0B, pressed),
                        KeyCode::KeyA => pc1500.handle_key_input(0x1E, pressed),
                        KeyCode::KeyB => pc1500.handle_key_input(0x30, pressed),
                        KeyCode::Enter => pc1500.handle_key_input(0x1C, pressed),
                        KeyCode::Space => pc1500.handle_key_input(0x39, pressed),
                        KeyCode::Escape => pc1500.handle_key_input(0x01, pressed),
                        _ => {}
                    }
                }
            }
        }
    }
}

impl ApplicationHandler for Pc1500App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_some() {
            return;
        }

        // Create window with PC-1500 display dimensions (scaled up for visibility)
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Ceres - PC-1500 Emulator")
                    .with_inner_size(LogicalSize::new(
                        156 * 4, // Scale 4x for visibility  
                        7 * 4,   // CORRECTED: PC-1500 is 156x7
                    ))
                    .with_min_inner_size(LogicalSize::new(156, 7)) // CORRECTED
                    .with_resizable(true),
            )
            .expect("Failed to create window");

        // Initialize graphics
        let state = pollster::block_on(video::State::<156, 7>::new( // CORRECTED
            window,
            ShaderOption::Nearest,
            ScalingOption::PixelPerfect,
        ))
        .expect("Failed to create graphics state");

        self.windows = Some(Windows { main: state });
        
        println!("PC-1500 Window Created! Use keyboard to interact with the emulator.");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("PC-1500 emulator closing...");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use winit::event::ElementState;
                match event.state {
                    ElementState::Pressed => {
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
    println!("🖥️  Launching PC-1500 Emulator Window...");
    println!("📋 Text Display Controls:");
    println!("   F1  - Display 'HELLO PC-1500'");
    println!("   F2  - Display 'CERES EMULATOR'");
    println!("   F3  - Show test pattern");
    println!("   F4  - Display 'TEST' and numbers");
    println!("   F5  - Display alphabet test");
    println!("   C   - Clear display");
    println!("📋 Direct Memory Writing Controls:");
    println!("   F6  - Write solid pattern");
    println!("   F7  - Write checkerboard pattern");
    println!("   F8  - Write horizontal lines");
    println!("   F9  - Dump memory content");
    println!("   F10 - Clear memory directly");
    println!("   F11 - Write TEST pattern");
    println!("   F12 - Write vertical bars");
    println!("   ESC - Exit");
    
    let event_loop = EventLoop::new()?;
    let mut app = Pc1500App::new();
    
    event_loop.run_app(&mut app)?;
    
    Ok(())
}
