/// PC-1500 Complete EGUI Application - FULL RESTORATION
/// 
/// This is the COMPLETE PC-1500 emulator with ALL functionality restored:
/// - Full PC-1500 calculator keyboard layout (exact authentic layout)
/// - ROM execution with real-time display updates
/// - Complete memory debugging and writing tools  
/// - Display system with 156x7 pixel resolution
/// - CPU state monitoring and ROM mode indication
/// - All debugging tools and interfaces
/// 
/// NEVER SIMPLIFY OR REMOVE FUNCTIONALITY FROM THIS FILE

use eframe::egui;
use ceres_core::{Pc1500, AudioCallback, Model};
use ceres_core::pc1500::joypad::Key as Pc1500Key;
use std::collections::HashSet;

// Audio callback for PC-1500
struct Pc1500AudioCallback;
impl AudioCallback for Pc1500AudioCallback {
    fn audio_sample(&self, _left: i16, _right: i16) {}
}

pub struct Pc1500App {
    // CORE EMULATOR - The real PC-1500 system
    emulator: Pc1500<Pc1500AudioCallback>,
    
    // KEYBOARD STATE - Full PC-1500 keyboard with timing
    pressed_keys: HashSet<Pc1500Key>,
    key_press_timers: std::collections::HashMap<Pc1500Key, std::time::Instant>,
    keyboard_enabled: bool,
    
    // UI STATE - Complete interface
    show_keyboard: bool,
    show_memory_editor: bool,
    show_display_debug: bool,
    show_cpu_info: bool,
    show_rom_controls: bool,
    debug_mode: bool,
    
    // MEMORY TOOLS - Complete memory editing
    current_memory_address: u16,
    memory_editor_address: String,
    memory_editor_value: String,
    hex_input_mode: bool,
    hex_input_buffer: String,
    writing_mode: bool,
    show_memory_map: bool,
    
    // ROM MODE - Full ROM support
    rom_mode_active: bool,
    rom_execution_info: String,
    cpu_cycles: u64,
    
    // DISPLAY - Full display system
    display_buffer: Vec<u8>,
    display_width: usize,
    display_height: usize,
    display_scale: f32,
    
    // DEBUG INFO
    frame_count: u64,
    key_press_log: Vec<String>,
    
    // PHYSICAL KEYBOARD MAPPING - Map PC keyboard to PC-1500 keys
    pc_to_pc1500_mapping: std::collections::HashMap<egui::Key, Pc1500Key>,
}

