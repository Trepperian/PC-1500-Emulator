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

    // DISPLAY - Full display system
    display_buffer: Vec<u8>,
    display_width: usize,
    display_height: usize,
    display_scale: f32,

    // SYMBOL STATES - Cache for rendering
    symbol_states: [bool; 14],

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
            display_buffer: vec![0; 156 * 7 * 4], // RGBA buffer
            display_width: 156,
            display_height: 7,
            display_scale: 6.0,
            symbol_states: [false; 14],
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
        mapping.insert(egui::Key::Exclamationmark, Pc1500Key::F1);
        mapping.insert(egui::Key::Quote, Pc1500Key::F2);
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

        // Parentheses
        mapping.insert(egui::Key::OpenBracket, Pc1500Key::LeftParen); // [ maps to (
        mapping.insert(egui::Key::CloseBracket, Pc1500Key::RightParen); // ] maps to )

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

        // Update display buffer
        let display = self.emulator.display();
        let pixel_data = display.rgba_buffer();
        self.display_buffer.copy_from_slice(pixel_data);

        // Cache symbol states for rendering
        use ceres_core::display::Symbol;
        self.symbol_states[Symbol::Busy as usize] = display.is_symbol_on(Symbol::Busy);
        self.symbol_states[Symbol::Shift as usize] = display.is_symbol_on(Symbol::Shift);
        self.symbol_states[Symbol::Kana as usize] = display.is_symbol_on(Symbol::Kana);
        self.symbol_states[Symbol::Small as usize] = display.is_symbol_on(Symbol::Small);
        self.symbol_states[Symbol::Deg as usize] = display.is_symbol_on(Symbol::Deg);
        self.symbol_states[Symbol::Rad as usize] = display.is_symbol_on(Symbol::Rad);
        self.symbol_states[Symbol::Run as usize] = display.is_symbol_on(Symbol::Run);
        self.symbol_states[Symbol::Pro as usize] = display.is_symbol_on(Symbol::Pro);
        self.symbol_states[Symbol::Reserve as usize] = display.is_symbol_on(Symbol::Reserve);
        self.symbol_states[Symbol::Def as usize] = display.is_symbol_on(Symbol::Def);
        self.symbol_states[Symbol::RomanI as usize] = display.is_symbol_on(Symbol::RomanI);
        self.symbol_states[Symbol::RomanII as usize] = display.is_symbol_on(Symbol::RomanII);
        self.symbol_states[Symbol::RomanIII as usize] = display.is_symbol_on(Symbol::RomanIII);
        self.symbol_states[Symbol::Battery as usize] = display.is_symbol_on(Symbol::Battery);
    }

    fn render_main_display(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            // First render the symbols above the display
            self.render_symbols(ui);

            ui.add_space(5.0);

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

    fn render_symbols(&mut self, ui: &mut egui::Ui) {
        use ceres_core::display::Symbol;

        ui.horizontal(|ui| {
            ui.add_space(20.0); // Align with display area

            // Define symbol positions roughly matching the PC-1500 layout
            let symbols = [
                ("BUSY", Symbol::Busy),
                ("SHIFT", Symbol::Shift),
                ("KANA", Symbol::Kana),
                ("SML", Symbol::Small),
                ("DEG", Symbol::Deg),
                ("RAD", Symbol::Rad),
                ("RUN", Symbol::Run),
                ("PRO", Symbol::Pro),
                ("RSV", Symbol::Reserve),
                ("DEF", Symbol::Def),
                ("I", Symbol::RomanI),
                ("II", Symbol::RomanII),
                ("III", Symbol::RomanIII),
                ("●", Symbol::Battery),
            ];

            for (label, symbol) in symbols.iter() {
                let is_on = self.symbol_states[*symbol as usize];
                let color = if is_on {
                    egui::Color32::BLACK
                } else {
                    egui::Color32::LIGHT_GRAY
                };

                ui.colored_label(color, *label);
                ui.add_space(15.0);
            }
        });
    }

    fn render_pc1500_keyboard(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
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

            // Row 1: DEF + Function keys + SHIFT + OFF/ON
            ui.horizontal(|ui| {
                create_key(ui, Pc1500Key::Control, "DEF", 35.0);
                ui.add_space(15.0);
                create_key(ui, Pc1500Key::F1, "!", 30.0);
                create_key(ui, Pc1500Key::F2, "\"", 30.0);
                create_key(ui, Pc1500Key::F3, "#", 30.0);
                create_key(ui, Pc1500Key::F4, "$", 30.0);
                create_key(ui, Pc1500Key::F5, "%", 30.0);
                create_key(ui, Pc1500Key::F6, "&", 30.0);
                ui.add_space(25.0);
                create_key(ui, Pc1500Key::Shift, "SHIFT", 50.0);
                ui.add_space(30.0);
                create_key(ui, Pc1500Key::Off, "OFF", 35.0);
                create_key(ui, Pc1500Key::On, "ON", 30.0);
            });

            ui.add_space(3.0);

            // Row 2: QWERTYUIOP + 789/?CL
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
                ui.add_space(15.0);
                create_key(ui, Pc1500Key::Seven, "7", 25.0);
                create_key(ui, Pc1500Key::Eight, "8", 25.0);
                create_key(ui, Pc1500Key::Nine, "9", 25.0);
                create_key(ui, Pc1500Key::Slash, "/", 25.0);
                create_key(ui, Pc1500Key::Cl, "CL", 35.0);
            });

            ui.add_space(3.0);

            // Row 3: ASDFGHJKL + 456*MODE
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
                ui.add_space(40.0);
                create_key(ui, Pc1500Key::Four, "4", 25.0);
                create_key(ui, Pc1500Key::Five, "5", 25.0);
                create_key(ui, Pc1500Key::Six, "6", 25.0);
                create_key(ui, Pc1500Key::Asterisk, "*", 25.0);
                create_key(ui, Pc1500Key::Mode, "MODE", 45.0);
            });

            ui.add_space(3.0);

            // Row 4: ZXCVBNM() + 123-←
            ui.horizontal(|ui| {
                create_key(ui, Pc1500Key::Z, "Z", 25.0);
                create_key(ui, Pc1500Key::X, "X", 25.0);
                create_key(ui, Pc1500Key::C, "C", 25.0);
                create_key(ui, Pc1500Key::V, "V", 25.0);
                create_key(ui, Pc1500Key::B, "B", 25.0);
                create_key(ui, Pc1500Key::N, "N", 25.0);
                create_key(ui, Pc1500Key::M, "M", 25.0);
                create_key(ui, Pc1500Key::LeftParen, "(", 25.0);
                create_key(ui, Pc1500Key::RightParen, ")", 25.0);
                ui.add_space(15.0);
                create_key(ui, Pc1500Key::One, "1", 25.0);
                create_key(ui, Pc1500Key::Two, "2", 25.0);
                create_key(ui, Pc1500Key::Three, "3", 25.0);
                create_key(ui, Pc1500Key::Minus, "-", 25.0);
                create_key(ui, Pc1500Key::Left, "Left", 25.0);
            });

            ui.add_space(3.0);

            ui.horizontal(|ui| {
                create_key(ui, Pc1500Key::Sml, "SML", 35.0);
                create_key(ui, Pc1500Key::Rcl, "RCL", 35.0);
                create_key(ui, Pc1500Key::Space, "SPACE", 100.0);
                create_key(ui, Pc1500Key::Down, "Down", 25.0);
                create_key(ui, Pc1500Key::Up, "Up", 25.0);
                create_key(ui, Pc1500Key::Enter, "ENTER", 80.0);
                ui.add_space(5.0);
                create_key(ui, Pc1500Key::Zero, "0", 25.0);
                create_key(ui, Pc1500Key::Dot, ".", 25.0);
                create_key(ui, Pc1500Key::Equals, "=", 25.0);
                create_key(ui, Pc1500Key::Plus, "+", 25.0);
                create_key(ui, Pc1500Key::Right, "Right", 25.0);
            });

            // Process clicked keys from virtual keyboard
            for key in clicked_keys {
                self.send_key_press(key);
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
            // Main display
            self.render_main_display(ui);

            ui.separator();

            // PC-1500 keyboard
            self.render_pc1500_keyboard(ui);
        });
    }
}
