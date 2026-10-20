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
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Aviso que se muestra mientras la máquina está apagada (tecla `OFF` o
/// auto-apagado de la ROM): sin él parecería que el emulador se ha
/// colgado, con el LCD en blanco.
const POWERED_OFF_HINT: &str = "Apagada \u{2014} pulsa ON (Esc)";

/// Límites del slider de velocidad (multiplicador respecto al tiempo
/// real de hardware). Por encima de unos x150-x400 (según el host y lo
/// que haga el programa) manda el host, y la etiqueta lo dice.
const SPEED_MIN: f64 = 0.02;
const SPEED_MAX: f64 = 1000.0;

/// Botones de velocidad rápida, junto al slider.
const SPEED_PRESETS: [f64; 9] = [0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 50.0, 100.0];

/// Paso (distancia entre puntos, en puntos lógicos de egui) del LCD
/// ampliado (casilla "LCD ampliado"), encima de la carátula; si la
/// ventana es más estrecha, el LCD se encoge. Con 6 la matriz de 156
/// columnas mide 936 puntos de ancho.
const LCD_PITCH: f32 = 6.0;

/// Bits de $764E (ver `Pc1500::lcd_symbol_bytes`) de los prefijos que se
/// quedan "enganchados" hasta la siguiente tecla: su tecla de la
/// carátula lleva un contorno mientras tanto.
const SYMBOL_SHIFT: u8 = 0x02;
const SYMBOL_SML: u8 = 0x08;
const SYMBOL_DEF: u8 = 0x80;

pub struct Pc1500App {
    // CORE EMULATOR - The real PC-1500 system
    emulator: Pc1500,

    /// Lo que muestra el LCD, copiado del emulador una sola vez por
    /// actualización (ver `update_emulator`) y pintado por
    /// `lcd_view::show` en cada repintado.
    lcd: LcdFrame,
    /// Casilla "Fantasma LCD": pintar apenas visibles los puntos e
    /// indicadores apagados, como se ven en un LCD real.
    lcd_ghost: bool,
    /// Casilla "LCD ampliado": además del LCD de la carátula, uno grande
    /// encima (de paso `LCD_PITCH`), para leerlo en ventanas pequeñas.
    big_lcd: bool,
    /// La carátula de la PC-1500 (vista principal, ver `faceplate`), con
    /// sus texturas procedurales ya generadas.
    faceplate: Faceplate,

    // TECLADO DEL PC → PC-1500. Ver `host_keymap`: aquí solo se roban los
    // eventos (`raw_input_hook`) y se pasan al alimentador de teclas del
    // emulador, que pulsa y suelta en tiempo emulado.
    /// Traductor de eventos de teclado del host (con la casilla
    /// "Distinguir mayúsculas/minúsculas" dentro, `exact_case`).
    host_keymap: HostKeymap,
    /// Eventos de teclado robados a egui en `raw_input_hook`, pendientes
    /// de traducir en la siguiente `update()`.
    host_events: Vec<egui::Event>,
    /// Casilla "Capturar teclado del PC" (activada por defecto).
    capture_keyboard: bool,
    /// Si en el último `raw_input_hook` se estaba capturando: al dejar de
    /// hacerlo (casilla, o foco en un campo de texto) se sueltan todas las
    /// teclas del host, cuyas sueltas ya no llegarán.
    was_capturing: bool,
    /// Los campos de texto de la interfaz, para no capturar el teclado
    /// mientras uno tiene el foco. TODOS los `TextEdit` (rutas de
    /// archivos, presentes y futuros) deben registrarse aquí.
    text_edits: TextEditRegistry,
    /// La política de mantener teclas en la última `update()` (ver
    /// `host_keymap::hold_policy`): al cambiar se sueltan todas.
    hold_policy: HoldPolicy,
    /// La tecla de la carátula que tiene pulsada el ratón ahora mismo.
    mouse_key: Option<Pc1500Key>,
    /// Ventana de ayuda "Teclado".
    show_key_help: bool,
    /// Último aviso del teclado (caracteres sin tecla...).
    key_status: Option<String>,

    /// Casilla "Auto-apagado" (ver `Pc1500::set_auto_power_off`).
    /// Activado por defecto, como en la máquina real.
    auto_power_off: bool,

    // LH5 LOADER - UI state for loading .lh5 files at runtime
    lh5_path_input: String,
    lh5_status: Option<String>,

    // PROGRAMA BASIC - ver `basic_io` y `render_basic_io`.
    /// Ruta del campo "Programa BASIC" (para cargar y para guardar).
    basic_path_input: String,
    /// Formato elegido en el desplegable.
    basic_format: FormatChoice,
    /// Programa leído esperando a que la ROM llegue al prompt (ver
    /// `basic_io::PendingLoad`). Sobrevive al botón Reset: así "Reset y
    /// cargar" funciona sin esperar a mano.
    pending_basic: Option<PendingLoad>,
    /// Último resultado de cargar/guardar.
    basic_status: Option<BasicStatus>,

    // CLOAD/CSAVE DESDE LA PC-1500 - ver `render_host_files` y
    // `ceres_core::host_io`.
    /// Campo "Carpeta CLOAD/CSAVE" (al arrancar, la carpeta actual).
    host_dir_input: String,
    /// Campo "Cinta": el archivo de `CLOAD`/`CSAVE` sin nombre.
    tape_input: String,
    /// Último evento de los comandos de cinta, o el resultado de cambiar
    /// la carpeta o la cinta.
    host_status: Option<BasicStatus>,

    // TIMING / SPEED CONTROL - ver el comentario de `update()` y de
    // `pacing`: la emulación va al ritmo real de la PC-1500 (~1.3 MHz)
    // multiplicado por `speed_multiplier`, no al del repintado del host.
    /// Multiplicador de velocidad: 1.0 = tiempo real de hardware, <1.0
    /// más lento (p.ej. 0.1 = 10x más lento, para poder observar con
    /// calma qué hace un programa), >1.0 más rápido, hasta `SPEED_MAX`.
    speed_multiplier: f64,
    /// Botón "Máx": emular todo lo que dé el host, sin mirar
    /// `speed_multiplier` (ver `SpeedMode::Max`).
    max_speed: bool,
    /// Planificador de tiempo real: cuántos ciclos emular en cada
    /// `update()` y la velocidad efectiva medida.
    pacer: Pacer,
    /// Si está en pausa, `update()` nunca avanza el emulador por sí solo
    /// — solo lo hace `step_once` al pulsar el botón de paso.
    paused: bool,
    /// Puesto a `true` por el botón "Paso" de la UI: fuerza exactamente
    /// un `update_emulator()` en la siguiente `update()`, incluso en
    /// pausa — para poder avanzar frame a frame durante un análisis.
    step_once: bool,
}

impl Pc1500App {
    /// `startup`: el archivo de la línea de órdenes, ya clasificado por
    /// su extensión en `main.rs` (ver `basic_io::StartupFile`): un `.lh5`
    /// se carga en el acto, como siempre; un programa BASIC se pone en la
    /// cola y entra en cuanto la ROM termina de arrancar.
    pub fn new(_cc: &eframe::CreationContext<'_>, startup: Option<StartupFile>) -> Self {
        let mut emulator = Pc1500::new();
        // El núcleo arranca con la E/S de archivos desactivada (`ERROR
        // 27`); en la GUI, `CLOAD`/`CSAVE` usan la carpeta actual.
        let host_dir = std::env::current_dir().ok();
        let host_dir_input = host_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default();
        emulator.set_host_dir(host_dir);

        let lh5_status = if let Some(StartupFile::Lh5(path)) = &startup {
            match emulator.load_lh5_file(path) {
                Ok(()) => Some(format!("Cargado: {}", path.display())),
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
            step_once: false,
        };
        if let Some(StartupFile::Basic(path)) = startup {
            app.basic_path_input = path.display().to_string();
            app.queue_basic_file(&path);
        }
        app
    }

    /// Carga un archivo `.lh5` en el emulador y actualiza el mensaje de estado.
    pub fn load_lh5(&mut self, path: &Path) {
        self.lh5_status = match self.emulator.load_lh5_file(path) {
            Ok(()) => Some(format!("Cargado: {}", path.display())),
            Err(e) => Some(format!("Error: {e}")),
        };
    }

    /// Reinicia el emulador a un estado de arranque limpio (equivalente a
    /// encenderlo de nuevo) — la única forma de volver al entorno BASIC
    /// interactivo normal de la ROM tras cargar y ejecutar un `.lh5` de
    /// código máquina nativo, que deja la CPU parada (`HALT`) sin volver
    /// nunca al bucle de la ROM. Antes de esto, la única forma era cerrar
    /// y reabrir la aplicación entera (`Pc1500::new()` solo se llamaba una
    /// vez, al arrancar `Pc1500App`).
    ///
    /// Usa `Pc1500::reset()`, no un `Pc1500::new()` nuevo: así sobrevive
    /// lo que en la máquina real no se pierde con un reset (el reloj, con
    /// una hora puesta con `TIME=`, que tiene su propia pila) y las
    /// opciones del emulador. La lista completa está en `Pc1500::reset`;
    /// el alimentador de teclas (teclas del host pulsadas, texto pegado
    /// pendiente) NO sobrevive. Aquí solo se limpia además el estado
    /// propio de la GUI (tecla del ratón, ritmo y medida de velocidad del
    /// `Pacer`). La velocidad elegida (slider, "Máx") se conserva.
    pub fn reset(&mut self) {
        self.emulator.reset();
        self.emulator.set_auto_power_off(self.auto_power_off);
        self.mouse_key = None;
        self.key_status = None;
        self.lh5_status = Some("Emulador reiniciado".to_string());
        self.pacer.pause();
    }

    /// Lee e interpreta un programa BASIC de `path` y lo pone en la cola
    /// (ver `basic_io`). Los errores de lectura o de formato se ven en el
    /// estado sin tocar la máquina.
    fn queue_basic_file(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => self.queue_basic_bytes(&bytes, path),
            Err(e) => self.basic_status = Some(BasicStatus::error(format!("Error al leer {}: {e}", path.display()))),
        }
    }

    /// Como `queue_basic_file` con los bytes ya leídos (archivos soltados
    /// que solo traen los bytes); `path` decide la extensión.
    fn queue_basic_bytes(&mut self, bytes: &[u8], path: &Path) {
        let keywords = self.emulator.basic_keywords();
        match basic_io::decode(bytes, path, self.basic_format, &keywords) {
            Ok(decoded) => {
                self.basic_status = Some(BasicStatus::waiting(format!(
                    "Esperando al prompt BASIC para cargar {}\u{2026}",
                    path.display()
                )));
                self.pending_basic = Some(PendingLoad::new(decoded, path.display().to_string()));
            }
            Err(e) => {
                self.pending_basic = None;
                self.basic_status = Some(BasicStatus::error(format!("Error en {}: {e}", path.display())));
            }
        }
    }

    /// Mete en la máquina el programa de la cola si la ROM ya está en el
    /// prompt (se llama tras cada avance de la emulación).
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
                    Err(e) => BasicStatus::error(format!("Error al cargar {}: {e}", ready.source())),
                });
            }
        }
    }

    /// Guarda el programa de la máquina en la ruta del campo.
    fn save_basic(&mut self) {
        let path = PathBuf::from(self.basic_path_input.trim());
        if path.as_os_str().is_empty() {
            self.basic_status = Some(BasicStatus::error("Escribe la ruta del archivo".to_owned()));
            return;
        }
        self.basic_status = Some(match self.emulator.save_basic_file(&path, self.basic_format.save_format()) {
            Ok(report) => BasicStatus::ok(format!(
                "Guardado {} ({}): {} l\u{ed}neas, {} bytes",
                path.display(),
                basic_io::save_format_name(report.format),
                report.lines,
                report.bytes
            )),
            Err(e) => BasicStatus::error(format!("Error al guardar {}: {e}", path.display())),
        });
    }

    /// Archivos soltados sobre la ventana: un `.lh5` (o extensión
    /// desconocida) se carga como código máquina, un programa BASIC va a
    /// la cola. Puede no funcionar en Wayland (winit no siempre entrega
    /// los archivos soltados); el campo de ruta funciona siempre.
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
                    self.basic_status = Some(BasicStatus::error(format!("No se puede abrir {}", path.display())));
                }
            }
        }
    }

    /// Panel "Programa BASIC": ruta (registrada para no capturar el
    /// teclado mientras tiene el foco), formato, Cargar/Guardar y estado.
    fn render_basic_io(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Programa BASIC:");
                let response = self.text_edits.add(
                    ui,
                    egui::TextEdit::singleline(&mut self.basic_path_input)
                        .hint_text("ruta/al/programa.bas")
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
                        "Auto: al cargar se reconoce el contenido (texto, tokenizado o cabecera \
                         CE-158); al guardar manda la extensi\u{f3}n: .bas CE-158 (como CSAVE), \
                         .asc texto CR (CE-158X), .txt texto CRLF, .bin tokenizado sin cabecera",
                    );

                let load = ui
                    .button("Cargar BASIC")
                    .on_hover_text(
                        "Sustituye el programa de la PC-1500 (como NEW y teclearlo). Espera al \
                         prompt BASIC si la m\u{e1}quina est\u{e1} arrancando. Tambi\u{e9}n se puede \
                         soltar el archivo sobre la ventana.",
                    )
                    .clicked();
                if load || enter {
                    let path = PathBuf::from(self.basic_path_input.trim());
                    self.queue_basic_file(&path);
                }
                if ui.button("Guardar BASIC").on_hover_text("Guarda el programa de la PC-1500 en la ruta").clicked() {
                    self.save_basic();
                }
                if self.pending_basic.is_some() && ui.button("Cancelar carga").clicked() {
                    self.pending_basic = None;
                    self.basic_status = None;
                }
            });
            if let Some(status) = &self.basic_status {
                ui.colored_label(status.color(), &status.text);
            }
        });
    }

    /// Panel "CLOAD/CSAVE": la carpeta de los archivos de `CLOAD`,
    /// `CSAVE`, `MERGE` y `CHAIN` tecleados en la PC-1500, el archivo de
    /// "cinta" para los que no llevan nombre y el resultado del último.
    /// Los dos campos están registrados para no capturar el teclado
    /// mientras tienen el foco.
    fn render_host_files(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Carpeta CLOAD/CSAVE:").on_hover_text(HOST_FILES_HELP);
                let dir_field = self.text_edits.add(
                    ui,
                    egui::TextEdit::singleline(&mut self.host_dir_input)
                        .hint_text("vac\u{ed}o = desactivado (ERROR 27)")
                        .desired_width(260.0),
                );
                let dir_enter = dir_field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Usar").on_hover_text(HOST_FILES_HELP).clicked() || dir_enter {
                    self.apply_host_dir();
                }

                ui.separator();

                ui.label("Cinta:").on_hover_text(
                    "Archivo de CLOAD y CSAVE sin nombre (p. ej. CSAVE a secas). Sin cinta, \
                     dan ERROR 40.",
                );
                let tape_field = self.text_edits.add(
                    ui,
                    egui::TextEdit::singleline(&mut self.tape_input)
                        .hint_text("ruta/a/cinta.bas")
                        .desired_width(200.0),
                );
                let tape_enter = tape_field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Usar cinta").clicked() || tape_enter {
                    self.apply_tape();
                }
            });
            if let Some(status) = &self.host_status {
                ui.colored_label(status.color(), &status.text);
            }
        });
    }

    /// Botón "Usar" de la carpeta (ver `parse_host_dir`).
    fn apply_host_dir(&mut self) {
        self.host_status = Some(match parse_host_dir(&self.host_dir_input) {
            Ok(Some(dir)) => {
                let text = format!("CLOAD/CSAVE en {}", dir.display());
                self.emulator.set_host_dir(Some(dir));
                BasicStatus::ok(text)
            }
            Ok(None) => {
                self.emulator.set_host_dir(None);
                BasicStatus::waiting("CLOAD/CSAVE desactivados (dan ERROR 27)".to_owned())
            }
            Err(e) => BasicStatus::error(e),
        });
    }

    /// Botón "Usar cinta".
    fn apply_tape(&mut self) {
        let tape = parse_tape(&self.tape_input);
        self.host_status = Some(tape.as_ref().map_or_else(
            || BasicStatus::waiting("Sin cinta: CLOAD/CSAVE sin nombre dan ERROR 40".to_owned()),
            |path| BasicStatus::ok(format!("Cinta: {}", path.display())),
        ));
        self.emulator.set_tape(tape);
    }

    /// La velocidad pedida en la interfaz.
    const fn speed_mode(&self) -> SpeedMode {
        if self.max_speed {
            SpeedMode::Max
        } else {
            SpeedMode::Multiplier(self.speed_multiplier)
        }
    }

    /// Avanza la emulación lo que toque en este `update()` (ver
    /// `pacing::Pacer`), o exactamente un frame si se pulsó "Paso".
    /// Devuelve `true` si la emulación está corriendo (no en pausa).
    fn advance_emulation(&mut self, ctx: &egui::Context) -> bool {
        if self.step_once {
            self.step_once = false;
            self.emulator.step_frame();
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
        self.pacer.advance(
            now,
            mode,
            || Instant::now() >= deadline,
            |ticks| emulator.run_for_ticks(ticks),
        );
        true
    }

    /// Casillas del LCD ("Fantasma LCD", "LCD ampliado") y el aviso de
    /// máquina apagada, en el panel de controles. El LCD en sí va en la
    /// carátula (ver `render_faceplate`).
    fn render_display_options(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.lcd_ghost, "Fantasma LCD").on_hover_text(
            "Muestra apenas visibles los puntos e indicadores apagados, como en un LCD real",
        );
        ui.checkbox(&mut self.big_lcd, "LCD ampliado")
            .on_hover_text("Muestra tambi\u{e9}n el LCD en grande, encima de la car\u{e1}tula");
        if !self.emulator.is_powered_on() {
            ui.colored_label(egui::Color32::RED, egui::RichText::new(POWERED_OFF_HINT).strong());
        }
    }

    /// Pasa al emulador lo que se tecleó en el PC desde la última
    /// `update()` (ver `host_keymap`), antes de emular nada.
    fn handle_host_keyboard(&mut self) {
        // Al cruzar 1x cambia cómo se mantienen las teclas (ver
        // `HoldPolicy`): lo que estuviera pulsado se da por soltado, para
        // que ninguna se quede pegada esperando una suelta que en modo
        // `Tap` se ignora.
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

    /// Las casillas del teclado, la ayuda y el estado del tecleo, en el
    /// panel de controles.
    fn render_keyboard_options(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.capture_keyboard, "Capturar teclado del PC").on_hover_text(
                "Lo que se teclea en el PC va a la PC-1500 (salvo con el foco en un campo de \
                 texto). Sin esto, solo las teclas de la car\u{e1}tula.",
            );
            ui.checkbox(self.host_keymap.exact_case_mut(), "Distinguir may\u{fa}sculas/min\u{fa}sculas")
                .on_hover_text(
                    "Apagado: cada letra es la tecla de la PC-1500 tal cual (MAY\u{da}SCULA, como \
                     piden las palabras clave de BASIC; min\u{fa}sculas con SML = F8 o SHIFT = F11). \
                     Encendido: las letras salen como en el PC.",
                );
            if ui.button("\u{2328} Teclado").on_hover_text("Qu\u{e9} tecla del PC es cada tecla de la PC-1500").clicked() {
                self.show_key_help = !self.show_key_help;
            }
            ui.separator();
            self.render_display_options(ui);
            let pending = self.emulator.pending_keystrokes();
            if pending > 0 {
                ui.separator();
                ui.label(format!("Tecleando\u{2026} {pending} pendientes"));
                if ui.button("Cancelar tecleo").clicked() {
                    self.emulator.cancel_typing();
                }
            }
            if let Some(status) = &self.key_status {
                ui.colored_label(egui::Color32::from_rgb(0xE0, 0x80, 0x10), status);
            }
        });
    }

    /// La carátula (ver `faceplate`): LCD y teclas, ocupando todo el
    /// espacio que queda. Una tecla se ve hundida mientras está pulsada
    /// en el teclado emulado (con el ratón o con el teclado del PC, ver
    /// `Pc1500::is_key_pressed`) o mientras el ratón la tiene pulsada,
    /// aunque el alimentador todavía no la haya pasado a la matriz (a
    /// poca velocidad tarda un poco en tiempo de host). Los prefijos
    /// enganchados (`SHIFT`, `SML`, `DEF`, bits de $764E) llevan un
    /// contorno hasta la tecla siguiente.
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

    /// El ratón sobre las teclas de la carátula: al pulsar una tecla se
    /// mantiene pulsada en la PC-1500 hasta soltar el botón (aunque el
    /// puntero salga de la tecla, como con `is_pointer_button_down_on`),
    /// igual que una tecla del PC, a través del mismo alimentador (así
    /// funcionan la autorrepetición de las flechas y los juegos con
    /// `INKEY$`). Aquí solo se detectan los flancos: cuánto dura como
    /// mínimo cada pulsación lo decide el alimentador en tiempo emulado
    /// (o la política `Tap` por encima de 1x), no un tiempo del host.
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

    /// Ventana "Teclado PC → PC-1500": las tablas de `host_keymap` tal
    /// cual, para que la ayuda no pueda contar algo distinto de lo que
    /// pasa.
    fn render_key_help(&mut self, ctx: &egui::Context) {
        let mut open = self.show_key_help;
        egui::Window::new("Teclado PC \u{2192} PC-1500")
            .open(&mut open)
            .default_width(560.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                    ui.label(KEY_HELP_NOTES);
                    ui.separator();
                    ui.strong("Teclas especiales");
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
                    ui.strong("Caracteres (con cualquier distribuci\u{f3}n de teclado del PC)");
                    egui::Grid::new("key_help_chars").striped(true).show(ui, |ui| {
                        for (index, (host, sequence)) in host_keymap::char_table().iter().enumerate() {
                            ui.monospace(host);
                            match sequence {
                                Some(sequence) => ui.label(sequence),
                                None => ui.weak("sin tecla"),
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

    /// Control de velocidad: pausa, paso a paso, un slider logarítmico de
    /// multiplicador respecto al tiempo real de hardware (`1.00x`) de
    /// `SPEED_MIN` a `SPEED_MAX`, botones de velocidades típicas, el
    /// botón "Máx" (todo lo que dé el host) y la velocidad efectiva
    /// medida, que avisa cuando el host no da para la pedida. Ver
    /// `pacing`.
    fn render_speed_control(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                let pause_label = if self.paused {
                    "\u{25b6} Reanudar"
                } else {
                    "\u{23f8} Pausa"
                };
                if ui.button(pause_label).clicked() {
                    self.paused = !self.paused;
                    // En pausa no se piden repintados (ver `update`): que
                    // el siguiente llegue ya, sin esperar a otro evento.
                    ui.ctx().request_repaint();
                }

                if ui
                    .add_enabled(self.paused, egui::Button::new("\u{23ed} Paso"))
                    .on_hover_text("Avanzar exactamente un frame emulado (~11.5 ms de la PC-1500)")
                    .clicked()
                {
                    self.step_once = true;
                    ui.ctx().request_repaint();
                }

                ui.separator();

                ui.label("Velocidad:");
                // Sin la casilla numérica del slider (`show_value(false)`):
                // al pulsarla, egui la convierte en un campo de texto que
                // no está en `text_edits`, y `raw_input_hook` le robaría las
                // teclas para mandarlas a la PC-1500. El valor se muestra
                // aparte como etiqueta.
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
                ui.toggle_value(&mut self.max_speed, "\u{23e9} M\u{e1}x")
                    .on_hover_text(
                        "Emula todo lo que d\u{e9} el ordenador, sin l\u{ed}mite de velocidad \
                         (la interfaz puede ir algo menos fluida)",
                    );

                ui.separator();
                self.render_measured_speed(ui);

                ui.separator();

                if ui
                    .checkbox(&mut self.auto_power_off, "Auto-apagado")
                    .on_hover_text(
                        "Como la PC-1500 real: se apaga sola tras unos 5 minutos sin pulsar \
                         ninguna tecla (antes si la velocidad es mayor de 1x). ON (Esc) la \
                         vuelve a encender donde estaba.",
                    )
                    .changed()
                {
                    self.emulator.set_auto_power_off(self.auto_power_off);
                }
            });
        });
    }

    /// La velocidad efectiva medida por el `Pacer` (ver
    /// `speed_status_text`), en naranja si la limita el host.
    fn render_measured_speed(&self, ui: &mut egui::Ui) {
        let measurement = self.pacer.measurement();
        let status = speed_status_text(self.paused, self.max_speed, measurement);
        ui.label(status.measured).on_hover_text(
            "Velocidad real conseguida (medida cada 0.5 s): m\u{fa}ltiplo del tiempo real de \
             la PC-1500 y ciclos de CPU emulados por segundo (la real va a 1.3 MHz)",
        );
        if let Some(warning) = status.warning {
            ui.colored_label(egui::Color32::from_rgb(0xE0, 0x80, 0x10), warning)
                .on_hover_text(
                    "El ordenador no da para emular a la velocidad pedida: va a la que se \
                     indica",
                );
        }
    }

    fn render_lh5_loader(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Archivo .lh5:");

                // Registrado: mientras tenga el foco, el teclado no va a
                // la PC-1500 (ver `TextEditRegistry`).
                let response = self.text_edits.add(
                    ui,
                    egui::TextEdit::singleline(&mut self.lh5_path_input)
                        .hint_text("ruta/al/programa.lh5")
                        .desired_width(300.0),
                );

                // Allow pressing Enter inside the text field to trigger loading
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let path = std::path::PathBuf::from(self.lh5_path_input.clone());
                    self.load_lh5(&path);
                }

                if ui.button("Cargar").clicked() {
                    let path = std::path::PathBuf::from(self.lh5_path_input.clone());
                    self.load_lh5(&path);
                }

                if ui
                    .button("Reset")
                    .on_hover_text("Reinicia el emulador a un arranque limpio (vuelve al entorno BASIC normal de la ROM)")
                    .clicked()
                {
                    self.reset();
                }

                ui.separator();

                // Hora del reloj µPD1990AC, la misma que leería `TIME`.
                ui.label(format!("Reloj: {}", self.emulator.rtc_now()))
                    .on_hover_text(
                        "La hora que lee TIME (d\u{ed}a/mes hora). Empieza en la hora local del \
                         ordenador y avanza en tiempo real, a cualquier velocidad de emulaci\u{f3}n, \
                         como el reloj con pila de la PC-1500 (WAIT y BEEP s\u{ed} siguen la \
                         velocidad). TIME=MMDDHH.MMSS la cambia; se conserva con Reset, no al \
                         cerrar el emulador.",
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
    /// Roba los eventos de teclado antes de que los procese egui (ver
    /// `host_keymap::capture_events`), salvo si la captura está
    /// desactivada o un campo de texto registrado tiene el foco.
    ///
    /// Se ejecuta antes de `update()` (eframe lo llama antes de empezar
    /// el pase de egui): el foco que se mira es el del repintado
    /// anterior, igual que los campos registrados.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let focused = ctx.memory(egui::Memory::focused);
        let capturing = self.capture_keyboard && self.text_edits.allows_capture(focused);
        if capturing {
            self.host_events
                .extend(host_keymap::capture_events(&mut raw_input.events));
        }
        if self.was_capturing && !capturing {
            // Las sueltas de lo que siguiera pulsado ya no van a llegar.
            self.host_events.push(egui::Event::WindowFocused(false));
        }
        self.was_capturing = capturing;
    }

    /// Emula lo que toque y pinta la interfaz.
    ///
    /// # Historia del ritmo
    /// Al principio esto hacía un `step_frame()` por repintado, sin mirar
    /// el reloj: la velocidad quedaba atada al vsync del monitor. Luego
    /// se ritmó con `FRAME_DURATION`, pero como mucho UN frame emulado
    /// (~11.5 ms) por repintado (~16.7 ms a 60 Hz), así que la emulación
    /// no pasaba de ~0.69x por mucho que se pidiera. Ahora el
    /// `pacing::Pacer` emula en cada `update()` los ciclos que tocan por
    /// el tiempo real transcurrido (de ~433 a 0.02x hasta millones en
    /// modo Máx), con un presupuesto de tiempo de host para que la
    /// interfaz siga respondiendo.
    ///
    /// Mientras corre se pide siempre el siguiente repintado
    /// (`request_repaint`): el vsync ya limita a la frecuencia del
    /// monitor, y así incluso a 0.02x la emulación avanza un poco en cada
    /// frame de la interfaz en vez de a saltos. En pausa no hace falta:
    /// egui repinta solo al haber eventos.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Teclado del PC primero (en tiempo emulado lo pulsa el
        // alimentador, dentro de `run_for_ticks`).
        self.handle_host_keyboard();
        self.handle_dropped_files(ctx);
        // Los campos de texto se vuelven a registrar al dibujarlos.
        self.text_edits.clear();

        if self.advance_emulation(ctx) {
            ctx.request_repaint();
        }
        // La carga BASIC pendiente, en cuanto la ROM llegue al prompt.
        self.apply_pending_basic();
        // Lo que hayan hecho CLOAD/CSAVE en la PC-1500: se ve el último.
        if let Some(event) = self.emulator.take_host_events().last() {
            self.host_status = Some(host_event_status(event));
        }

        // Una única instantánea del LCD por actualización (ver
        // `Pc1500::display`), pintada luego por `lcd_view::show`.
        self.lcd = LcdFrame::from_display(self.emulator.display());

        // Arriba, el panel de controles (velocidad, cargadores, casillas
        // del teclado y del LCD); el resto de la ventana es para la
        // carátula, que se escala para llenarlo (ver `faceplate`). Los
        // paneles de archivos BASIC y CLOAD/CSAVE se pueden plegar para
        // dejarle más sitio.
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.add_space(2.0);
            // Control de velocidad (pausa/paso/multiplicador)
            self.render_speed_control(ui);
            // Cargador .lh5, Reset y reloj
            self.render_lh5_loader(ui);
            egui::CollapsingHeader::new("Programas BASIC y CLOAD/CSAVE")
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

/// Estado del panel "Programa BASIC".
#[derive(Clone, Debug, PartialEq, Eq)]
struct BasicStatus {
    kind: StatusKind,
    text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusKind {
    Ok,
    /// En espera, o hecho pero con avisos.
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

/// Ayuda del panel "CLOAD/CSAVE".
const HOST_FILES_HELP: &str = "\
Carpeta de los archivos de CLOAD, CSAVE, MERGE y CHAIN tecleados en la PC-1500 \
(como con una CE-158). CSAVE \"X\" guarda X.BAS tokenizado con cabecera CE-158; \
CSAVEa \"X\" guarda X.ASC en texto; con extensi\u{f3}n .bin se guarda tokenizado sin \
cabecera y con .txt en texto. CLOAD \"X\" reconoce el formato solo (busca X.BAS, \
X.ASC, X.TXT, X.BIN sin distinguir may\u{fa}sculas); CLOAD? \"X\" compara. CLOAD y \
MERGE solo en modo directo; CHAIN \"X\"[,l\u{ed}nea] dentro de un programa. \
Vac\u{ed}o = desactivado (ERROR 27).";

/// Lo que dice la carpeta del panel "CLOAD/CSAVE": vacía, desactivar
/// (`Ok(None)`); una carpeta que existe, usarla; otra cosa, un error que
/// mostrar sin cambiar nada.
fn parse_host_dir(input: &str) -> Result<Option<PathBuf>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let dir = PathBuf::from(trimmed);
    if dir.is_dir() {
        Ok(Some(dir))
    } else {
        Err(format!("No es una carpeta: {}", dir.display()))
    }
}

/// El archivo de cinta del campo "Cinta" (vacío = sin cinta). No tiene
/// por qué existir todavía: `CSAVE` lo crea.
fn parse_tape(input: &str) -> Option<PathBuf> {
    let trimmed = input.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// La línea de estado de un evento de `CLOAD`/`CSAVE` (su texto, de
/// `HostEvent`): rojo si la PC-1500 dio `ERROR`, naranja si cargó con
/// avisos, verde si no.
fn host_event_status(event: &HostEvent) -> BasicStatus {
    let text = event.to_string();
    match event {
        HostEvent::Error { .. } => BasicStatus::error(text),
        HostEvent::Loaded { warnings, .. } if !warnings.is_empty() => BasicStatus::waiting(text),
        _ => BasicStatus::ok(text),
    }
}

/// Notas de la ventana de ayuda del teclado.
const KEY_HELP_NOTES: &str = "\
\u{b7} SHIFT y DEF son prefijos, como en la m\u{e1}quina real: se pulsan y se sueltan \
(F11 / F7) y afectan a la tecla siguiente. El Shift del PC solo elige qu\u{e9} car\u{e1}cter \
se teclea: \"(\" o \"?\" salen solos, con el SHIFT de la PC-1500 que haga falta.\n\
\u{b7} Las letras salen en may\u{fa}scula. Min\u{fa}sculas: SML (F8), SHIFT (F11) + letra, o la \
casilla \"Distinguir may\u{fa}sculas/min\u{fa}sculas\".\n\
\u{b7} Ctrl+V teclea el portapapeles (despacio: la ROM necesita unos 8 frames emulados, \
~92 ms a 1x, por tecla); \"Cancelar tecleo\" o ON/BRK lo paran.\n\
\u{b7} Hasta 1x una tecla mantenida se mantiene en la PC-1500 (las flechas se repiten solas); \
por encima de 1x cada pulsaci\u{f3}n (y cada repetici\u{f3}n del PC) es un toque corto.\n\
\u{b7} Mientras un campo de texto tiene el foco, el teclado es para ese campo.";

/// `true` si `key` es un prefijo enganchado ahora mismo (bits de $764E).
const fn latched(key: Pc1500Key, symbols: u8) -> bool {
    let bit = match key {
        Pc1500Key::Shift => SYMBOL_SHIFT,
        Pc1500Key::Sml => SYMBOL_SML,
        Pc1500Key::Control => SYMBOL_DEF,
        _ => return false,
    };
    symbols & bit != 0
}

/// Texto de un botón de velocidad: `0.1`, `0.25`, `1`, `50`...
fn format_preset(preset: f64) -> String {
    let text = format!("{preset:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Lo que muestra la interfaz sobre la velocidad efectiva.
#[derive(Debug, PartialEq, Eq)]
struct SpeedStatus {
    /// Siempre visible: velocidad medida, o por qué no la hay.
    measured: String,
    /// Aviso en naranja, si lo hay.
    warning: Option<&'static str>,
}

/// Ver `SpeedStatus`. En pausa o antes de la primera medida no hay
/// velocidad que mostrar; en modo Máx no se avisa de que limita el host,
/// porque eso es justo lo que se ha pedido.
fn speed_status_text(
    paused: bool,
    max_speed: bool,
    measurement: Option<pacing::Measurement>,
) -> SpeedStatus {
    if paused {
        return SpeedStatus { measured: "Real: en pausa".to_owned(), warning: None };
    }
    let Some(m) = measurement else {
        return SpeedStatus { measured: "Real: midiendo\u{2026}".to_owned(), warning: None };
    };
    let measured = format!(
        "Real: x{} \u{b7} {:.2} MHz",
        pacing::format_multiplier(m.multiplier),
        m.emulated_mhz
    );
    let warning = if max_speed {
        Some("turbo: todo lo que da el host")
    } else if m.host_limited {
        Some("limitado por el host")
    } else {
        None
    };
    SpeedStatus { measured, warning }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(parse_host_dir("  "), Ok(None), "vac\u{ed}o desactiva");
        let dir = std::env::temp_dir();
        assert_eq!(parse_host_dir(&format!(" {} ", dir.display())), Ok(Some(dir.clone())), "carpeta");
        let missing = dir.join("ceres-no-existe-\u{f1}");
        assert!(parse_host_dir(&missing.display().to_string()).is_err(), "no existe");
        assert_eq!(parse_tape(" "), None, "sin cinta");
        assert_eq!(parse_tape(" a/b.bas "), Some(PathBuf::from("a/b.bas")), "cinta");
    }

    #[test]
    fn host_event_colors() {
        let error = HostEvent::Error { command: ceres_core::HostCommand::Cload, code: 61, message: "no existe".to_owned() };
        assert_eq!(host_event_status(&error).kind, StatusKind::Error, "error");
        let verified = HostEvent::Verified { path: PathBuf::from("x"), bytes: 1 };
        let status = host_event_status(&verified);
        assert_eq!(status.kind, StatusKind::Ok, "verificado");
        assert_eq!(status.text, "CLOAD? x: coincide (1 bytes)", "texto del evento");
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
        assert_eq!(paused.measured, "Real: en pausa");
        assert_eq!(paused.warning, None);
        assert_eq!(speed_status_text(false, false, None).warning, None);

        let ok = speed_status_text(false, false, Some(normal));
        assert_eq!(ok.measured, "Real: x1.00 \u{b7} 1.30 MHz");
        assert_eq!(ok.warning, None);

        let slow_host = speed_status_text(false, false, Some(limited));
        assert_eq!(slow_host.measured, "Real: x152 \u{b7} 198.12 MHz");
        assert_eq!(slow_host.warning, Some("limitado por el host"));

        let turbo = speed_status_text(false, true, Some(limited));
        assert_eq!(turbo.warning, Some("turbo: todo lo que da el host"));
    }
}
