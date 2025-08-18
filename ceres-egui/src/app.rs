// NOTE: This file currently uses GameBoy components (GbThread, ceres_std::Button)
// which are incompatible with PC-1500. You have two options:
//
// 1. USE PC1500_APP.RS: The file pc1500_app.rs already has a complete PC-1500 implementation
//    with proper keyboard mapping, display system, and ROM loading.
//
// 2. CONVERT THIS FILE: Replace GbThread with a PC-1500 instance and complete the TODO
//    items below to make this work with PC-1500.
//
// For now, we've created the complete key mapping structure below, but you need to
// decide which approach to take.

// use ceres_core::joypad::Key as Pc1500Key; // TODO: Uncomment when converting to PC-1500
use ceres_std::Pc1500Thread; // TODO: Replace with PC-1500 equivalent
use ceres_std::{AppOption, ShaderOption};
use eframe::egui::{self, CornerRadius, Key, style::HandleShape};
use rfd::FileDialog;
use std::{
    // fs::File, // TODO: Will be needed when implementing save functionality
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::screen;

fn setup_theme(ctx: &egui::Context) {
    let bg0 = egui::Color32::from_rgb(40, 40, 40); // Background
    let bg1 = egui::Color32::from_rgb(60, 56, 54); // Lighter background
    let bg2 = egui::Color32::from_rgb(80, 73, 69); // Selection background
    let fg0 = egui::Color32::from_rgb(251, 241, 199); // Main text
    let fg1 = egui::Color32::from_rgb(235, 219, 178); // Secondary text
    // let red = egui::Color32::from_rgb(204, 36, 29); // Red accent
    // let green = egui::Color32::from_rgb(152, 151, 26); // Green accent
    let yellow = egui::Color32::from_rgb(215, 153, 33); // Yellow accent
    // let orange = egui::Color32::from_rgb(214, 93, 14); // Orange accent
    let blue = egui::Color32::from_rgb(69, 133, 136); // Blue accent
    // let aqua = egui::Color32::from_rgb(104, 157, 106); // Aqua accent

    let mut style = (*ctx.style()).clone();

    style.visuals.window_fill = bg0;
    style.visuals.panel_fill = bg0;

    style.visuals.widgets.inactive.bg_fill = bg0;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, fg1);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = bg0;

    style.visuals.widgets.noninteractive.bg_fill = bg0;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.noninteractive.weak_bg_fill = bg0;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.5, fg1);

    style.visuals.widgets.hovered.bg_fill = bg1;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.hovered.weak_bg_fill = bg1;
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, fg0);

    style.visuals.widgets.active.bg_fill = bg2;
    style.visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.active.weak_bg_fill = bg2;
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0, yellow);

    style.visuals.widgets.open.bg_fill = bg1;
    style.visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.open.weak_bg_fill = bg1;
    style.visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, fg0);

    let corner_radius = CornerRadius::same(2);
    style.visuals.window_corner_radius = corner_radius;
    style.visuals.menu_corner_radius = corner_radius;
    style.visuals.widgets.noninteractive.corner_radius = corner_radius;
    style.visuals.widgets.inactive.corner_radius = corner_radius;
    style.visuals.widgets.hovered.corner_radius = corner_radius;
    style.visuals.widgets.active.corner_radius = corner_radius;
    style.visuals.widgets.open.corner_radius = corner_radius;

    let shadow = egui::epaint::Shadow {
        offset: [1, 1],
        blur: 5,
        spread: 0,
        color: bg0,
    };
    style.visuals.popup_shadow = shadow;
    style.visuals.window_shadow = shadow;
    style.visuals.handle_shape = HandleShape::Rect { aspect_ratio: 0.5 };
    style.visuals.window_stroke = egui::Stroke {
        width: 0.0,
        color: fg1,
    };
    style.visuals.selection.bg_fill = bg2;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, yellow);

    style.visuals.hyperlink_color = blue;

    style.visuals.override_text_color = Some(fg0);

    ctx.set_style(style);
}

