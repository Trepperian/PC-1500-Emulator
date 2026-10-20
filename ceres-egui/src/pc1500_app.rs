use crate::basic_io::{self, FormatChoice, PendingLoad, PendingState, StartupFile};
use crate::faceplate::{Faceplate, FaceplateInput};
use crate::host_keymap::{
    self, HoldPolicy, HostAction, HostKeymap, MOUSE_ID, TextEditRegistry,
};
use crate::lcd_view::{self, LcdFrame};
use crate::pacing::{self, Pacer, SpeedMode};
use ceres_core::{HostEvent, Key as Pc1500Key, KeyStroke, Pc1500, ShiftPolicy};
use eframe::egui;
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Notice shown while the machine is off (the `OFF` key or the ROM's automatic power-off): without
/// it, with the LCD blank, the emulator would look hung.
const POWERED_OFF_HINT: &str = "Off \u{2014} press ON (Esc)";

/// Runs one step of the emulation, catching a panic of the core instead of letting it close the
/// window: the CPU panics on an instruction the emulator does not know (`POINT` beyond the screen
/// makes the ROM run the undocumented `&98`, a machine-code program can jump into data...). It
/// returns the panic's message.
fn run_guarded(step: impl FnOnce()) -> Result<(), String> {
    panic::catch_unwind(AssertUnwindSafe(step)).map_err(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_else(|| "unknown error".to_string())
    })
}

/// Limits of the speed slider (multiplier of real hardware time). Above some x150-x400 (depending
/// on the host and on what the program does) the host rules, and the label says so.
const SPEED_MIN: f64 = 0.02;
const SPEED_MAX: f64 = 1000.0;

/// Quick speed buttons, next to the slider.
const SPEED_PRESETS: [f64; 9] = [0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 50.0, 100.0];

/// Pitch (distance between dots, in egui's logical points) of the large LCD (the "Large LCD"
/// checkbox), above the faceplate; if the window is narrower, the LCD shrinks. With 6, the matrix
/// of 156 columns is 936 points wide.
const LCD_PITCH: f32 = 6.0;

/// Bits of $764E (see `Pc1500::lcd_symbol_bytes`) of the prefixes that stay "latched" until the
/// next key: their key on the faceplate has an outline meanwhile.
const SYMBOL_SHIFT: u8 = 0x02;
const SYMBOL_SML: u8 = 0x08;
const SYMBOL_DEF: u8 = 0x80;

pub struct Pc1500App {
    // CORE EMULATOR - The real PC-1500 system
    emulator: Pc1500,

    /// What the LCD shows, copied from the emulator only once per update (see `update_emulator`)
    /// and painted by `lcd_view::show` at each repaint.
    lcd: LcdFrame,
    /// The "LCD ghosting" checkbox: paint the unlit dots and indicators barely visible, as they
    /// show on a real LCD.
    lcd_ghost: bool,
    /// The "Large LCD" checkbox: besides the faceplate's LCD, a big one above it (of pitch
    /// `LCD_PITCH`), to read it in small windows.
    big_lcd: bool,
    /// The PC-1500's faceplate (main view, see `faceplate`), with its procedural textures already
    /// generated.
    faceplate: Faceplate,

    // PC KEYBOARD → PC-1500. See `host_keymap`: here the events are only taken (`raw_input_hook`)
    // and passed to the emulator's key feeder, which presses and releases in emulated time.
    /// Translator of the host's keyboard events (with the "Match upper/lower case" checkbox inside,
    /// `exact_case`).
    host_keymap: HostKeymap,
    /// Keyboard events taken from egui in `raw_input_hook`, waiting to be translated in the next
    /// `update()`.
    host_events: Vec<egui::Event>,
    /// The "Capture the PC keyboard" checkbox (on by default).
    capture_keyboard: bool,
    /// Whether the last `raw_input_hook` was capturing: when it stops (checkbox, or focus on a text
    /// field) all the host's keys are released, as their releases will not arrive any more.
    was_capturing: bool,
    /// The GUI's text fields, so as not to capture the keyboard while one has the focus. ALL
    /// `TextEdit`s (file paths, present and future) must be registered here.
    text_edits: TextEditRegistry,
    /// The policy of holding keys in the last `update()` (see `host_keymap::hold_policy`): when it
    /// changes, all are released.
    hold_policy: HoldPolicy,
    /// The key of the faceplate the mouse is pressing right now.
    mouse_key: Option<Pc1500Key>,
    /// The "Keyboard" help window.
    show_key_help: bool,
    /// Last notice of the keyboard (characters with no key...).
    key_status: Option<String>,

    /// The "Auto power-off" checkbox (see `Pc1500::set_auto_power_off`). On by default, as on the
    /// real machine.
    auto_power_off: bool,

    // LH5 LOADER - UI state for loading .lh5 files at runtime
    lh5_path_input: String,
    lh5_status: Option<String>,

    // BASIC PROGRAM - see `basic_io` and `render_basic_io`.
    /// Path of the "BASIC program" field (to load and to save).
    basic_path_input: String,
    /// Format chosen in the drop-down.
    basic_format: FormatChoice,
    /// Program read, waiting for the ROM to reach the prompt (see `basic_io::PendingLoad`). It
    /// survives the Reset button: so "Reset and load" works without waiting by hand.
    pending_basic: Option<PendingLoad>,
    /// Last result of loading/saving.
    basic_status: Option<BasicStatus>,

    // CLOAD/CSAVE FROM THE PC-1500 - see `render_host_files` and `ceres_core::host_io`.
    /// The "CLOAD/CSAVE folder" field (at start, the current folder).
    host_dir_input: String,
    /// The "Tape" field: the file of `CLOAD`/`CSAVE` without a name.
    tape_input: String,
    /// Last event of the tape commands, or the result of changing the folder or the tape.
    host_status: Option<BasicStatus>,

