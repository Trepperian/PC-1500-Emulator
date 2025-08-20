use ceres_core::Pc1500;
use ceres_core::keyboard::Key as Pc1500Key;
use eframe::egui;
use std::collections::HashSet;

pub struct Pc1500App {
    // CORE EMULATOR - The real PC-1500 system
    emulator: Pc1500,

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
        let emulator = Pc1500::new();

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

        // Update display buffer
        let pixel_data = self.emulator.display().rgba_buffer();
        self.display_buffer.copy_from_slice(pixel_data);
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.show_keyboard, "PC-1500 Keyboard");
                ui.checkbox(&mut self.show_memory_editor, "Memory Editor");
                ui.checkbox(&mut self.show_display_debug, "Display Debug");
                ui.checkbox(&mut self.show_cpu_info, "CPU Information");
                ui.checkbox(&mut self.show_rom_controls, "ROM Controls");
                ui.separator();
                ui.checkbox(&mut self.debug_mode, "Debug Mode");
            });
        });
    }

    fn render_main_display(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label("PC-1500 Display (156×7 pixels)");

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
                self.key_press_log
                    .push(format!("Key clicked: {:?} (virtual keyboard)", key));
                if self.key_press_log.len() > 10 {
                    self.key_press_log.remove(0);
                }
            }
        });
    }

    fn send_key_press(&mut self, key: Pc1500Key) {
        // Add key to pressed set for visual feedback
        self.pressed_keys.insert(key);

        // Record the press time for auto-release
        self.key_press_timers.insert(key, std::time::Instant::now());

        // Send key press to emulator
        self.emulator.press(key);

        // Log key press for debugging
        self.key_press_log
            .push(format!("Key pressed: {:?} (physical keyboard)", key));
        if self.key_press_log.len() > 10 {
            self.key_press_log.remove(0);
        }
    }

    fn send_key_release(&mut self, key: Pc1500Key) {
        // Remove key from pressed set
        self.pressed_keys.remove(&key);
        self.key_press_timers.remove(&key);

        // Send key release to emulator
        self.emulator.release(key);
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
        let keys_to_release: Vec<Pc1500Key> = self
            .key_press_timers
            .iter()
            .filter(|(_, press_time)| now.duration_since(**press_time).as_millis() > 150)
            .map(|(key, _)| *key)
            .collect();

        for key in keys_to_release {
            self.send_key_release(key);
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

            // Debug info if enabled
            if self.debug_mode {
                ui.separator();
                ui.label(format!(
                    "Debug: Frame {}, Keys: {:?}",
                    self.frame_count, self.pressed_keys
                ));

                // Show key press log
                ui.label("Recent key presses:");
                for log_entry in &self.key_press_log {
                    ui.small(log_entry);
                }

                // Show keyboard mapping status
                ui.label(format!(
                    "Physical keyboard enabled: {}",
                    self.keyboard_enabled
                ));
                ui.label(format!(
                    "Active key timers: {}",
                    self.key_press_timers.len()
                ));
            }
        });
    }
}