pub struct PainterCallbackImpl {
    ctx: egui::Context,
    buffer: Arc<Mutex<Box<[u8]>>>,
}

impl PainterCallbackImpl {
    pub fn new(ctx: &egui::Context, buffer: Arc<Mutex<Box<[u8]>>>) -> Self {
        Self {
            ctx: ctx.clone(),
            buffer,
        }
    }
}

impl ceres_std::PainterCallback for PainterCallbackImpl {
    fn paint(&self, pixel_data_rgba: &[u8]) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.copy_from_slice(pixel_data_rgba);
        }
    }

    fn request_repaint(&self) {
        self.ctx.request_repaint();
    }
}

pub struct App {
    project_dirs: directories::ProjectDirs,
    thread: Pc1500Thread,
    screen:
        screen::GBScreen<{ ceres_std::DISPLAY_WIDTH as u32 }, { ceres_std::DISPLAY_HEIGHT as u32 }>,
    rom_path: Option<PathBuf>,
    sav_path: Option<PathBuf>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model: ceres_std::Model,
        project_dirs: directories::ProjectDirs,
        rom_path: Option<&std::path::Path>,
        shader_option: ShaderOption,
    ) -> anyhow::Result<Self> {
        // Apply our minimal black and white theme
        setup_theme(&cc.egui_ctx);

        let sav_path = rom_path.and_then(|path| Self::sav_path_from_rom_path(&project_dirs, path));

        let pixel_data_rgba = Arc::new(Mutex::new(
            vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
        ));

        let mut thread = Pc1500Thread::new(
            model,
            sav_path.as_deref(),
            rom_path,
            PainterCallbackImpl::new(&cc.egui_ctx, Arc::clone(&pixel_data_rgba)),
        )?;

        let screen = screen::GBScreen::new(cc, pixel_data_rgba, shader_option);

        thread.resume()?;

        Ok(Self {
            project_dirs,
            thread,
            screen,
            rom_path: rom_path.map(std::path::Path::to_path_buf),
            sav_path,
        })
    }

    fn sav_path_from_rom_path(
        project_dirs: &directories::ProjectDirs,
        rom_path: &std::path::Path,
    ) -> Option<PathBuf> {
        let file_stem = rom_path.file_stem()?;
        Some(
            project_dirs
                .data_dir()
                .join(file_stem)
                .with_extension("sav"),
        )
    }
}