impl Pc1500App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let audio = Pc1500AudioCallback;
        let mut emulator = Pc1500::new(Model::default());
        
        // Initialize in test mode by default
        emulator.init_test_mode();
        emulator.display_message("PC-1500 READY");
        
        Self {
            emulator,
            pressed_keys: HashSet::new(),
            key_press_timers: std::collections::HashMap::new(),
            keyboard_enabled: true,
            show_keyboard: true,
            show_memory_editor: false,
            show_display_debug: false,
            show_cpu_info: false,
            show_rom_controls: true,
            debug_mode: false,
            current_memory_address: 0x7600,
            memory_editor_address: String::from("7600"),
            memory_editor_value: String::from("00"),
            hex_input_mode: false,
            hex_input_buffer: String::new(),
            writing_mode: false,
            show_memory_map: false,
            rom_mode_active: false,
            rom_execution_info: String::from("Test Mode Active"),
            cpu_cycles: 0,
            display_buffer: vec![0; 156 * 7 * 4], // RGBA buffer
            display_width: 156,
            display_height: 7,
            display_scale: 8.0,
            frame_count: 0,
            key_press_log: Vec::new(),
            pc_to_pc1500_mapping: Self::create_keyboard_mapping(),
        }
    }
    
    // Create keyboard mapping from PC keyboard to PC-1500 keys
    fn create_keyboard_mapping() -> std::collections::HashMap<egui::Key, Pc1500Key> {
        let mut mapping = std::collections::HashMap::new();
        
        // Numbers
        mapping.insert(egui::Key::Num0, Pc1500Key::Zero);
        mapping.insert(egui::Key::Num1, Pc1500Key::One);
        mapping.insert(egui::Key::Num2, Pc1500Key::Two);
        mapping.insert(egui::Key::Num3, Pc1500Key::Three);
        mapping.insert(egui::Key::Num4, Pc1500Key::Four);
        mapping.insert(egui::Key::Num5, Pc1500Key::Five);
        mapping.insert(egui::Key::Num6, Pc1500Key::Six);
        mapping.insert(egui::Key::Num7, Pc1500Key::Seven);
        mapping.insert(egui::Key::Num8, Pc1500Key::Eight);
        mapping.insert(egui::Key::Num9, Pc1500Key::Nine);
        
        // Letters (QWERTY to PC-1500)
        mapping.insert(egui::Key::A, Pc1500Key::A);
        mapping.insert(egui::Key::B, Pc1500Key::B);
        mapping.insert(egui::Key::C, Pc1500Key::C);
        mapping.insert(egui::Key::D, Pc1500Key::D);
        mapping.insert(egui::Key::E, Pc1500Key::E);
        mapping.insert(egui::Key::F, Pc1500Key::F);
        mapping.insert(egui::Key::G, Pc1500Key::G);
        mapping.insert(egui::Key::H, Pc1500Key::H);
        mapping.insert(egui::Key::I, Pc1500Key::I);
        mapping.insert(egui::Key::J, Pc1500Key::J);
        mapping.insert(egui::Key::K, Pc1500Key::K);
        mapping.insert(egui::Key::L, Pc1500Key::L);
        mapping.insert(egui::Key::M, Pc1500Key::M);
        mapping.insert(egui::Key::N, Pc1500Key::N);
        mapping.insert(egui::Key::O, Pc1500Key::O);
        mapping.insert(egui::Key::P, Pc1500Key::P);
        mapping.insert(egui::Key::Q, Pc1500Key::Q);
        mapping.insert(egui::Key::R, Pc1500Key::R);
        mapping.insert(egui::Key::S, Pc1500Key::S);
        mapping.insert(egui::Key::T, Pc1500Key::T);
        mapping.insert(egui::Key::U, Pc1500Key::U);
        mapping.insert(egui::Key::V, Pc1500Key::V);
        mapping.insert(egui::Key::W, Pc1500Key::W);
        mapping.insert(egui::Key::X, Pc1500Key::X);
        mapping.insert(egui::Key::Y, Pc1500Key::Y);
        mapping.insert(egui::Key::Z, Pc1500Key::Z);
        
        // Function keys
        mapping.insert(egui::Key::F1, Pc1500Key::F1);
        mapping.insert(egui::Key::F2, Pc1500Key::F2);
        mapping.insert(egui::Key::F3, Pc1500Key::F3);
        mapping.insert(egui::Key::F4, Pc1500Key::F4);
        mapping.insert(egui::Key::F5, Pc1500Key::F5);
        mapping.insert(egui::Key::F6, Pc1500Key::F6);
        
        // Special keys
        mapping.insert(egui::Key::Space, Pc1500Key::Space);
        mapping.insert(egui::Key::Enter, Pc1500Key::Enter);
        mapping.insert(egui::Key::ArrowUp, Pc1500Key::Up);
        mapping.insert(egui::Key::ArrowDown, Pc1500Key::Down);
        mapping.insert(egui::Key::ArrowLeft, Pc1500Key::Left);
        mapping.insert(egui::Key::ArrowRight, Pc1500Key::Right);
        
        // Operators - using basic key mappings
        // Plus key will be handled with text input or as Shift+Equals
        mapping.insert(egui::Key::Minus, Pc1500Key::Minus);  
        mapping.insert(egui::Key::Slash, Pc1500Key::Slash);
        mapping.insert(egui::Key::Equals, Pc1500Key::Equals);
        mapping.insert(egui::Key::Period, Pc1500Key::Dot);
        
        // Special PC-1500 keys mapped to PC keys
        mapping.insert(egui::Key::Backspace, Pc1500Key::Cl);
        mapping.insert(egui::Key::Tab, Pc1500Key::Mode);
        mapping.insert(egui::Key::Escape, Pc1500Key::Off);
        mapping.insert(egui::Key::End, Pc1500Key::On);
        
        mapping
    }
    
    fn update_emulator(&mut self) {
        // Step the emulator
        self.emulator.step_frame();
        self.frame_count += 1;
        
        // Update ROM mode status
        self.rom_mode_active = self.emulator.is_rom_mode();
        if self.rom_mode_active {
            let cpu_state = self.emulator.cpu_state();
            self.rom_execution_info = format!(
                "ROM EXECUTING - PC: 0x{:04X} | A: 0x{:02X} | Cycles: {}",
                cpu_state.pc, cpu_state.a, self.cpu_cycles
            );
        } else {
            self.rom_execution_info = "Test Mode Active - Press 'Load ROM' to run authentic firmware".to_string();
        }
        
        // Update display buffer
        let pixel_data = self.emulator.display().rgba_buffer();
        self.display_buffer.copy_from_slice(pixel_data);
    }
    
    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Load ROM...").clicked() {
                    self.load_rom_dialog();
                }
                ui.separator();
                if ui.button("Reset System").clicked() {
                    self.reset_system();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });
            
            ui.menu_button("System", |ui| {
                if ui.button("Toggle Test/ROM Mode").clicked() {
                    if self.rom_mode_active {
                        self.emulator.init_test_mode();
                        self.emulator.display_message("TEST MODE");
                    } else {
                        // Try to load ROM
                        self.try_load_default_rom();
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.rom_mode_active, "ROM Mode Active");
            });
            
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.show_keyboard, "PC-1500 Keyboard");
                ui.checkbox(&mut self.show_memory_editor, "Memory Editor");
                ui.checkbox(&mut self.show_display_debug, "Display Debug");
                ui.checkbox(&mut self.show_cpu_info, "CPU Information");
                ui.checkbox(&mut self.show_rom_controls, "ROM Controls");
                ui.separator();
                ui.checkbox(&mut self.debug_mode, "Debug Mode");
            });
            
            ui.menu_button("Tools", |ui| {
                if ui.button("Clear Display").clicked() {
                    self.clear_display();
                }
                if ui.button("Test Pattern").clicked() {
                    self.show_test_pattern();
                }
                if ui.button("Memory Map").clicked() {
                    self.show_memory_map = !self.show_memory_map;
                }
                ui.separator();
                if ui.button("Test PC-1500 Display System").clicked() {
                    self.test_pc1500_display_system();
                }
                if ui.button("Show System Information").clicked() {
                    self.show_system_info();
                }
            });
        });
    }
    
    fn render_main_display(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label("PC-1500 Display (156×7 pixels)");
            
            // ROM mode indicator
            if self.rom_mode_active {
                ui.colored_label(egui::Color32::GREEN, "🟢 ROM MODE - Authentic PC-1500 firmware executing");
            } else {
                ui.colored_label(egui::Color32::YELLOW, "🟡 TEST MODE - Manual control available");
            }
            
            ui.label(&self.rom_execution_info);
            
            // Display the actual screen
            let display_size = egui::Vec2::new(
                self.display_width as f32 * self.display_scale,
                self.display_height as f32 * self.display_scale,
            );
            
            let (rect, _response) = ui.allocate_exact_size(display_size, egui::Sense::hover());
            
            // Create texture from display buffer
            let texture = ui.ctx().load_texture(
                "pc1500_display",
                egui::ColorImage::from_rgba_unmultiplied(
                    [self.display_width, self.display_height],
                    &self.display_buffer,
                ),
                egui::TextureOptions::NEAREST,
            );
            
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        });
    }
    
    fn render_pc1500_keyboard(&mut self, ui: &mut egui::Ui) {
        if !self.show_keyboard {
            return;
        }
        
        ui.group(|ui| {
            ui.label("PC-1500 Virtual Keyboard - AUTHENTIC LAYOUT");
            ui.label("Click keys to send to emulator | Yellow = pressed");
            
            let mut clicked_keys = Vec::new();
            
            // Helper function to create keyboard button
            let mut create_key = |ui: &mut egui::Ui, key: Pc1500Key, label: &str, width: f32| {
                let is_pressed = self.pressed_keys.contains(&key);
                let button_color = if is_pressed {
                    egui::Color32::YELLOW
                } else {
                    ui.style().visuals.widgets.inactive.bg_fill
                };
                
                let button = egui::Button::new(label)
                    .fill(button_color)
                    .min_size(egui::Vec2::new(width, 25.0));
                
                if ui.add(button).clicked() {
                    clicked_keys.push(key);
                }
            };
            
            // Row 1: Function keys and system keys
            ui.horizontal(|ui| {
                create_key(ui, Pc1500Key::Def, "DEF", 35.0);
                create_key(ui, Pc1500Key::F1, "F1", 30.0);
                create_key(ui, Pc1500Key::F2, "F2", 30.0);
                create_key(ui, Pc1500Key::F3, "F3", 30.0);
                create_key(ui, Pc1500Key::F4, "F4", 30.0);
                create_key(ui, Pc1500Key::F5, "F5", 30.0);
                create_key(ui, Pc1500Key::F6, "F6", 30.0);
                create_key(ui, Pc1500Key::Shift, "SHIFT", 45.0);
                create_key(ui, Pc1500Key::Off, "OFF", 35.0);
                create_key(ui, Pc1500Key::On, "ON", 30.0);
            });
            
            ui.add_space(3.0);
            
            // Row 2: QWERTY + numbers 7,8,9,/,CL
            ui.horizontal(|ui| {
                create_key(ui, Pc1500Key::Q, "Q", 25.0);
                create_key(ui, Pc1500Key::W, "W", 25.0);
                create_key(ui, Pc1500Key::E, "E", 25.0);
                create_key(ui, Pc1500Key::R, "R", 25.0);
                create_key(ui, Pc1500Key::T, "T", 25.0);
                create_key(ui, Pc1500Key::Y, "Y", 25.0);
                create_key(ui, Pc1500Key::U, "U", 25.0);
                create_key(ui, Pc1500Key::I, "I", 25.0);
                create_key(ui, Pc1500Key::O, "O", 25.0);
                create_key(ui, Pc1500Key::P, "P", 25.0);
                ui.add_space(10.0);
                create_key(ui, Pc1500Key::Seven, "7", 25.0);
                create_key(ui, Pc1500Key::Eight, "8", 25.0);
                create_key(ui, Pc1500Key::Nine, "9", 25.0);
                create_key(ui, Pc1500Key::Slash, "/", 25.0);
                create_key(ui, Pc1500Key::Cl, "CL", 30.0);
            });
            
            ui.add_space(3.0);
            
            // Row 3: ASDF + numbers 4,5,6,*,MODE  
            ui.horizontal(|ui| {
                create_key(ui, Pc1500Key::A, "A", 25.0);
                create_key(ui, Pc1500Key::S, "S", 25.0);
                create_key(ui, Pc1500Key::D, "D", 25.0);
                create_key(ui, Pc1500Key::F, "F", 25.0);
                create_key(ui, Pc1500Key::G, "G", 25.0);
                create_key(ui, Pc1500Key::H, "H", 25.0);
                create_key(ui, Pc1500Key::J, "J", 25.0);
                create_key(ui, Pc1500Key::K, "K", 25.0);
                create_key(ui, Pc1500Key::L, "L", 25.0);
                ui.add_space(10.0);
                create_key(ui, Pc1500Key::Four, "4", 25.0);
                create_key(ui, Pc1500Key::Five, "5", 25.0);
                create_key(ui, Pc1500Key::Six, "6", 25.0);
                create_key(ui, Pc1500Key::Asterisk, "*", 25.0);
                create_key(ui, Pc1500Key::Mode, "MODE", 40.0);
            });
            
            ui.add_space(3.0);
            
            // Row 4: ZXCV + numbers 1,2,3,-,←
            ui.horizontal(|ui| {
                create_key(ui, Pc1500Key::Z, "Z", 25.0);
                create_key(ui, Pc1500Key::X, "X", 25.0);
                create_key(ui, Pc1500Key::C, "C", 25.0);
                create_key(ui, Pc1500Key::V, "V", 25.0);
                create_key(ui, Pc1500Key::B, "B", 25.0);
                create_key(ui, Pc1500Key::N, "N", 25.0);
                create_key(ui, Pc1500Key::M, "M", 25.0);
                ui.add_space(10.0);
                create_key(ui, Pc1500Key::One, "1", 25.0);
                create_key(ui, Pc1500Key::Two, "2", 25.0);
                create_key(ui, Pc1500Key::Three, "3", 25.0);
                create_key(ui, Pc1500Key::Minus, "-", 25.0);
                create_key(ui, Pc1500Key::Left, "◄", 25.0);
            });
            
            ui.add_space(3.0);
            
            // Row 5: Special keys, SPACE, 0, ., =, +, →
            ui.horizontal(|ui| {
                create_key(ui, Pc1500Key::Sml, "SML", 35.0);
                create_key(ui, Pc1500Key::Up, "▲", 25.0);
                create_key(ui, Pc1500Key::Down, "▼", 25.0);
                create_key(ui, Pc1500Key::Rcl, "RCL", 35.0);
                ui.add_space(10.0);
                create_key(ui, Pc1500Key::Space, "SPACE", 80.0);
                ui.add_space(10.0);
                create_key(ui, Pc1500Key::Zero, "0", 25.0);
                create_key(ui, Pc1500Key::Dot, ".", 25.0);
                create_key(ui, Pc1500Key::Equals, "=", 25.0);
                create_key(ui, Pc1500Key::Plus, "+", 25.0);
                create_key(ui, Pc1500Key::Right, "►", 25.0);
            });
            
            ui.add_space(3.0);
            
            // Row 6: ENTER (large key)
            ui.horizontal(|ui| {
                ui.add_space(200.0);
                create_key(ui, Pc1500Key::Enter, "ENTER", 100.0);
            });
            
            // Process clicked keys from virtual keyboard
            for key in clicked_keys {
                self.send_key_press(key);
                // For virtual keyboard clicks, also log as virtual click
                self.key_press_log.push(format!("Key clicked: {:?} (virtual keyboard)", key));
                if self.key_press_log.len() > 10 {
                    self.key_press_log.remove(0);
                }
            }
        });
    }
    
    fn render_memory_editor(&mut self, ctx: &egui::Context) {
        if !self.show_memory_editor {
            return;
        }
        
        egui::Window::new("Memory Editor")
            .open(&mut self.show_memory_editor)
            .show(ctx, |ui| {
                ui.label("PC-1500 Memory Editor");
                
                ui.horizontal(|ui| {
                    ui.label("Address:");
                    ui.text_edit_singleline(&mut self.memory_editor_address);
                    
                    if ui.button("Read").clicked() {
                        if let Ok(addr) = u16::from_str_radix(&self.memory_editor_address, 16) {
                            let value = self.emulator.read_memory(addr);
                            self.memory_editor_value = format!("{:02X}", value);
                        }
                    }
                });
                
                ui.horizontal(|ui| {
                    ui.label("Value:");
                    ui.text_edit_singleline(&mut self.memory_editor_value);
                    
                    if ui.button("Write").clicked() {
                        if let (Ok(addr), Ok(value)) = (
                            u16::from_str_radix(&self.memory_editor_address, 16),
                            u8::from_str_radix(&self.memory_editor_value, 16),
                        ) {
                            self.emulator.write_memory(addr, value);
                        }
                    }
                });
                
                ui.separator();
                
                // Display memory region (PC-1500 display memory)
                ui.label("Display Memory (0x7600-0x774F):");
                ui.monospace("Address  | Value | Binary   | PC-1500 Display");
                ui.separator();
                
                for i in 0..16 {
                    let addr = 0x7600 + i;
                    let value = self.emulator.read_memory(addr);
                    let first_char = value & 0x0F;
                    let second_char = (value >> 4) & 0x07;
                    
                    ui.monospace(format!(
                        "0x{:04X}   |  {:02X}  | {:08b} | 1st:0x{:X} 2nd:0x{:X}",
                        addr, value, value, first_char, second_char
                    ));
                }
            });
    }
    
    fn render_cpu_info(&mut self, ctx: &egui::Context) {
        if !self.show_cpu_info {
            return;
        }
        
        egui::Window::new("CPU Information")
            .open(&mut self.show_cpu_info)
            .show(ctx, |ui| {
                let cpu_state = self.emulator.cpu_state();
                
                ui.label("LH5801 CPU State:");
                ui.monospace(format!("PC: 0x{:04X}", cpu_state.pc));
                ui.monospace(format!("A:  0x{:02X}", cpu_state.a));
                ui.monospace(format!("X:  0x{:04X}", cpu_state.x));
                ui.monospace(format!("Y:  0x{:04X}", cpu_state.y));
                ui.monospace(format!("U:  0x{:04X}", cpu_state.u));
                ui.monospace(format!("S:  0x{:04X}", cpu_state.s));
                
                ui.separator();
                
                if self.rom_mode_active {
                    ui.colored_label(egui::Color32::GREEN, "ROM MODE ACTIVE");
                    ui.label("✅ Authentic PC-1500 firmware executing");
                    ui.label("📊 Real-time CPU state from ROM execution");
                } else {
                    ui.colored_label(egui::Color32::YELLOW, "TEST MODE");
                    ui.label("🧪 Test program loaded");
                    ui.label("🔧 Manual control available");
                }
                
                ui.separator();
                ui.label(format!("Frame: {}", self.frame_count));
                ui.label(format!("Cycles: {}", self.cpu_cycles));
            });
    }
    
    fn render_rom_controls(&mut self, ui: &mut egui::Ui) {
        if !self.show_rom_controls {
            return;
        }
        
        ui.group(|ui| {
            ui.label("ROM Controls");
            
            ui.horizontal(|ui| {
                if ui.button("Load ROM").clicked() {
                    self.load_rom_dialog();
                }
                
                if ui.button("Reset to Test Mode").clicked() {
                    self.emulator.init_test_mode();
                    self.emulator.display_message("TEST MODE");
                    self.rom_mode_active = false;
                    self.rom_execution_info = "Reset to test mode".to_string();
                    
                    // Log the action
                    self.key_press_log.push("System reset to test mode".to_string());
                    if self.key_press_log.len() > 10 {
                        self.key_press_log.remove(0);
                    }
                }
            });
            
            if self.rom_mode_active {
                ui.colored_label(egui::Color32::GREEN, "🟢 ROM LOADED AND EXECUTING");
                ui.label("Authentic PC-1500 firmware is running");
                ui.label("Display updates come from ROM execution");
                
                ui.horizontal(|ui| {
                    if ui.button("Show ROM Memory Map").clicked() {
                        // Show ROM memory layout in a new window
                        self.show_memory_map = true;
                    }
                    
                    if ui.button("Show CPU State").clicked() {
                        self.show_cpu_info = true;
                    }
                });
                
                // Show current execution info
                let cpu_state = self.emulator.cpu_state();
                ui.small(format!("PC: 0x{:04X} | A: 0x{:02X} | Frame: {}", 
                    cpu_state.pc, cpu_state.a, self.frame_count));
            } else {
                ui.colored_label(egui::Color32::GRAY, "⚪ TEST MODE");
                ui.label("Test program loaded - ROM not active");
                ui.label("Use keyboard or memory editor for manual control");
                
                ui.horizontal(|ui| {
                    if ui.button("Show Test Memory").clicked() {
                        self.show_memory_map = true;
                    }
                    
                    if ui.button("Load Sample Data").clicked() {
                        // Load some sample data for testing
                        self.emulator.display_message("SAMPLE");
                        for i in 0..20 {
                            self.emulator.write_memory(0x7600 + i, 0x55 + (i % 4) as u8);
                        }
                        
                        self.key_press_log.push("Sample data loaded".to_string());
                        if self.key_press_log.len() > 10 {
                            self.key_press_log.remove(0);
                        }
                    }
                });
            }
        });
    }
    
    fn render_memory_map_window(&mut self, ctx: &egui::Context) {
        if !self.show_memory_map {
            return;
        }
        
        egui::Window::new("PC-1500 Memory Map")
            .open(&mut self.show_memory_map)
            .default_width(600.0)
            .default_height(400.0)
            .show(ctx, |ui| {
                ui.label("PC-1500 Memory Layout:");
                ui.separator();
                
                // ROM/System Memory
                ui.label("📖 ROM & SYSTEM MEMORY:");
                ui.monospace("0x0000-0x3FFF: ROM Area (16KB) - PC-1500_A04.ROM");
                if self.rom_mode_active {
                    ui.colored_label(egui::Color32::GREEN, "  ✅ ROM loaded and active");
                    ui.monospace("  PC-1500_A04.ROM executing authentic firmware");
                } else {
                    ui.colored_label(egui::Color32::YELLOW, "  🧪 Test program loaded");
                    ui.monospace("  Simple test program for basic functionality");
                }
                
                ui.add_space(10.0);
                
                // RAM Memory
                ui.label("💾 RAM MEMORY:");
                ui.monospace("0x8000-0x9FFF: RAM Area (8KB)");
                ui.monospace("  Variables, programs, and user data");
                
                ui.add_space(10.0);
                
                // Display Memory (most important for PC-1500)
                ui.label("📺 DISPLAY MEMORY:");
                ui.monospace("0x7600-0x764F: First section (80 bytes, columns 0-79)");
                ui.monospace("0x7700-0x774F: Second section (80 bytes, columns 80-159)"); 
                ui.monospace("Total: 160 bytes for 156x7 pixel display");
                
                ui.add_space(5.0);
                ui.label("Display format:");
                ui.monospace("  Each byte = 7 dots (bits 0-6)");
                ui.monospace("  Bits 0,1,2,3: 4 DOTS (upper section)");
                ui.monospace("  Bits 4,5,6:   3 DOTS (lower section)");
                ui.monospace("  Bit 7:       Ignored by display");
                
                ui.separator();
                
                // Current CPU State
                if self.rom_mode_active {
                    let cpu_state = self.emulator.cpu_state();
                    ui.label("💻 CURRENT CPU STATE:");
                    ui.monospace(format!("PC: 0x{:04X} (Program Counter)", cpu_state.pc));
                    ui.monospace(format!("A:  0x{:02X}   (Accumulator)", cpu_state.a));
                    ui.monospace(format!("X:  0x{:04X} (Index Register X)", cpu_state.x));
                    ui.monospace(format!("Y:  0x{:04X} (Index Register Y)", cpu_state.y));
                    ui.monospace(format!("U:  0x{:04X} (User Stack)", cpu_state.u));
                    ui.monospace(format!("S:  0x{:04X} (System Stack)", cpu_state.s));
                } else {
                    ui.label("💻 CPU STATE: Test Mode - Limited functionality");
                }
                
                ui.separator();
                
                // Display Memory Sample
                ui.label("🔍 DISPLAY MEMORY SAMPLE (First 16 bytes):");
                ui.monospace("Addr  | Hex | Binary   | 4-DOTS | 3-DOTS | Visual");
                for i in 0..16 {
                    let addr = 0x7600 + i;
                    let value = self.emulator.read_memory(addr);
                    let first_char = value & 0x0F;          // bits 0,1,2,3
                    let second_char = (value >> 4) & 0x07;  // bits 4,5,6
                    
                    // Visual representation
                    let mut visual = String::new();
                    for bit in 0..7 {
                        visual.push(if (value >> bit) & 1 != 0 { '█' } else { '·' });
                    }
                    
                    ui.monospace(format!(
                        "0x{:04X}|  {:02X} | {:08b} |   0x{:X}  |   0x{:X}  | {}",
                        addr, value, value, first_char, second_char, visual
                    ));
                }
                
                if ui.button("🔄 Refresh Memory View").clicked() {
                    // Force refresh of the display
                }
            });
    }
    
    // Key handling functions - COMPLETE RESTORATION WITH TIMING
    fn send_key_press(&mut self, key: Pc1500Key) {
        // Add key to pressed set for visual feedback
        self.pressed_keys.insert(key);
        
        // Record the press time for auto-release
        self.key_press_timers.insert(key, std::time::Instant::now());
        
        // Send key press to emulator
        self.emulator.press_key(key as u32);
        
        // Log key press for debugging
        self.key_press_log.push(format!("Key pressed: {:?} (physical keyboard)", key));
        if self.key_press_log.len() > 10 {
            self.key_press_log.remove(0);
        }
    }
    
    fn send_key_release(&mut self, key: Pc1500Key) {
        // Remove key from pressed set
        self.pressed_keys.remove(&key);
        self.key_press_timers.remove(&key);
        
        // Send key release to emulator
        self.emulator.release_key(key as u32);
    }
    
    fn handle_physical_keyboard(&mut self, ctx: &egui::Context) {
        // Check for physical keyboard input
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key { key, pressed, .. } = event {
                    if let Some(&pc1500_key) = self.pc_to_pc1500_mapping.get(key) {
                        if *pressed {
                            self.send_key_press(pc1500_key);
                        } else {
                            self.send_key_release(pc1500_key);
                        }
                    }
                }
            }
        });
    }
    
    fn update_key_timers(&mut self) {
        // Auto-release keys after 150ms for visual feedback
        let now = std::time::Instant::now();
        let keys_to_release: Vec<Pc1500Key> = self.key_press_timers
            .iter()
            .filter(|(_, press_time)| now.duration_since(**press_time).as_millis() > 150)
            .map(|(key, _)| *key)
            .collect();
        
        for key in keys_to_release {
            self.send_key_release(key);
        }
    }
    
    fn schedule_key_release(&mut self, key: Pc1500Key) {
        // This function is now handled by update_key_timers() automatically
        // Keys are released after 150ms for visual feedback
        // Just ensure the key is in the timer system
        if !self.key_press_timers.contains_key(&key) {
            self.key_press_timers.insert(key, std::time::Instant::now());
        }
    }
    
    // ROM and system functions
    fn load_rom_dialog(&mut self) {
        // Try multiple ROM file names that might exist
        let rom_files = [
            "PC-1500_A04.ROM", 
            "PC-1500.ROM",
            "pc1500.rom", 
            "pc1500_a04.rom",
            "PC1500.BIN"
        ];
        
        let mut rom_loaded = false;
        let mut rom_file_used = String::new();
        
        for rom_file in &rom_files {
            match self.emulator.load_rom(rom_file) {
                Ok(()) => {
                    self.rom_mode_active = true;
                    rom_file_used = rom_file.to_string();
                    rom_loaded = true;
                    break;
                }
                Err(_) => continue,
            }
        }
        
        if rom_loaded {
            self.rom_execution_info = format!(
                "ROM LOADED: {} - Authentic PC-1500 firmware executing", 
                rom_file_used
            );
            
            // Update key log
            self.key_press_log.push(format!("ROM loaded: {}", rom_file_used));
            if self.key_press_log.len() > 10 {
                self.key_press_log.remove(0);
            }
        } else {
            self.rom_execution_info = format!(
                "ROM NOT FOUND - Tried: {} - Continuing in test mode", 
                rom_files.join(", ")
            );
            
            // Update key log
            self.key_press_log.push("ROM load failed - using test mode".to_string());
            if self.key_press_log.len() > 10 {
                self.key_press_log.remove(0);
            }
        }
    }
    
    fn try_load_default_rom(&mut self) {
        if let Err(_) = self.emulator.load_rom("PC-1500_A04.ROM") {
            self.emulator.init_test_mode();
            self.emulator.display_message("ROM NOT FOUND");
        }
    }
    
    fn reset_system(&mut self) {
        self.emulator.init_test_mode();
        self.emulator.display_message("SYSTEM RESET");
        self.pressed_keys.clear();
        self.rom_mode_active = false;
        self.rom_execution_info = "System Reset - Test Mode Active".to_string();
    }
    
    fn clear_display(&mut self) {
        for i in 0x7600..=0x774F {
            self.emulator.write_memory(i, 0x00);
        }
    }
    
    fn show_test_pattern(&mut self) {
        // Write test pattern to display memory
        for i in 0..80 {
            self.emulator.write_memory(0x7600 + i, (i % 16) as u8);
            self.emulator.write_memory(0x7700 + i, ((i + 8) % 16) as u8);
        }
        
        // Log the action
        self.key_press_log.push("Test pattern written to display".to_string());
        if self.key_press_log.len() > 10 {
            self.key_press_log.remove(0);
        }
    }
    
    fn test_pc1500_display_system(&mut self) {
        // Advanced test pattern similar to pc1500_memory_writer.rs
        
        // Clear both sections first
        for i in 0..80 {
            self.emulator.write_memory(0x7600 + i, 0x00);
            self.emulator.write_memory(0x7700 + i, 0x00);
        }
        
        // Test first char (bits 0,1,2,3) - 4 DOTS in first section
        for i in 0..16u16 {
            let pattern = i as u8; // First char 0-F in bits 0,1,2,3
            self.emulator.write_memory(0x7600 + 20 + i, pattern);
        }
        
        // Test second char (bits 4,5,6) - 3 DOTS in first section
        for i in 0..8u16 {
            let pattern = (i << 4) as u8; // Second char 0-7 in bits 4,5,6
            self.emulator.write_memory(0x7600 + 40 + i, pattern);
        }
        
        // Test combined patterns in second section
        for i in 0..8u16 {
            let first_char = i + 8;  // 8-F
            let second_char = i;     // 0-7
            let pattern = (first_char | (second_char << 4)) as u8;
            self.emulator.write_memory(0x7700 + i, pattern);
        }
        
        // Test full patterns in second section
        for i in 0..16u16 {
            let pattern = (0x70 | i) as u8; // Mixed pattern
            self.emulator.write_memory(0x7700 + 20 + i, pattern);
        }
        
        // Log the action
        self.key_press_log.push("PC-1500 display system test executed".to_string());
        if self.key_press_log.len() > 10 {
            self.key_press_log.remove(0);
        }
    }
    
    fn show_system_info(&mut self) {
        // Show system information in the key log
        let cpu_state = self.emulator.cpu_state();
        
        if self.emulator.is_rom_mode() {
            self.key_press_log.push("SYSTEM: ROM MODE ACTIVE".to_string());
            self.key_press_log.push("Running authentic PC-1500 firmware".to_string());
        } else {
            self.key_press_log.push("SYSTEM: TEST MODE ACTIVE".to_string());
            self.key_press_log.push("Running test program".to_string());
        }
        
        self.key_press_log.push(format!("CPU PC: 0x{:04X}", cpu_state.pc));
        self.key_press_log.push(format!("CPU A: 0x{:02X}", cpu_state.a));
        self.key_press_log.push(format!("Frame: {}", self.frame_count));
        
        // Keep log size reasonable
        while self.key_press_log.len() > 10 {
            self.key_press_log.remove(0);
        }
    }
}