    // TIMING / SPEED CONTROL - see the comments of `update()` and of `pacing`: the emulation goes
    // at the real PC-1500's pace (~1.3 MHz) multiplied by `speed_multiplier`, not at that of the
    // host's repaints.
    /// Speed multiplier: 1.0 = real hardware time, <1.0 slower (e.g. 0.1 = 10x slower, to watch
    /// calmly what a program does), >1.0 faster, up to `SPEED_MAX`.
    speed_multiplier: f64,
    /// The "Max" button: emulate as much as the host gives, without looking at `speed_multiplier`
    /// (see `SpeedMode::Max`).
    max_speed: bool,
    /// Real-time scheduler: how many cycles to emulate in each `update()` and the effective speed
    /// measured.
    pacer: Pacer,
    /// If paused, `update()` never moves the emulator on by itself — only `step_once` does it, when
    /// the step button is clicked.
    paused: bool,
    /// Why the emulation stopped, if the core panicked (see `run_guarded`): the machine is left as
    /// it was and nothing more is emulated until Reset or a `.lh5` is loaded.
    stopped: Option<String>,
    /// Set to `true` by the GUI's "Step" button: forces exactly one `update_emulator()` in the next
    /// `update()`, even while paused — to move on frame by frame during an analysis.
    step_once: bool,
}

impl Pc1500App {
    /// `startup`: the file of the command line, already sorted by its extension in `main.rs` (see
    /// `basic_io::StartupFile`): a `.lh5` is loaded at once, as always; a BASIC program goes in the
    /// queue and goes in as soon as the ROM finishes booting.
    pub fn new(_cc: &eframe::CreationContext<'_>, startup: Option<StartupFile>, pc1500a: bool) -> Self {
        let mut emulator = if pc1500a { Pc1500::new_pc1500a() } else { Pc1500::new() };
        // The core starts with the file I/O disabled (`ERROR 27`); in the GUI, `CLOAD`/`CSAVE` use
        // the current folder.
        let host_dir = std::env::current_dir().ok();
        let host_dir_input = host_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default();
        emulator.set_host_dir(host_dir);

        let lh5_status = if let Some(StartupFile::Lh5(path)) = &startup {
            match emulator.load_lh5_file(path) {
                Ok(()) => Some(format!("Loaded: {}", path.display())),
                Err(e) => Some(format!("Error: {e}")),
            }
        } else {
            None
        };

        let mut app = Self {
            emulator,
            lcd: LcdFrame::BLANK,
            lcd_ghost: true,
            big_lcd: false,
            faceplate: Faceplate::default(),
            host_keymap: HostKeymap::default(),
            host_events: Vec::new(),
            capture_keyboard: true,
            was_capturing: false,
            text_edits: TextEditRegistry::default(),
            hold_policy: HoldPolicy::Hold,
            mouse_key: None,
            show_key_help: false,
            key_status: None,
            auto_power_off: true,
            lh5_path_input: String::new(),
            lh5_status,
            basic_path_input: String::new(),
            basic_format: FormatChoice::Auto,
            pending_basic: None,
            basic_status: None,
            host_dir_input,
            tape_input: String::new(),
            host_status: None,
            speed_multiplier: 1.0,
            max_speed: false,
            pacer: Pacer::new(),
            paused: false,
            stopped: None,
            step_once: false,
        };
        if let Some(StartupFile::Basic(path)) = startup {
            app.basic_path_input = path.display().to_string();
            app.queue_basic_file(&path);
        }
        app
    }

    /// Loads a `.lh5` file into the emulator and updates the status message.
    pub fn load_lh5(&mut self, path: &Path) {
        self.lh5_status = match self.emulator.load_lh5_file(path) {
            Ok(()) => {
                // A new program, with the PC at its start: the emulation can go on.
                self.stopped = None;
                Some(format!("Loaded: {}", path.display()))
            }
            Err(e) => Some(format!("Error: {e}")),
        };
    }

    /// Restarts the emulator from a clean boot (the same as switching it on again) — the only way
    /// back to the ROM's normal interactive BASIC after loading and running a `.lh5` of native
    /// machine code, which leaves the CPU stopped (`HALT`) and never goes back to the ROM's loop.
    /// Before this, the only way was to close and reopen the whole application (`Pc1500::new()` was
    /// only called once, when `Pc1500App` started).
    ///
    /// It uses `Pc1500::reset()`, not a new `Pc1500::new()`: so what the real machine does not lose
    /// with a reset (the clock, with a time set with `TIME=`, which has its own battery) and the
    /// emulator's options survive, and so does the model (PC-1500 or PC-1500A). The complete list
    /// is in `Pc1500::reset`; the key feeder (host keys down, pasted text pending) does NOT
    /// survive. Here only the GUI's own state is cleared as well (the mouse's key, the `Pacer`'s
    /// pace and speed measurement). The chosen speed (slider, "Max") is kept.
    pub fn reset(&mut self) {
        self.emulator.reset();
        self.stopped = None;
        self.emulator.set_auto_power_off(self.auto_power_off);
        self.mouse_key = None;
        self.key_status = None;
        self.lh5_status = Some("Emulator restarted".to_string());
        self.pacer.pause();
    }