impl eframe::App for App {
    #[expect(clippy::too_many_lines)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |top_panel_ui| {
            egui::MenuBar::new().ui(top_panel_ui, |menu_bar_ui| {
                menu_bar_ui.menu_button("File", |menu_button_ui| {
                    if menu_button_ui.button("Open").clicked() {
                        let file = FileDialog::new()
                            .add_filter("gb", &["gb", "gbc"])
                            .pick_file();

                        if let Some(file) = file {
                            let sav_path = Self::sav_path_from_rom_path(&self.project_dirs, &file);

                            // Placeholder: Set paths without loading
                            self.sav_path = sav_path;
                            self.rom_path = Some(file);
                            println!("ROM selected: {:?}", self.rom_path);
                        }
                    }

                    if menu_button_ui.button("Export").clicked() {
                        println!("Export save file");
                    }
                });

                menu_bar_ui.menu_button("View", |menu_button_ui| {
                    menu_button_ui.horizontal(|horizontal_ui| {
                        let paused = self.thread.is_paused();
                        if horizontal_ui
                            .selectable_label(paused, if paused { "\u{25b6}" } else { "\u{23f8}" })
                            .on_hover_text("Pause")
                            .clicked()
                        {
                            if let Err(e) = if paused {
                                self.thread.resume()
                            } else {
                                self.thread.pause()
                            } {
                                eprintln!("couldn't pause/resume: {e}");
                            }
                        }

                        // TODO: Add speed controls (1x, 2x, 4x) when implementing PC-1500 thread
                        // let multiplier = self.thread.multiplier();
                    });

                    // Audio/Volume controls removed - not applicable for PC-1500

                    menu_button_ui.menu_button("Shader", |menu_button_ui| {
                        for shader_option in ShaderOption::iter() {
                            let shader_button = egui::Button::selectable(
                                self.screen.shader_option() == shader_option,
                                shader_option.str(),
                            );

                            if menu_button_ui.add(shader_button).clicked() {
                                *self.screen.shader_option_mut() = shader_option;
                            }
                        }
                    });

                    // Scaling menu removed - ScalingOption no longer available
                    /*
                    menu_button_ui.menu_button("Scaling", |menu_button_ui| {
                        for pixel_mode in ScalingOption::iter() {
                            let pixel_button = egui::Button::selectable(
                                self.screen.pixel_mode() == pixel_mode,
                                pixel_mode.str(),
                            );

                            if menu_button_ui.add(pixel_button).clicked() {
                                *self.screen.mut_pixel_mode() = pixel_mode;
                            }
                        }
                    });
                    */
                });
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: egui::Margin::default(),
                outer_margin: egui::Margin::default(),
                corner_radius: egui::CornerRadius::default(),
                shadow: egui::Shadow::default(),
                fill: egui::Color32::BLACK,
                stroke: egui::Stroke::NONE,
            })
            .show(ctx, |central_panel_ui| {
                self.screen.custom_painting(central_panel_ui);
            });