impl eframe::App for Pc1500App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle physical keyboard input FIRST
        self.handle_physical_keyboard(ctx);
        
        // Update key timers for visual feedback
        self.update_key_timers();
        
        // Update emulator
        self.update_emulator();
        
        // Request continuous repaints for smooth animation
        ctx.request_repaint();
        
        // Main UI
        egui::CentralPanel::default().show(ctx, |ui| {
            // Menu bar
            self.render_menu_bar(ui);
            
            ui.separator();
            
            // Main display
            self.render_main_display(ui);
            
            ui.separator();
            
            // PC-1500 keyboard
            self.render_pc1500_keyboard(ui);
            
            ui.separator();
            
            // ROM controls
            self.render_rom_controls(ui);
            
            // Debug info if enabled
            if self.debug_mode {
                ui.separator();
                ui.label(format!("Debug: Frame {}, Keys: {:?}", 
                    self.frame_count, 
                    self.pressed_keys
                ));
                
                // Show key press log
                ui.label("Recent key presses:");
                for log_entry in &self.key_press_log {
                    ui.small(log_entry);
                }
                
                // Show keyboard mapping status
                ui.label(format!("Physical keyboard enabled: {}", self.keyboard_enabled));
                ui.label(format!("Active key timers: {}", self.key_press_timers.len()));
            }
        });
        
        // Additional windows - these create their own UI contexts
        self.render_memory_editor(ctx);
        self.render_cpu_info(ctx);
        self.render_memory_map_window(ctx);
    }
}