    /// Reads and interprets a BASIC program from `path` and puts it in the queue (see `basic_io`).
    /// Read or format errors show in the status without touching the machine.
    fn queue_basic_file(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => self.queue_basic_bytes(&bytes, path),
            Err(e) => self.basic_status = Some(BasicStatus::error(format!("Error reading {}: {e}", path.display()))),
        }
    }

    /// As `queue_basic_file` with the bytes already read (dropped files that only bring the bytes);
    /// `path` decides the extension.
    fn queue_basic_bytes(&mut self, bytes: &[u8], path: &Path) {
        let keywords = self.emulator.basic_keywords();
        match basic_io::decode(bytes, path, self.basic_format, &keywords) {
            Ok(decoded) => {
                self.basic_status = Some(BasicStatus::waiting(format!(
                    "Waiting for the BASIC prompt to load {}\u{2026}",
                    path.display()
                )));
                self.pending_basic = Some(PendingLoad::new(decoded, path.display().to_string()));
            }
            Err(e) => {
                self.pending_basic = None;
                self.basic_status = Some(BasicStatus::error(format!("Error in {}: {e}", path.display())));
            }
        }
    }

    /// Puts the queue's program into the machine if the ROM is already at the prompt (called after
    /// each step of the emulation).
    fn apply_pending_basic(&mut self) {
        let Some(pending) = &mut self.pending_basic else {
            return;
        };
        match pending.poll(self.emulator.is_at_basic_prompt(), self.emulator.ticks()) {
            PendingState::Waiting => {}
            PendingState::TimedOut => {
                self.pending_basic = None;
                self.basic_status = Some(BasicStatus::error(basic_io::NOT_AT_PROMPT_MESSAGE.to_owned()));
            }
            PendingState::Ready => {
                let Some(ready) = self.pending_basic.take() else {
                    return;
                };
                let decoded = ready.decoded();
                self.basic_status = Some(match self.emulator.load_basic_program(&decoded.program) {
                    Ok(mut report) => {
                        report.warnings.clone_from(&decoded.warnings);
                        let cold = self.emulator.lcd_text().trim_end() == "NEW0? :CHECK";
                        let text = basic_io::load_status(ready.source(), decoded.format, &report, cold);
                        if report.warnings.is_empty() { BasicStatus::ok(text) } else { BasicStatus::waiting(text) }
                    }
                    Err(e) => BasicStatus::error(format!("Error loading {}: {e}", ready.source())),
                });
            }
        }
    }

    /// Saves the machine's program to the field's path.
    fn save_basic(&mut self) {
        let path = PathBuf::from(self.basic_path_input.trim());
        if path.as_os_str().is_empty() {
            self.basic_status = Some(BasicStatus::error("Type the file's path".to_owned()));
            return;
        }
        self.basic_status = Some(match self.emulator.save_basic_file(&path, self.basic_format.save_format()) {
            Ok(report) => BasicStatus::ok(format!(
                "Saved {} ({}): {} lines, {} bytes",
                path.display(),
                basic_io::save_format_name(report.format),
                report.lines,
                report.bytes
            )),
            Err(e) => BasicStatus::error(format!("Error saving {}: {e}", path.display())),
        });
    }

    /// Files dropped on the window: a `.lh5` (or an unknown extension) is loaded as machine code, a
    /// BASIC program goes to the queue. It may not work on Wayland (winit does not always deliver
    /// the dropped files); the path field always works.
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            let path = file.path.clone().unwrap_or_else(|| PathBuf::from(&file.name));
            match (StartupFile::from_path(path.clone()), file.path, file.bytes) {
                (StartupFile::Lh5(_), Some(real), _) => {
                    self.lh5_path_input = real.display().to_string();
                    self.load_lh5(&real);
                }
                (StartupFile::Basic(_), Some(real), _) => {
                    self.basic_path_input = real.display().to_string();
                    self.queue_basic_file(&real);
                }
                (StartupFile::Basic(_), None, Some(bytes)) => self.queue_basic_bytes(&bytes, &path),
                _ => {
                    self.basic_status = Some(BasicStatus::error(format!("{} cannot be opened", path.display())));
                }
            }
        }
    }

    /// The "BASIC program" panel: path (registered so as not to capture the keyboard while it has
    /// the focus), format, Load/Save and status.
    fn render_basic_io(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("BASIC program:");
                let response = self.text_edits.add(
                    ui,
                    egui::TextEdit::singleline(&mut self.basic_path_input)
                        .hint_text("path/to/program.bas")
                        .desired_width(300.0),
                );
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                egui::ComboBox::from_id_salt("basic_format")
                    .selected_text(self.basic_format.label())
                    .show_ui(ui, |ui| {
                        for choice in FormatChoice::ALL {
                            ui.selectable_value(&mut self.basic_format, choice, choice.label());
                        }
                    })
                    .response
                    .on_hover_text(
                        "Auto: when loading, the contents are recognized (text, tokenized or CE-158 \
                         header); when saving, the extension rules: .bas CE-158 (as CSAVE), \
                         .asc CR text (CE-158X), .txt CRLF text, .bin tokenized without header",
                    );

                let load = ui
                    .button("Load BASIC")
                    .on_hover_text(
                        "Replaces the PC-1500's program (as NEW and typing it). It waits for the \
                         BASIC prompt if the machine is booting. The file can also be \
                         dropped on the window.",
                    )
                    .clicked();
                if load || enter {
                    let path = PathBuf::from(self.basic_path_input.trim());
                    self.queue_basic_file(&path);
                }
                if ui.button("Save BASIC").on_hover_text("Saves the PC-1500's program to the path").clicked() {
                    self.save_basic();
                }
                if self.pending_basic.is_some() && ui.button("Cancel load").clicked() {
                    self.pending_basic = None;
                    self.basic_status = None;
                }
            });
            if let Some(status) = &self.basic_status {
                ui.colored_label(status.color(), &status.text);
            }
        });
    }

    /// The "CLOAD/CSAVE" panel: the folder of the files of `CLOAD`, `CSAVE`, `MERGE` and `CHAIN`
    /// typed on the PC-1500, the "tape" file for those without a name and the result of the last
    /// one. Both fields are registered so as not to capture the keyboard while they have the focus.
    fn render_host_files(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("CLOAD/CSAVE folder:").on_hover_text(HOST_FILES_HELP);
                let dir_field = self.text_edits.add(
                    ui,
                    egui::TextEdit::singleline(&mut self.host_dir_input)
                        .hint_text("empty = disabled (ERROR 27)")
                        .desired_width(260.0),
                );
                let dir_enter = dir_field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Use").on_hover_text(HOST_FILES_HELP).clicked() || dir_enter {
                    self.apply_host_dir();
                }

                ui.separator();

                ui.label("Tape:").on_hover_text(
                    "File of CLOAD and CSAVE without a name (e.g. a bare CSAVE). Without a tape, \
                     they give ERROR 40.",
                );
                let tape_field = self.text_edits.add(
                    ui,
                    egui::TextEdit::singleline(&mut self.tape_input)
                        .hint_text("path/to/tape.bas")
                        .desired_width(200.0),
                );
                let tape_enter = tape_field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Use tape").clicked() || tape_enter {
                    self.apply_tape();
                }
            });
            if let Some(status) = &self.host_status {
                ui.colored_label(status.color(), &status.text);
            }
        });
    }

    /// The folder's "Use" button (see `parse_host_dir`).
    fn apply_host_dir(&mut self) {
        self.host_status = Some(match parse_host_dir(&self.host_dir_input) {
            Ok(Some(dir)) => {
                let text = format!("CLOAD/CSAVE in {}", dir.display());
                self.emulator.set_host_dir(Some(dir));
                BasicStatus::ok(text)
            }
            Ok(None) => {
                self.emulator.set_host_dir(None);
                BasicStatus::waiting("CLOAD/CSAVE disabled (they give ERROR 27)".to_owned())
            }
            Err(e) => BasicStatus::error(e),
        });
    }

    /// The "Use tape" button.
    fn apply_tape(&mut self) {
        let tape = parse_tape(&self.tape_input);
        self.host_status = Some(tape.as_ref().map_or_else(
            || BasicStatus::waiting("No tape: CLOAD/CSAVE without a name give ERROR 40".to_owned()),
            |path| BasicStatus::ok(format!("Tape: {}", path.display())),
        ));
        self.emulator.set_tape(tape);
    }

    /// The speed asked for in the GUI.
    const fn speed_mode(&self) -> SpeedMode {
        if self.max_speed {
            SpeedMode::Max
        } else {
            SpeedMode::Multiplier(self.speed_multiplier)
        }
    }

    /// Moves the emulation on as much as is due in this `update()` (see `pacing::Pacer`), or
    /// exactly one frame if "Step" was clicked. It returns `true` if the emulation is running (not
    /// paused).
    fn advance_emulation(&mut self, ctx: &egui::Context) -> bool {
        if self.stopped.is_some() {
            self.step_once = false;
            self.pacer.pause();
            return false;
        }
        if self.step_once {
            self.step_once = false;
            let emulator = &mut self.emulator;
            if let Err(why) = run_guarded(|| emulator.step_frame()) {
                self.stopped = Some(why);
            }
            self.pacer.pause();
            return false;
        }
        if self.paused {
            self.pacer.pause();
            return false;
        }

        let now = Instant::now();
        let budget = pacing::update_budget(ctx.input(|i| i.predicted_dt), self.max_speed);
        let deadline = now + budget;
        let mode = self.speed_mode();
        let emulator = &mut self.emulator;
        let pacer = &mut self.pacer;
        let result = run_guarded(|| {
            pacer.advance(now, mode, || Instant::now() >= deadline, |ticks| emulator.run_for_ticks(ticks));
        });
        if let Err(why) = result {
            self.stopped = Some(why);
            self.pacer.pause();
            return false;
        }
        true
    }

    /// The LCD's checkboxes ("LCD ghosting", "Large LCD") and the notice of the machine being off,
    /// in the control panel. The LCD itself goes in the faceplate (see `render_faceplate`).
    fn render_display_options(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.lcd_ghost, "LCD ghosting").on_hover_text(
            "Shows the unlit dots and indicators barely visible, as on a real LCD",
        );
        ui.checkbox(&mut self.big_lcd, "Large LCD")
            .on_hover_text("Also shows the LCD large, above the faceplate");
        if !self.emulator.is_powered_on() {
            ui.colored_label(egui::Color32::RED, egui::RichText::new(POWERED_OFF_HINT).strong());
        }
        if let Some(why) = &self.stopped {
            ui.colored_label(
                egui::Color32::RED,
                egui::RichText::new(format!("Emulation stopped ({why}): press Reset")).strong(),
            )
            .on_hover_text(
                "The CPU met an instruction the emulator cannot run. The machine is left as it was; \
                 Reset switches it on again, or load a .lh5.",
            );
        }
    }

    /// Passes to the emulator what was typed on the PC since the last `update()` (see
    /// `host_keymap`), before emulating anything.
    fn handle_host_keyboard(&mut self) {
        // On crossing 1x, how keys are held changes (see `HoldPolicy`): whatever was down is taken
        // as released, so that none gets stuck waiting for a release that in `Tap` mode is ignored.
        let policy = host_keymap::hold_policy(self.max_speed, self.speed_multiplier);
        if policy != self.hold_policy {
            self.emulator.release_all_host_keys();
            self.hold_policy = policy;
        }
        let events = mem::take(&mut self.host_events);
        for action in self.host_keymap.translate(&events) {
            self.apply_host_action(&action);
        }
    }

    fn apply_host_action(&mut self, action: &HostAction) {
        if let Some(status) = host_keymap::apply(&mut self.emulator, action, self.hold_policy) {
            self.key_status = Some(status);
        }
    }

    /// The keyboard's checkboxes, the help and the state of the typing, in the control panel.
    fn render_keyboard_options(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.capture_keyboard, "Capture the PC keyboard").on_hover_text(
                "What is typed on the PC goes to the PC-1500 (except with the focus on a text \
                 field). Without this, only the faceplate's keys.",
            );
            ui.checkbox(self.host_keymap.exact_case_mut(), "Match upper/lower case")
                .on_hover_text(
                    "Off: each letter is the PC-1500 key as it is (UPPERCASE, as BASIC's \
                     keywords want; lowercase with SML = F8 or SHIFT = F11). \
                     On: the letters come out as on the PC.",
                );
            if ui.button("\u{2328} Keyboard").on_hover_text("Which PC key is each PC-1500 key").clicked() {
                self.show_key_help = !self.show_key_help;
            }
            ui.separator();
            self.render_display_options(ui);
            let pending = self.emulator.pending_keystrokes();
            if pending > 0 {
                ui.separator();
                ui.label(format!("Typing\u{2026} {pending} pending"));
                if ui.button("Cancel typing").clicked() {
                    self.emulator.cancel_typing();
                }
            }
            if let Some(status) = &self.key_status {
                ui.colored_label(egui::Color32::from_rgb(0xE0, 0x80, 0x10), status);
            }
        });
    }

    /// The faceplate (see `faceplate`): LCD and keys, taking all the room left. A key shows pressed
    /// while it is down in the emulated keyboard (with the mouse or with the PC's keyboard, see
    /// `Pc1500::is_key_pressed`) or while the mouse is pressing it, even if the feeder has not
    /// passed it to the matrix yet (at low speed it takes a little in host time). Latched prefixes
    /// (`SHIFT`, `SML`, `DEF`, bits of $764E) have an outline until the next key.
    fn render_faceplate(&mut self, ui: &mut egui::Ui) {
        let symbols = self.emulator.lcd_symbol_bytes()[0];
        let emulator = &self.emulator;
        let mouse_key = self.mouse_key;
        let held = Pc1500Key::ALL.map(|k| emulator.is_key_pressed(k) || mouse_key == Some(k));
        let latched_keys = Pc1500Key::ALL.map(|k| latched(k, symbols));
        let input = FaceplateInput {
            lcd: &self.lcd,
            ghost: self.lcd_ghost,
            powered_off: (!emulator.is_powered_on()).then_some(POWERED_OFF_HINT),
            held: &held,
            latched: &latched_keys,
            hint: &host_keymap::hint,
        };
        let output = self.faceplate.show(ui, &input);
        self.update_mouse_key(output.pointer_key);
    }

    /// The mouse on the faceplate's keys: pressing a key holds it down on the PC-1500 until the
    /// button is released (even if the pointer leaves the key, as with
    /// `is_pointer_button_down_on`), as a PC key, through the same feeder (that is how the arrows'
    /// key repeat and the games with `INKEY$` work). Only the edges are detected here: how long
    /// each key press lasts at least is decided by the feeder in emulated time (or by the `Tap`
    /// policy above 1x), not by a host time.
    fn update_mouse_key(&mut self, pointer_on: Option<Pc1500Key>) {
        if pointer_on == self.mouse_key {
            return;
        }
        if let Some(old) = self.mouse_key.take() {
            let action = if old == Pc1500Key::On {
                HostAction::Brk { id: MOUSE_ID, down: false }
            } else {
                HostAction::Up { id: MOUSE_ID }
            };
            self.apply_host_action(&action);
        }
        if let Some(new) = pointer_on {
            let action = if new == Pc1500Key::On {
                HostAction::Brk { id: MOUSE_ID, down: true }
            } else {
                HostAction::Down { id: MOUSE_ID, stroke: KeyStroke::plain(new), policy: ShiftPolicy::AddIfNeeded }
            };
            self.apply_host_action(&action);
            self.mouse_key = Some(new);
        }
    }

    /// The "PC keyboard → PC-1500" window: the tables of `host_keymap` as they are, so that the
    /// help cannot tell something different from what happens.
    fn render_key_help(&mut self, ctx: &egui::Context) {
        let mut open = self.show_key_help;
        egui::Window::new("PC keyboard \u{2192} PC-1500")
            .open(&mut open)
            .default_width(560.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                    ui.label(KEY_HELP_NOTES);
                    ui.separator();
                    ui.strong("Special keys");
                    egui::Grid::new("key_help_special").striped(true).show(ui, |ui| {
                        ui.strong("PC");
                        ui.strong("PC-1500");
                        ui.strong("");
                        ui.end_row();
                        for special in host_keymap::SPECIAL {
                            ui.label(special.host_label);
                            ui.label(host_keymap::action_label(special.action));
                            ui.label(special.note);
                            ui.end_row();
                        }
                        for (host, note) in host_keymap::EXTRA_HELP {
                            ui.label(*host);
                            ui.label("");
                            ui.label(*note);
                            ui.end_row();
                        }
                    });
                    ui.separator();
                    ui.strong("Characters (with any PC keyboard layout)");
                    egui::Grid::new("key_help_chars").striped(true).show(ui, |ui| {
                        for (index, (host, sequence)) in host_keymap::char_table().iter().enumerate() {
                            ui.monospace(host);
                            match sequence {
                                Some(sequence) => ui.label(sequence),
                                None => ui.weak("no key"),
                            };
                            if index % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
                });
            });
        self.show_key_help = open;
    }

    /// Speed control: pause, step by step, a logarithmic slider of the multiplier of real hardware
    /// time (`1.00x`) from `SPEED_MIN` to `SPEED_MAX`, buttons of usual speeds, the "Max" button
    /// (as much as the host gives) and the effective speed measured, which warns when the host
    /// cannot keep up with the one asked for. See `pacing`.
    fn render_speed_control(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                let pause_label = if self.paused {
                    "\u{25b6} Resume"
                } else {
                    "\u{23f8} Pause"
                };
                if ui.button(pause_label).clicked() {
                    self.paused = !self.paused;
                    // While paused, no repaints are requested (see `update`): let the next one come
                    // now, without waiting for another event.
                    ui.ctx().request_repaint();
                }

                if ui
                    .add_enabled(self.paused, egui::Button::new("\u{23ed} Step"))
                    .on_hover_text("Move on exactly one emulated frame (~11.5 ms of the PC-1500)")
                    .clicked()
                {
                    self.step_once = true;
                    ui.ctx().request_repaint();
                }

                ui.separator();

                ui.label("Speed:");
                // Without the slider's numeric field (`show_value(false)`): when clicked, egui
                // turns it into a text field that is not in `text_edits`, and `raw_input_hook`
                // would take its keys to send them to the PC-1500. The value is shown apart as a
                // label.
                ui.add_enabled(
                    !self.max_speed,
                    egui::Slider::new(&mut self.speed_multiplier, SPEED_MIN..=SPEED_MAX)
                        .logarithmic(true)
                        .show_value(false),
                );
                ui.label(format!("{}x", pacing::format_multiplier(self.speed_multiplier)));
                for preset in SPEED_PRESETS {
                    let selected = !self.max_speed && (self.speed_multiplier - preset).abs() < 1e-9;
                    let label = format!("{}x", format_preset(preset));
                    if ui.selectable_label(selected, label).clicked() {
                        self.speed_multiplier = preset;
                        self.max_speed = false;
                    }
                }
                ui.toggle_value(&mut self.max_speed, "\u{23e9} Max")
                    .on_hover_text(
                        "Emulates as much as the computer gives, without a speed limit \
                         (the GUI may be somewhat less smooth)",
                    );

                ui.separator();
                self.render_measured_speed(ui);

                ui.separator();

                if ui
                    .checkbox(&mut self.auto_power_off, "Auto power-off")
                    .on_hover_text(
                        "As the real PC-1500: it switches itself off after some 5 minutes without any \
                         key pressed (sooner if the speed is above 1x). ON (Esc) switches it \
                         on again where it was.",
                    )
                    .changed()
                {
                    self.emulator.set_auto_power_off(self.auto_power_off);
                }
            });
        });
    }

    /// The effective speed measured by the `Pacer` (see `speed_status_text`), in orange if the host
    /// limits it.
    fn render_measured_speed(&self, ui: &mut egui::Ui) {
        let measurement = self.pacer.measurement();
        let status = speed_status_text(self.paused, self.max_speed, measurement);
        ui.label(status.measured).on_hover_text(
            "Real speed achieved (measured every 0.5 s): multiple of the PC-1500's real \
             time and emulated CPU cycles per second (the real one runs at 1.3 MHz)",
        );
        if let Some(warning) = status.warning {
            ui.colored_label(egui::Color32::from_rgb(0xE0, 0x80, 0x10), warning)
                .on_hover_text(
                    "The computer cannot emulate at the speed asked for: it runs at the one \
                     shown",
                );
        }
    }

    fn render_lh5_loader(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(".lh5 file:");

                // Registered: while it has the focus, the keyboard does not go to the PC-1500 (see
                // `TextEditRegistry`).
                let response = self.text_edits.add(
                    ui,
                    egui::TextEdit::singleline(&mut self.lh5_path_input)
                        .hint_text("path/to/program.lh5")
                        .desired_width(300.0),
                );

                // Allow pressing Enter inside the text field to trigger loading
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let path = std::path::PathBuf::from(self.lh5_path_input.clone());
                    self.load_lh5(&path);
                }

                if ui.button("Load").clicked() {
                    let path = std::path::PathBuf::from(self.lh5_path_input.clone());
                    self.load_lh5(&path);
                }

                if ui
                    .button("Reset")
                    .on_hover_text("Restarts the emulator from a clean boot (back to the ROM's normal BASIC)")
                    .clicked()
                {
                    self.reset();
                }

                ui.separator();

                // Time of the µPD1990AC clock, the same that `TIME` would read.
                ui.label(format!("Clock: {}", self.emulator.rtc_now()))
                    .on_hover_text(
                        "The time TIME reads (day/month time). It starts at the computer's local \
                         time and moves on in real time, at any emulation speed, \
                         as the PC-1500's battery-backed clock (WAIT and BEEP do follow the \
                         speed). TIME=MMDDHH.MMSS changes it; it is kept by Reset, not when \
                         closing the emulator.",
                    );
            });

            if let Some(status) = &self.lh5_status {
                let color = if status.starts_with("Error") {
                    egui::Color32::RED
                } else {
                    egui::Color32::GREEN
                };
                ui.colored_label(color, status);
            }
        });
    }
}