        // Handle PC-1500 keyboard input - Complete key mapping
        ctx.input(|i| {
            // Numbers 0-9
            if i.key_pressed(Key::Num0) { /* TODO: Add PC-1500 instance to press Pc1500Key::Zero */ }
            if i.key_released(Key::Num0) { /* TODO: Add PC-1500 instance to release Pc1500Key::Zero */ }
            if i.key_pressed(Key::Num1) { /* TODO: Add PC-1500 instance to press Pc1500Key::One */ }
            if i.key_released(Key::Num1) { /* TODO: Add PC-1500 instance to release Pc1500Key::One */ }
            if i.key_pressed(Key::Num2) { /* TODO: Add PC-1500 instance to press Pc1500Key::Two */ }
            if i.key_released(Key::Num2) { /* TODO: Add PC-1500 instance to release Pc1500Key::Two */ }
            if i.key_pressed(Key::Num3) { /* TODO: Add PC-1500 instance to press Pc1500Key::Three */ }
            if i.key_released(Key::Num3) { /* TODO: Add PC-1500 instance to release Pc1500Key::Three */ }
            if i.key_pressed(Key::Num4) { /* TODO: Add PC-1500 instance to press Pc1500Key::Four */ }
            if i.key_released(Key::Num4) { /* TODO: Add PC-1500 instance to release Pc1500Key::Four */ }
            if i.key_pressed(Key::Num5) { /* TODO: Add PC-1500 instance to press Pc1500Key::Five */ }
            if i.key_released(Key::Num5) { /* TODO: Add PC-1500 instance to release Pc1500Key::Five */ }
            if i.key_pressed(Key::Num6) { /* TODO: Add PC-1500 instance to press Pc1500Key::Six */ }
            if i.key_released(Key::Num6) { /* TODO: Add PC-1500 instance to release Pc1500Key::Six */ }
            if i.key_pressed(Key::Num7) { /* TODO: Add PC-1500 instance to press Pc1500Key::Seven */ }
            if i.key_released(Key::Num7) { /* TODO: Add PC-1500 instance to release Pc1500Key::Seven */ }
            if i.key_pressed(Key::Num8) { /* TODO: Add PC-1500 instance to press Pc1500Key::Eight */ }
            if i.key_released(Key::Num8) { /* TODO: Add PC-1500 instance to release Pc1500Key::Eight */ }
            if i.key_pressed(Key::Num9) { /* TODO: Add PC-1500 instance to press Pc1500Key::Nine */ }
            if i.key_released(Key::Num9) { /* TODO: Add PC-1500 instance to release Pc1500Key::Nine */ }

            // Letters A-Z
            if i.key_pressed(Key::A) { /* TODO: Add PC-1500 instance to press Pc1500Key::A */ }
            if i.key_released(Key::A) { /* TODO: Add PC-1500 instance to release Pc1500Key::A */ }
            if i.key_pressed(Key::B) { /* TODO: Add PC-1500 instance to press Pc1500Key::B */ }
            if i.key_released(Key::B) { /* TODO: Add PC-1500 instance to release Pc1500Key::B */ }
            if i.key_pressed(Key::C) { /* TODO: Add PC-1500 instance to press Pc1500Key::C */ }
            if i.key_released(Key::C) { /* TODO: Add PC-1500 instance to release Pc1500Key::C */ }
            if i.key_pressed(Key::D) { /* TODO: Add PC-1500 instance to press Pc1500Key::D */ }
            if i.key_released(Key::D) { /* TODO: Add PC-1500 instance to release Pc1500Key::D */ }
            if i.key_pressed(Key::E) { /* TODO: Add PC-1500 instance to press Pc1500Key::E */ }
            if i.key_released(Key::E) { /* TODO: Add PC-1500 instance to release Pc1500Key::E */ }
            if i.key_pressed(Key::F) { /* TODO: Add PC-1500 instance to press Pc1500Key::F */ }
            if i.key_released(Key::F) { /* TODO: Add PC-1500 instance to release Pc1500Key::F */ }
            if i.key_pressed(Key::G) { /* TODO: Add PC-1500 instance to press Pc1500Key::G */ }
            if i.key_released(Key::G) { /* TODO: Add PC-1500 instance to release Pc1500Key::G */ }
            if i.key_pressed(Key::H) { /* TODO: Add PC-1500 instance to press Pc1500Key::H */ }
            if i.key_released(Key::H) { /* TODO: Add PC-1500 instance to release Pc1500Key::H */ }
            if i.key_pressed(Key::I) { /* TODO: Add PC-1500 instance to press Pc1500Key::I */ }
            if i.key_released(Key::I) { /* TODO: Add PC-1500 instance to release Pc1500Key::I */ }
            if i.key_pressed(Key::J) { /* TODO: Add PC-1500 instance to press Pc1500Key::J */ }
            if i.key_released(Key::J) { /* TODO: Add PC-1500 instance to release Pc1500Key::J */ }
            if i.key_pressed(Key::K) { /* TODO: Add PC-1500 instance to press Pc1500Key::K */ }
            if i.key_released(Key::K) { /* TODO: Add PC-1500 instance to release Pc1500Key::K */ }
            if i.key_pressed(Key::L) { /* TODO: Add PC-1500 instance to press Pc1500Key::L */ }
            if i.key_released(Key::L) { /* TODO: Add PC-1500 instance to release Pc1500Key::L */ }
            if i.key_pressed(Key::M) { /* TODO: Add PC-1500 instance to press Pc1500Key::M */ }
            if i.key_released(Key::M) { /* TODO: Add PC-1500 instance to release Pc1500Key::M */ }
            if i.key_pressed(Key::N) { /* TODO: Add PC-1500 instance to press Pc1500Key::N */ }
            if i.key_released(Key::N) { /* TODO: Add PC-1500 instance to release Pc1500Key::N */ }
            if i.key_pressed(Key::O) { /* TODO: Add PC-1500 instance to press Pc1500Key::O */ }
            if i.key_released(Key::O) { /* TODO: Add PC-1500 instance to release Pc1500Key::O */ }
            if i.key_pressed(Key::P) { /* TODO: Add PC-1500 instance to press Pc1500Key::P */ }
            if i.key_released(Key::P) { /* TODO: Add PC-1500 instance to release Pc1500Key::P */ }
            if i.key_pressed(Key::Q) { /* TODO: Add PC-1500 instance to press Pc1500Key::Q */ }
            if i.key_released(Key::Q) { /* TODO: Add PC-1500 instance to release Pc1500Key::Q */ }
            if i.key_pressed(Key::R) { /* TODO: Add PC-1500 instance to press Pc1500Key::R */ }
            if i.key_released(Key::R) { /* TODO: Add PC-1500 instance to release Pc1500Key::R */ }
            if i.key_pressed(Key::S) { /* TODO: Add PC-1500 instance to press Pc1500Key::S */ }
            if i.key_released(Key::S) { /* TODO: Add PC-1500 instance to release Pc1500Key::S */ }
            if i.key_pressed(Key::T) { /* TODO: Add PC-1500 instance to press Pc1500Key::T */ }
            if i.key_released(Key::T) { /* TODO: Add PC-1500 instance to release Pc1500Key::T */ }
            if i.key_pressed(Key::U) { /* TODO: Add PC-1500 instance to press Pc1500Key::U */ }
            if i.key_released(Key::U) { /* TODO: Add PC-1500 instance to release Pc1500Key::U */ }
            if i.key_pressed(Key::V) { /* TODO: Add PC-1500 instance to press Pc1500Key::V */ }
            if i.key_released(Key::V) { /* TODO: Add PC-1500 instance to release Pc1500Key::V */ }
            if i.key_pressed(Key::W) { /* TODO: Add PC-1500 instance to press Pc1500Key::W */ }
            if i.key_released(Key::W) { /* TODO: Add PC-1500 instance to release Pc1500Key::W */ }
            if i.key_pressed(Key::X) { /* TODO: Add PC-1500 instance to press Pc1500Key::X */ }
            if i.key_released(Key::X) { /* TODO: Add PC-1500 instance to release Pc1500Key::X */ }
            if i.key_pressed(Key::Y) { /* TODO: Add PC-1500 instance to press Pc1500Key::Y */ }
            if i.key_released(Key::Y) { /* TODO: Add PC-1500 instance to release Pc1500Key::Y */ }
            if i.key_pressed(Key::Z) { /* TODO: Add PC-1500 instance to press Pc1500Key::Z */ }
            if i.key_released(Key::Z) { /* TODO: Add PC-1500 instance to release Pc1500Key::Z */ }

            // Function keys
            if i.key_pressed(Key::F1) { /* TODO: Add PC-1500 instance to press Pc1500Key::F1 */ }
            if i.key_released(Key::F1) { /* TODO: Add PC-1500 instance to release Pc1500Key::F1 */ }
            if i.key_pressed(Key::F2) { /* TODO: Add PC-1500 instance to press Pc1500Key::F2 */ }
            if i.key_released(Key::F2) { /* TODO: Add PC-1500 instance to release Pc1500Key::F2 */ }
            if i.key_pressed(Key::F3) { /* TODO: Add PC-1500 instance to press Pc1500Key::F3 */ }
            if i.key_released(Key::F3) { /* TODO: Add PC-1500 instance to release Pc1500Key::F3 */ }
            if i.key_pressed(Key::F4) { /* TODO: Add PC-1500 instance to press Pc1500Key::F4 */ }
            if i.key_released(Key::F4) { /* TODO: Add PC-1500 instance to release Pc1500Key::F4 */ }
            if i.key_pressed(Key::F5) { /* TODO: Add PC-1500 instance to press Pc1500Key::F5 */ }
            if i.key_released(Key::F5) { /* TODO: Add PC-1500 instance to release Pc1500Key::F5 */ }
            if i.key_pressed(Key::F6) { /* TODO: Add PC-1500 instance to press Pc1500Key::F6 */ }
            if i.key_released(Key::F6) { /* TODO: Add PC-1500 instance to release Pc1500Key::F6 */ }

            // Arrow keys
            if i.key_pressed(Key::ArrowUp) { /* TODO: Add PC-1500 instance to press Pc1500Key::Up */ }
            if i.key_released(Key::ArrowUp) { /* TODO: Add PC-1500 instance to release Pc1500Key::Up */ }
            if i.key_pressed(Key::ArrowDown) { /* TODO: Add PC-1500 instance to press Pc1500Key::Down */ }
            if i.key_released(Key::ArrowDown) { /* TODO: Add PC-1500 instance to release Pc1500Key::Down */ }
            if i.key_pressed(Key::ArrowLeft) { /* TODO: Add PC-1500 instance to press Pc1500Key::Left */ }
            if i.key_released(Key::ArrowLeft) { /* TODO: Add PC-1500 instance to release Pc1500Key::Left */ }
            if i.key_pressed(Key::ArrowRight) { /* TODO: Add PC-1500 instance to press Pc1500Key::Right */ }
            if i.key_released(Key::ArrowRight) { /* TODO: Add PC-1500 instance to release Pc1500Key::Right */ }

            // Special keys
            if i.key_pressed(Key::Space) { /* TODO: Add PC-1500 instance to press Pc1500Key::Space */ }
            if i.key_released(Key::Space) { /* TODO: Add PC-1500 instance to release Pc1500Key::Space */ }
            if i.key_pressed(Key::Enter) { /* TODO: Add PC-1500 instance to press Pc1500Key::Enter */ }
            if i.key_released(Key::Enter) { /* TODO: Add PC-1500 instance to release Pc1500Key::Enter */ }
            if i.key_pressed(Key::Escape) { /* TODO: Add PC-1500 instance to press Pc1500Key::Off */ }
            if i.key_released(Key::Escape) { /* TODO: Add PC-1500 instance to release Pc1500Key::Off */ }
            if i.key_pressed(Key::Tab) { /* TODO: Add PC-1500 instance to press Pc1500Key::Mode */ }
            if i.key_released(Key::Tab) { /* TODO: Add PC-1500 instance to release Pc1500Key::Mode */ }
            if i.key_pressed(Key::Backspace) { /* TODO: Add PC-1500 instance to press Pc1500Key::Cl */ }
            if i.key_released(Key::Backspace) { /* TODO: Add PC-1500 instance to release Pc1500Key::Cl */ }

            // Math operators (using available egui keys)
            if i.key_pressed(Key::Plus) { /* TODO: Add PC-1500 instance to press Pc1500Key::Plus */ }
            if i.key_released(Key::Plus) { /* TODO: Add PC-1500 instance to release Pc1500Key::Plus */ }
            if i.key_pressed(Key::Minus) { /* TODO: Add PC-1500 instance to press Pc1500Key::Minus */ }
            if i.key_released(Key::Minus) { /* TODO: Add PC-1500 instance to release Pc1500Key::Minus */ }
            // Note: Asterisk (*) would need to be mapped to Shift+8 or another key combination
            if i.key_pressed(Key::Slash) { /* TODO: Add PC-1500 instance to press Pc1500Key::Slash */ }
            if i.key_released(Key::Slash) { /* TODO: Add PC-1500 instance to release Pc1500Key::Slash */ }
            if i.key_pressed(Key::Equals) { /* TODO: Add PC-1500 instance to press Pc1500Key::Equals */ }
            if i.key_released(Key::Equals) { /* TODO: Add PC-1500 instance to release Pc1500Key::Equals */ }
            if i.key_pressed(Key::Period) { /* TODO: Add PC-1500 instance to press Pc1500Key::Dot */ }
            if i.key_released(Key::Period) { /* TODO: Add PC-1500 instance to release Pc1500Key::Dot */ }

            // Additional PC-1500 specific keys (mapped to available PC keys)
            // You can customize these mappings as needed
        });
    }
    //Possible use!
    fn on_exit(&mut self) {}
}