impl eframe::App for Pc1500App {
    /// Takes the keyboard events before egui processes them (see `host_keymap::capture_events`),
    /// unless the capture is disabled or a registered text field has the focus.
    ///
    /// It runs before `update()` (eframe calls it before starting egui's pass): the focus looked at
    /// is that of the previous repaint, as are the registered fields.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let focused = ctx.memory(egui::Memory::focused);
        let capturing = self.capture_keyboard && self.text_edits.allows_capture(focused);
        if capturing {
            self.host_events
                .extend(host_keymap::capture_events(&mut raw_input.events));
        }
        if self.was_capturing && !capturing {
            // The releases of whatever was still down will not arrive any more.
            self.host_events.push(egui::Event::WindowFocused(false));
        }
        self.was_capturing = capturing;
    }

    /// Emulates what is due and paints the GUI.
    ///
    /// # History of the pacing
    /// At first this did one `step_frame()` per repaint, without looking at the clock: the speed
    /// was tied to the monitor's vsync. Then it was paced with `FRAME_DURATION`, but at most ONE
    /// emulated frame (~11.5 ms) per repaint (~16.7 ms at 60 Hz), so the emulation did not go
    /// beyond ~0.69x however much was asked for. Now the `pacing::Pacer` emulates in each
    /// `update()` the cycles due for the real time elapsed (from ~433 at 0.02x to millions in Max
    /// mode), with a budget of host time so that the GUI stays responsive.
    ///
    /// While it runs, the next repaint is always requested (`request_repaint`): the vsync already
    /// limits it to the monitor's frequency, and so even at 0.02x the emulation moves on a little
    /// at each frame of the GUI instead of in jumps. While paused it is not needed: egui repaints
    /// by itself when there are events.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The PC's keyboard first (in emulated time the feeder presses it, inside `run_for_ticks`).
        self.handle_host_keyboard();
        self.handle_dropped_files(ctx);
        // The text fields are registered again when they are drawn.
        self.text_edits.clear();

        if self.advance_emulation(ctx) {
            ctx.request_repaint();
        }
        // The pending BASIC load, as soon as the ROM reaches the prompt.
        self.apply_pending_basic();
        // What CLOAD/CSAVE have done on the PC-1500: the last one shows.
        if let Some(event) = self.emulator.take_host_events().last() {
            self.host_status = Some(host_event_status(event));
        }

        // A single snapshot of the LCD per update (see `Pc1500::display`), painted later by
        // `lcd_view::show`.
        self.lcd = LcdFrame::from_display(self.emulator.display());

        // At the top, the control panel (speed, loaders, the keyboard's and the LCD's checkboxes);
        // the rest of the window is for the faceplate, which is scaled to fill it (see
        // `faceplate`). The panels of BASIC files and of CLOAD/CSAVE can be folded to leave it more
        // room.
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.add_space(2.0);
            // Speed control (pause/step/multiplier)
            self.render_speed_control(ui);
            // .lh5 loader, Reset and clock
            self.render_lh5_loader(ui);
            egui::CollapsingHeader::new("BASIC programs and CLOAD/CSAVE")
                .id_salt("files_panel")
                .default_open(true)
                .show(ui, |ui| {
                    self.render_basic_io(ui);
                    self.render_host_files(ui);
                });
            self.render_keyboard_options(ui);
            ui.add_space(2.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.big_lcd {
                lcd_view::show(ui, &self.lcd, self.lcd_ghost, LCD_PITCH);
                ui.add_space(4.0);
            }
            self.render_faceplate(ui);
        });

        self.render_key_help(ctx);
    }
}

/// Status of the "BASIC program" panel.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BasicStatus {
    kind: StatusKind,
    text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusKind {
    Ok,
    /// Waiting, or done but with warnings.
    Attention,
    Error,
}

impl BasicStatus {
    const fn ok(text: String) -> Self {
        Self { kind: StatusKind::Ok, text }
    }

    const fn waiting(text: String) -> Self {
        Self { kind: StatusKind::Attention, text }
    }

    const fn error(text: String) -> Self {
        Self { kind: StatusKind::Error, text }
    }

    const fn color(&self) -> egui::Color32 {
        match self.kind {
            StatusKind::Ok => egui::Color32::GREEN,
            StatusKind::Attention => egui::Color32::from_rgb(0xE0, 0x80, 0x10),
            StatusKind::Error => egui::Color32::RED,
        }
    }
}

/// Help of the "CLOAD/CSAVE" panel.
const HOST_FILES_HELP: &str = "\
Folder of the files of CLOAD, CSAVE, MERGE and CHAIN typed on the PC-1500 \
(as with a CE-158). CSAVE \"X\" saves X.BAS tokenized with a CE-158 header; \
CSAVEa \"X\" saves X.ASC as text; with the extension .bin it is saved tokenized without \
header and with .txt as text. CLOAD \"X\" recognizes the format by itself (it looks for X.BAS, \
X.ASC, X.TXT, X.BIN ignoring case); CLOAD? \"X\" compares. CLOAD and \
MERGE only in direct mode; CHAIN \"X\"[,line] inside a program. \
Empty = disabled (ERROR 27).";

/// What the folder of the "CLOAD/CSAVE" panel says: empty, disable (`Ok(None)`); a folder that
/// exists, use it; anything else, an error to show without changing anything.
fn parse_host_dir(input: &str) -> Result<Option<PathBuf>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let dir = PathBuf::from(trimmed);
    if dir.is_dir() {
        Ok(Some(dir))
    } else {
        Err(format!("Not a folder: {}", dir.display()))
    }
}

/// The tape file of the "Tape" field (empty = no tape). It need not exist yet: `CSAVE` creates it.
fn parse_tape(input: &str) -> Option<PathBuf> {
    let trimmed = input.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// The status line of an event of `CLOAD`/`CSAVE` (its text, from `HostEvent`): red if the PC-1500
/// gave an `ERROR`, orange if it loaded with warnings, green otherwise.
fn host_event_status(event: &HostEvent) -> BasicStatus {
    let text = event.to_string();
    match event {
        HostEvent::Error { .. } => BasicStatus::error(text),
        HostEvent::Loaded { warnings, .. } if !warnings.is_empty() => BasicStatus::waiting(text),
        _ => BasicStatus::ok(text),
    }
}

/// Notes of the keyboard's help window.
const KEY_HELP_NOTES: &str = "\
\u{b7} SHIFT and DEF are prefixes, as on the real machine: they are pressed and released \
(F11 / F7) and affect the next key. The PC's Shift only chooses which character \
is typed: \"(\" or \"?\" come out by themselves, with whatever PC-1500 SHIFT is needed.\n\
\u{b7} Letters come out in uppercase. Lowercase: SML (F8), SHIFT (F11) + letter, or the \
\"Match upper/lower case\" checkbox.\n\
\u{b7} Ctrl+V types the clipboard (slowly: the ROM needs some 8 emulated frames, \
~92 ms at 1x, per key); \"Cancel typing\" or ON/BRK stop it.\n\
\u{b7} Up to 1x a key held is held on the PC-1500 (the arrows repeat by themselves); \
above 1x each key press (and each repeat of the PC) is a short tap.\n\
\u{b7} While a text field has the focus, the keyboard is for that field.";

/// `true` if `key` is a prefix latched right now (bits of $764E).
const fn latched(key: Pc1500Key, symbols: u8) -> bool {
    let bit = match key {
        Pc1500Key::Shift => SYMBOL_SHIFT,
        Pc1500Key::Sml => SYMBOL_SML,
        Pc1500Key::Control => SYMBOL_DEF,
        _ => return false,
    };
    symbols & bit != 0
}

/// Text of a speed button: `0.1`, `0.25`, `1`, `50`...
fn format_preset(preset: f64) -> String {
    let text = format!("{preset:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// What the GUI shows about the effective speed.
#[derive(Debug, PartialEq, Eq)]
struct SpeedStatus {
    /// Always visible: speed measured, or why there is none.
    measured: String,
    /// Warning in orange, if any.
    warning: Option<&'static str>,
}

/// See `SpeedStatus`. While paused or before the first measurement there is no speed to show; in
/// Max mode there is no warning that the host limits it, because that is exactly what was asked
/// for.
fn speed_status_text(
    paused: bool,
    max_speed: bool,
    measurement: Option<pacing::Measurement>,
) -> SpeedStatus {
    if paused {
        return SpeedStatus { measured: "Real: paused".to_owned(), warning: None };
    }
    let Some(m) = measurement else {
        return SpeedStatus { measured: "Real: measuring\u{2026}".to_owned(), warning: None };
    };
    let measured = format!(
        "Real: x{} \u{b7} {:.2} MHz",
        pacing::format_multiplier(m.multiplier),
        m.emulated_mhz
    );
    let warning = if max_speed {
        Some("turbo: as much as the host gives")
    } else if m.host_limited {
        Some("limited by the host")
    } else {
        None
    };
    SpeedStatus { measured, warning }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An instruction the emulator does not know stops the emulation with its message instead of
    /// closing the window: `&98` (what `POINT` beyond the screen makes the ROM run) at `&0100`.
    #[test]
    fn an_unknown_instruction_stops_the_emulation_without_closing_the_window() {
        let path = std::env::temp_dir().join(format!("ceres-unknown-opcode-{}.lh5", std::process::id()));
        std::fs::write(&path, [0x00, 0x01, 0x01, 0x00, 0x98]).unwrap_or_else(|e| panic!("{e}"));
        let mut pc = Pc1500::new();
        pc.load_lh5_file(&path).unwrap_or_else(|e| panic!("{e}"));
        let _ = std::fs::remove_file(&path);
        let result = run_guarded(|| {
            pc.step_frame();
        });
        let why = result.expect_err("the CPU does not know &98");
        assert!(why.contains("0x98") && why.contains("0x0100"), "{why}");
        assert_eq!(run_guarded(|| {}), Ok(()), "a step that does not panic");
    }

    #[test]
    fn latched_prefixes_light_their_key() {
        assert!(latched(Pc1500Key::Shift, SYMBOL_SHIFT));
        assert!(latched(Pc1500Key::Sml, SYMBOL_SML | SYMBOL_SHIFT));
        assert!(latched(Pc1500Key::Control, SYMBOL_DEF));
        assert!(!latched(Pc1500Key::Shift, SYMBOL_SML));
        assert!(!latched(Pc1500Key::A, 0xFF));
    }

    #[test]
    fn host_dir_field() {
        assert_eq!(parse_host_dir("  "), Ok(None), "empty disables");
        let dir = std::env::temp_dir();
        assert_eq!(parse_host_dir(&format!(" {} ", dir.display())), Ok(Some(dir.clone())), "folder");
        let missing = dir.join("ceres-does-not-exist-\u{f1}");
        assert!(parse_host_dir(&missing.display().to_string()).is_err(), "it does not exist");
        assert_eq!(parse_tape(" "), None, "no tape");
        assert_eq!(parse_tape(" a/b.bas "), Some(PathBuf::from("a/b.bas")), "tape");
    }

    #[test]
    fn host_event_colors() {
        let error = HostEvent::Error { command: ceres_core::HostCommand::Cload, code: 61, message: "it does not exist".to_owned() };
        assert_eq!(host_event_status(&error).kind, StatusKind::Error, "error");
        let verified = HostEvent::Verified { path: PathBuf::from("x"), bytes: 1 };
        let status = host_event_status(&verified);
        assert_eq!(status.kind, StatusKind::Ok, "verified");
        assert_eq!(status.text, "CLOAD? x: coincide (1 bytes)", "text of the event");
    }

    #[test]
    fn preset_labels() {
        let labels: Vec<String> = SPEED_PRESETS.iter().map(|&p| format_preset(p)).collect();
        assert_eq!(labels, ["0.1", "0.25", "0.5", "1", "2", "5", "10", "50", "100"]);
        assert!(SPEED_PRESETS.iter().all(|p| (SPEED_MIN..=SPEED_MAX).contains(p)));
    }

    #[test]
    fn speed_status() {
        let normal = pacing::Measurement { multiplier: 1.0, emulated_mhz: 1.3, host_limited: false };
        let limited = pacing::Measurement { multiplier: 152.4, emulated_mhz: 198.12, host_limited: true };

        let paused = speed_status_text(true, false, Some(normal));
        assert_eq!(paused.measured, "Real: paused");
        assert_eq!(paused.warning, None);
        assert_eq!(speed_status_text(false, false, None).warning, None);

        let ok = speed_status_text(false, false, Some(normal));
        assert_eq!(ok.measured, "Real: x1.00 \u{b7} 1.30 MHz");
        assert_eq!(ok.warning, None);

        let slow_host = speed_status_text(false, false, Some(limited));
        assert_eq!(slow_host.measured, "Real: x152 \u{b7} 198.12 MHz");
        assert_eq!(slow_host.warning, Some("limited by the host"));

        let turbo = speed_status_text(false, true, Some(limited));
        assert_eq!(turbo.warning, Some("turbo: as much as the host gives"));
    }
}
