//! Del teclado del PC (eventos de egui) a teclas de la PC-1500.
//!
//! Todo lo de aquí es lógica pura, sin interfaz, para poder probarla con
//! listas de eventos sintéticas: [`HostKeymap::translate`] convierte
//! eventos en [`HostAction`], y [`apply`] se las pasa al alimentador de
//! teclas del emulador (`Pc1500::host_key_down`...), que es quien pulsa y
//! suelta en tiempo EMULADO. La interfaz (`pc1500_app.rs`) solo roba los
//! eventos de teclado antes de que los vea egui ([`capture_events`],
//! desde `raw_input_hook`) y pinta.
//!
//! # Caracteres por texto, el resto por tabla
//! Los caracteres imprimibles se toman del `Event::Text` que manda
//! egui-winit justo detrás del `Event::Key` de la misma pulsación, no de
//! la tecla: así da igual la distribución del teclado del host (en un
//! teclado español `(` es Shift+8, y egui no tiene ni siquiera variante
//! de `egui::Key` para `(`). Lo que no es texto (flechas, F1...,
//! Esc...) sale de la tabla fija [`SPECIAL`]. El `SHIFT` del host no es
//! el `SHIFT` de la PC-1500 (que es un prefijo, ver
//! `ceres_core::KeyStroke`): solo decide qué carácter produce el host, y
//! las combinaciones Shift+tecla especial de [`SPECIAL`]. El `SHIFT` de
//! la PC-1500 es F11 o la tecla de la pantalla.
//!
//! La ayuda de la interfaz (ventana "Teclado") y las pistas de cada
//! tecla ([`hint`]) se generan de las mismas tablas, así que no pueden
//! desincronizarse del comportamiento real.

use std::collections::HashMap;

use ceres_core::{
    HostKeyId, Key, KeyStroke, Pc1500, ShiftPolicy, harness::key_for_char,
};
use eframe::egui::{self, Event};

/// Identificador de "tecla del host" para el ratón sobre las teclas de la
/// pantalla. Fuera del rango de `egui::Key as u32`.
pub const MOUSE_ID: HostKeyId = 0xFFFF_0000;

/// Identificador para el `ON/BRK` de Ctrl+C (`Event::Copy`, que no trae
/// `Event::Key`).
pub const COPY_ID: HostKeyId = 0xFFFF_0001;

/// Modificadores con los que se busca una fila de [`SPECIAL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mods {
    /// Sin Ctrl (con o sin Shift si no hay fila `Shift` propia).
    None,
    /// Shift sin Ctrl.
    Shift,
    /// Ctrl (o Cmd).
    Ctrl,
}

/// Lo que hace una tecla especial del host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialAction {
    /// Una tecla de la PC-1500 que se mantiene mientras se mantenga en el
    /// host (ver `Pc1500::host_key_down`).
    Hold(KeyStroke),
    /// Pulsaciones sueltas de duración fija. Con `repeat`, la
    /// autorrepetición del sistema operativo las repite (Retroceso).
    Tap { strokes: &'static [KeyStroke], repeat: bool },
    /// `ON/BRK`.
    Brk,
}

/// Una fila de la tabla de teclas especiales.
#[derive(Clone, Copy, Debug)]
pub struct Special {
    pub key: egui::Key,
    pub mods: Mods,
    /// Cómo se llama la tecla del host en la ayuda.
    pub host_label: &'static str,
    pub action: SpecialAction,
    /// Explicación para la ayuda (puede ir vacía).
    pub note: &'static str,
}

const fn hold(key: egui::Key, mods: Mods, host_label: &'static str, stroke: KeyStroke, note: &'static str) -> Special {
    Special { key, mods, host_label, action: SpecialAction::Hold(stroke), note }
}

const fn brk(key: egui::Key, mods: Mods, host_label: &'static str, note: &'static str) -> Special {
    Special { key, mods, host_label, action: SpecialAction::Brk, note }
}

const BACKSPACE: &[KeyStroke] = &[KeyStroke::plain(Key::Left), KeyStroke::shifted(Key::Left)];
const OFF: &[KeyStroke] = &[KeyStroke::plain(Key::Off)];
const SHIFT_PREFIX: &[KeyStroke] = &[KeyStroke::plain(Key::Shift)];

/// Las teclas del host que no son texto (o lo son pero con otro
/// significado), en el orden en que salen en la ayuda. La tabla
/// definitiva decidida con el usuario: Esc = `ON/BRK`, `OFF` solo con
/// Ctrl+F12, Inicio = `CL`, Retroceso = borrar de verdad.
pub const SPECIAL: &[Special] = {
    use KeyStroke as S;
    use egui::Key as E;
    use Key as P;
    use Mods::{Ctrl, None as No, Shift};
    &[
        brk(E::Escape, No, "Esc", "BREAK; enciende la m\u{e1}quina apagada"),
        brk(E::F12, No, "F12", "igual que Esc"),
        brk(E::End, No, "Fin", "igual que Esc"),
        Special {
            key: E::F12,
            mods: Ctrl,
            host_label: "Ctrl+F12",
            action: SpecialAction::Tap { strokes: OFF, repeat: false },
            note: "apaga (solo con Ctrl, para no pulsarlo sin querer)",
        },
        hold(E::Home, No, "Inicio", S::plain(P::Cl), "borra la l\u{ed}nea / el error"),
        hold(E::Home, Shift, "Shift+Inicio", S::shifted(P::Cl), "CA: borra todo"),
        hold(E::Enter, No, "Intro", S::plain(P::Enter), ""),
        Special {
            key: E::Backspace,
            mods: No,
            host_label: "Retroceso",
            action: SpecialAction::Tap { strokes: BACKSPACE, repeat: true },
            note: "borra a la izquierda (\u{2190} y luego DEL)",
        },
        hold(E::Delete, No, "Supr", S::shifted(P::Left), "DEL: borra bajo el cursor (sin repetici\u{f3}n, como la real)"),
        hold(E::Insert, No, "Insert", S::shifted(P::Right), "INS: abre un hueco"),
        hold(E::ArrowLeft, No, "\u{2190}", S::plain(P::Left), "con autorrepetici\u{f3}n de la ROM"),
        hold(E::ArrowRight, No, "\u{2192}", S::plain(P::Right), ""),
        hold(E::ArrowUp, No, "\u{2191}", S::plain(P::Up), ""),
        hold(E::ArrowDown, No, "\u{2193}", S::plain(P::Down), ""),
        hold(E::ArrowLeft, Shift, "Shift+\u{2190}", S::shifted(P::Left), "DEL"),
        hold(E::ArrowRight, Shift, "Shift+\u{2192}", S::shifted(P::Right), "INS"),
        hold(E::ArrowUp, Shift, "Shift+\u{2191}", S::shifted(P::Up), "[ (en el LCD se ve \u{221a})"),
        hold(E::ArrowDown, Shift, "Shift+\u{2193}", S::shifted(P::Down), "] (en el LCD se ve \u{3c0})"),
        hold(E::PageUp, No, "Re P\u{e1}g", S::plain(P::Up), ""),
        hold(E::PageDown, No, "Av P\u{e1}g", S::plain(P::Down), ""),
        hold(E::Tab, No, "Tab", S::plain(P::Mode), "MODE: RUN / PRO"),
        hold(E::Tab, Shift, "Shift+Tab", S::shifted(P::Mode), "RESERVE"),
        hold(E::F1, No, "F1", S::plain(P::F1), ""),
        hold(E::F2, No, "F2", S::plain(P::F2), ""),
        hold(E::F3, No, "F3", S::plain(P::F3), ""),
        hold(E::F4, No, "F4", S::plain(P::F4), ""),
        hold(E::F5, No, "F5", S::plain(P::F5), ""),
        hold(E::F6, No, "F6", S::plain(P::F6), ""),
        hold(E::F1, Shift, "Shift+F1", S::shifted(P::F1), "!"),
        hold(E::F2, Shift, "Shift+F2", S::shifted(P::F2), "\""),
        hold(E::F3, Shift, "Shift+F3", S::shifted(P::F3), "#"),
        hold(E::F4, Shift, "Shift+F4", S::shifted(P::F4), "$"),
        hold(E::F5, Shift, "Shift+F5", S::shifted(P::F5), "%"),
        hold(E::F6, Shift, "Shift+F6", S::shifted(P::F6), "&"),
        hold(E::F7, No, "F7", S::plain(P::Control), "DEF (prefijo)"),
        hold(E::F8, No, "F8", S::plain(P::Sml), "SML: min\u{fa}sculas s\u{ed}/no"),
        hold(E::F9, No, "F9", S::plain(P::Rsv), "RSV: cambia de capa de teclas reservadas"),
        hold(E::F10, No, "F10", S::plain(P::Rcl), ""),
        Special {
            key: E::F11,
            mods: No,
            host_label: "F11",
            action: SpecialAction::Tap { strokes: SHIFT_PREFIX, repeat: false },
            note: "SHIFT de la PC-1500 (prefijo: afecta a la siguiente tecla)",
        },
    ]
};

/// Filas extra de la ayuda que no son teclas de [`SPECIAL`] (llegan como
/// otros eventos de egui): `(tecla del host, efecto)`.
pub const EXTRA_HELP: &[(&str, &str)] = &[
    ("Ctrl+C", "ON/BRK (BREAK), como en un terminal"),
    ("Ctrl+V", "teclea el texto del portapapeles (exacto, may\u{fa}sculas/min\u{fa}sculas incluidas)"),
];

/// Busca la fila de [`SPECIAL`] de una tecla con sus modificadores. Con
/// Ctrl solo valen las filas `Ctrl`; con Shift, la fila `Shift` si la
/// hay y si no la normal (Shift+F8 = F8).
#[must_use]
pub fn find_special(key: egui::Key, modifiers: egui::Modifiers) -> Option<&'static Special> {
    let find = |mods| SPECIAL.iter().find(|s| s.key == key && s.mods == mods);
    if is_command(modifiers) {
        find(Mods::Ctrl)
    } else if modifiers.shift {
        find(Mods::Shift).or_else(|| find(Mods::None))
    } else {
        find(Mods::None)
    }
}

/// Ctrl (o Cmd en macOS): los atajos que se dejan a egui (zoom...).
const fn is_command(modifiers: egui::Modifiers) -> bool {
    modifiers.ctrl || modifiers.command || modifiers.mac_cmd
}

/// Lo que hay que hacer en el emulador por un evento del host (ver
/// [`apply`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostAction {
    /// Tecla física pulsada: mantener `stroke` hasta su [`HostAction::Up`].
    Down { id: HostKeyId, stroke: KeyStroke, policy: ShiftPolicy },
    /// Autorrepetición del sistema operativo de una tecla mantenida.
    Repeat { id: HostKeyId, stroke: KeyStroke, policy: ShiftPolicy },
    /// Tecla física soltada.
    Up { id: HostKeyId },
    /// Pulsaciones sueltas de duración fija.
    Tap(Vec<KeyStroke>),
    /// Autorrepetición del sistema operativo de una tecla que se traduce
    /// a toques (Retroceso). Es distinta de [`HostAction::Tap`] porque
    /// [`apply`] solo la encola si el alimentador no tiene nada
    /// pendiente: un retroceso son dos toques (`←` y `SHIFT` + `←`, unos
    /// 24 frames emulados, ~276 ms a 1x) y el SO repite cada ~33 ms, así
    /// que encolándolas todas la cola crecería sin límite y la máquina
    /// seguiría borrando segundos después de soltar la tecla.
    TapRepeat(Vec<KeyStroke>),
    /// Texto a teclear (pegado, o caracteres que llegan sin tecla).
    TypeText(String),
    /// `ON/BRK` pulsada (`down`) o soltada.
    Brk { id: HostKeyId, down: bool },
    /// Soltar todo (la ventana perdió el foco).
    ReleaseAll,
    /// Un carácter que no tiene tecla en la PC-1500.
    Ignored(char),
}

/// Qué se recuerda de una tecla del host mientras está pulsada: para
/// soltarla aunque al soltarla cambien los modificadores y para saber
/// qué hacer con su autorrepetición.
#[derive(Clone, Copy, Debug)]
enum Held {
    Live(KeyStroke, ShiftPolicy),
    Tap(&'static [KeyStroke]),
    Brk,
    Nothing,
}

/// El traductor de eventos (con el estado de qué teclas físicas siguen
/// pulsadas, que hay que llevar a mano porque los eventos se roban antes
/// de que egui los procese, ver [`capture_events`]).
#[derive(Debug, Default)]
pub struct HostKeymap {
    down: HashMap<HostKeyId, Held>,
    /// Casilla "Distinguir mayúsculas/minúsculas": las letras salen con
    /// la caja del host (minúscula = `SHIFT` + letra, compensando `SML`).
    /// Apagada (lo predeterminado), cada letra es la tecla de la PC-1500
    /// sin más, que es mayúscula salvo con `SML`: lo que quieren las
    /// palabras clave de BASIC.
    exact_case: bool,
}

impl HostKeymap {
    /// La casilla "Distinguir mayúsculas/minúsculas" (ver `exact_case`).
    pub const fn exact_case_mut(&mut self) -> &mut bool {
        &mut self.exact_case
    }

    /// Traduce los eventos de un repintado. Ver el comentario del módulo.
    ///
    /// - `Event::Key` pulsado seguido de `Event::Text`: es la MISMA
    ///   pulsación (egui-winit los manda así); el texto se toma de ahí y
    ///   se consume, para no teclearlo dos veces.
    /// - El `id` de una tecla es su tecla física (`physical_key`, o la
    ///   lógica si no hay): la suelta se empareja por él.
    /// - Una pulsación de un `id` que ya está pulsado es autorrepetición
    ///   del sistema operativo (egui-winit siempre manda `repeat: false`
    ///   y es egui quien lo rellena, pero los eventos se le roban antes).
    #[must_use]
    pub fn translate(&mut self, events: &[Event]) -> Vec<HostAction> {
        let mut out = Vec::new();
        let mut index = 0;
        while index < events.len() {
            match &events[index] {
                Event::Key { key, physical_key, pressed, repeat, modifiers } => {
                    let id = host_key_id(*key, *physical_key);
                    let text = match events.get(index + 1) {
                        Some(Event::Text(text)) if *pressed => {
                            index += 1;
                            Some(text.as_str())
                        }
                        _ => None,
                    };
                    if !*pressed {
                        self.release(id, &mut out);
                    } else if let Some(&held) = self.down.get(&id) {
                        repeat_action(id, held, &mut out);
                    } else if *repeat {
                        // Repetición de una tecla pulsada antes de que
                        // empezáramos a capturar: no es nuestra.
                    } else {
                        let held = self.press(id, *key, *modifiers, text, &mut out);
                        self.down.insert(id, held);
                    }
                }
                Event::Text(text) | Event::Paste(text) => {
                    out.push(HostAction::TypeText(text.clone()));
                }
                Event::Copy => {
                    out.push(HostAction::Brk { id: COPY_ID, down: true });
                    out.push(HostAction::Brk { id: COPY_ID, down: false });
                }
                Event::WindowFocused(false) => {
                    self.down.clear();
                    out.push(HostAction::ReleaseAll);
                }
                _ => {}
            }
            index += 1;
        }
        out
    }

    fn release(&mut self, id: HostKeyId, out: &mut Vec<HostAction>) {
        match self.down.remove(&id) {
            Some(Held::Live(..)) => out.push(HostAction::Up { id }),
            Some(Held::Brk) => out.push(HostAction::Brk { id, down: false }),
            Some(Held::Tap(_) | Held::Nothing) | None => {}
        }
    }

    fn press(
        &self,
        id: HostKeyId,
        key: egui::Key,
        modifiers: egui::Modifiers,
        text: Option<&str>,
        out: &mut Vec<HostAction>,
    ) -> Held {
        if let Some(special) = find_special(key, modifiers) {
            return match special.action {
                SpecialAction::Hold(stroke) => {
                    out.push(HostAction::Down { id, stroke, policy: ShiftPolicy::AddIfNeeded });
                    Held::Live(stroke, ShiftPolicy::AddIfNeeded)
                }
                SpecialAction::Tap { strokes, repeat } => {
                    out.push(HostAction::Tap(strokes.to_vec()));
                    if repeat { Held::Tap(strokes) } else { Held::Nothing }
                }
                SpecialAction::Brk => {
                    out.push(HostAction::Brk { id, down: true });
                    Held::Brk
                }
            };
        }
        if is_command(modifiers) {
            return Held::Nothing;
        }
        let Some(text) = text else {
            // Shift solo, una tecla muerta (el carácter llega con la
            // siguiente pulsación)...
            return Held::Nothing;
        };
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => {
                if let Some((stroke, policy)) = self.char_stroke(c) {
                    out.push(HostAction::Down { id, stroke, policy });
                    Held::Live(stroke, policy)
                } else {
                    out.push(HostAction::Ignored(c));
                    Held::Nothing
                }
            }
            (Some(_), Some(_)) => {
                out.push(HostAction::TypeText(text.to_owned()));
                Held::Nothing
            }
            (None, _) => Held::Nothing,
        }
    }

    /// La pulsación de un carácter tecleado en vivo (ver `exact_case`).
    fn char_stroke(&self, c: char) -> Option<(KeyStroke, ShiftPolicy)> {
        if c.is_ascii_alphabetic() && !self.exact_case {
            let stroke = key_for_char(c.to_ascii_uppercase())?;
            return Some((stroke, ShiftPolicy::AddIfNeeded));
        }
        let policy = if c.is_ascii_alphabetic() { ShiftPolicy::Exact } else { ShiftPolicy::AddIfNeeded };
        key_for_char(c).map(|stroke| (stroke, policy))
    }
}

/// Ver [`HostKeymap::translate`].
fn host_key_id(key: egui::Key, physical_key: Option<egui::Key>) -> HostKeyId {
    physical_key.unwrap_or(key) as HostKeyId
}

fn repeat_action(id: HostKeyId, held: Held, out: &mut Vec<HostAction>) {
    match held {
        Held::Live(stroke, policy) => out.push(HostAction::Repeat { id, stroke, policy }),
        Held::Tap(strokes) => out.push(HostAction::TapRepeat(strokes.to_vec())),
        Held::Brk | Held::Nothing => {}
    }
}

/// Cómo se mantienen las teclas según la velocidad (decisión del
/// usuario).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldPolicy {
    /// Hasta 1x: una tecla del host se mantiene en la PC-1500 mientras
    /// se mantenga físicamente (con un mínimo en tiempo emulado); la
    /// autorrepetición la hace la ROM, como en la máquina real.
    Hold,
    /// Por encima de 1x (y en modo Máx): cada pulsación del host,
    /// autorrepeticiones del sistema operativo incluidas, es una
    /// pulsación de duración fija en tiempo emulado y las sueltas se
    /// ignoran. Si no, a 50x una flecha mantenida un instante se
    /// autorrepetiría decenas de veces.
    Tap,
}

/// Ver [`HoldPolicy`].
#[must_use]
pub fn hold_policy(max_speed: bool, multiplier: f64) -> HoldPolicy {
    // Margen para que el 1.00x del slider logarítmico (que puede quedar
    // en 1.0000001) siga siendo "tiempo real".
    if max_speed || multiplier > 1.0 + 1e-6 { HoldPolicy::Tap } else { HoldPolicy::Hold }
}

/// Aplica una acción al emulador. Devuelve un mensaje para la línea de
/// estado si hay algo que contar (caracteres sin tecla).
pub fn apply(pc: &mut Pc1500, action: &HostAction, policy: HoldPolicy) -> Option<String> {
    let tap = policy == HoldPolicy::Tap;
    match *action {
        HostAction::Down { id, stroke, policy: shift } => {
            if tap {
                pc.host_tap_with(&[stroke], shift);
            } else {
                pc.host_key_down_with(id, stroke, shift);
            }
        }
        HostAction::Repeat { stroke, policy: shift, .. } => {
            // Hasta 1x la autorrepetición la hace la propia ROM. Por
            // encima, cada repetición del SO es un toque, pero solo si el
            // alimentador ya no tiene nada pendiente: cada toque dura 8
            // frames emulados (~46 ms de host a 2x) y el SO repite cada
            // ~33 ms, así que sin esto se acumularía cola y la tecla
            // seguiría "repitiéndose" un rato después de soltarla.
            if tap && pc.pending_keystrokes() == 0 {
                pc.host_tap_with(&[stroke], shift);
            }
        }
        HostAction::Up { id } => {
            if !tap {
                pc.host_key_up(id);
            }
        }
        HostAction::Tap(ref strokes) => pc.host_tap(strokes),
        HostAction::TapRepeat(ref strokes) => {
            // Con cualquier política: ver [`HostAction::TapRepeat`]. Al
            // soltar la tecla queda como mucho el retroceso en curso.
            if pc.pending_keystrokes() == 0 {
                pc.host_tap(strokes);
            }
        }
        HostAction::TypeText(ref text) => return skipped_message(&pc.queue_text(text)),
        HostAction::Brk { id, down: true } => {
            pc.host_key_down(id, KeyStroke::plain(Key::On));
            if tap {
                pc.host_key_up(id);
            }
        }
        HostAction::Brk { id, down: false } => pc.host_key_up(id),
        HostAction::ReleaseAll => pc.release_all_host_keys(),
        HostAction::Ignored(c) => return skipped_message(&[c]),
    }
    None
}

/// Mensaje para caracteres que no se han podido teclear.
fn skipped_message(skipped: &[char]) -> Option<String> {
    match skipped {
        [] => None,
        [c] => Some(format!("car\u{e1}cter sin tecla en la PC-1500: {}", show_char(*c))),
        many => {
            let list: Vec<String> = many.iter().map(|&c| show_char(c)).collect();
            Some(format!("caracteres sin tecla en la PC-1500 (omitidos): {}", list.join(" ")))
        }
    }
}

fn show_char(c: char) -> String {
    if c.is_control() { c.escape_default().collect() } else { c.to_string() }
}

/// Saca de `events` los eventos de teclado que son para el emulador y los
/// devuelve (se llama desde `raw_input_hook`, antes de que egui los
/// procese, solo si se está capturando el teclado).
///
/// Así egui no ve Tab ni las flechas (que moverían el foco entre
/// controles) ni Espacio/Intro (que pulsarían el botón con el foco).
/// Se quedan en egui los atajos con Ctrl/Cmd salvo Ctrl+F12 (zoom...).
/// Las sueltas de tecla y la pérdida de foco van a los DOS sitios: así
/// ninguno de los dos se queda con una tecla pegada.
#[must_use]
pub fn capture_events(events: &mut Vec<Event>) -> Vec<Event> {
    let mut stolen = Vec::new();
    let mut kept = Vec::with_capacity(events.len());
    for event in events.drain(..) {
        match &event {
            Event::Key { pressed: false, .. } | Event::WindowFocused(false) => {
                stolen.push(event.clone());
                kept.push(event);
            }
            Event::Key { key, modifiers, .. } => {
                if is_command(*modifiers) && *key != egui::Key::F12 {
                    kept.push(event);
                } else {
                    stolen.push(event);
                }
            }
            Event::Text(_) | Event::Paste(_) | Event::Copy => stolen.push(event),
            _ => kept.push(event),
        }
    }
    *events = kept;
    stolen
}

/// Registro de los campos de texto de la interfaz: mientras uno tiene el
/// foco no se captura el teclado (lo que se teclee es para el campo, no
/// para la PC-1500).
///
/// TODOS los `TextEdit` de la interfaz (rutas de archivos y cualquiera
/// que se añada) tienen que crearse con [`TextEditRegistry::add`] o
/// registrarse con [`TextEditRegistry::register`] en cada repintado, o
/// lo que se escriba en ellos llegará también al emulador.
#[derive(Debug, Default)]
pub struct TextEditRegistry {
    ids: Vec<egui::Id>,
}

impl TextEditRegistry {
    /// Al empezar cada repintado, antes de dibujar nada.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    pub fn register(&mut self, id: egui::Id) {
        self.ids.push(id);
    }

    /// `ui.add(edit)` y registrarlo.
    pub fn add(&mut self, ui: &mut egui::Ui, edit: egui::TextEdit<'_>) -> egui::Response {
        let response = ui.add(edit);
        self.register(response.id);
        response
    }

    /// `true` si, con el foco de egui en `focused`, el teclado es para la
    /// PC-1500 (ningún campo de texto registrado lo tiene).
    #[must_use]
    pub fn allows_capture(&self, focused: Option<egui::Id>) -> bool {
        focused.is_none_or(|id| !self.ids.contains(&id))
    }
}

/// `"("` o `"SHIFT + ("`.
#[must_use]
pub fn stroke_label(stroke: KeyStroke) -> String {
    if stroke.shift {
        format!("SHIFT + {}", stroke.key.label())
    } else {
        stroke.key.label().to_owned()
    }
}

/// La columna "PC-1500" de una fila de la ayuda.
#[must_use]
pub fn action_label(action: SpecialAction) -> String {
    match action {
        SpecialAction::Hold(stroke) => stroke_label(stroke),
        SpecialAction::Tap { strokes, .. } => {
            strokes.iter().map(|&s| stroke_label(s)).collect::<Vec<_>>().join(", ")
        }
        SpecialAction::Brk => Key::On.label().to_owned(),
    }
}

/// Cómo se llama un carácter del host en la ayuda.
fn char_label(c: char) -> String {
    if c == ' ' { "Espacio".to_owned() } else { c.to_string() }
}

/// La tabla de caracteres de la ayuda: cada ASCII imprimible con la
/// secuencia de la PC-1500 que lo teclea, o `None` si no tiene tecla.
#[must_use]
pub fn char_table() -> Vec<(String, Option<String>)> {
    (' '..='~').map(|c| (char_label(c), key_for_char(c).map(stroke_label))).collect()
}

/// Qué teclas del PC dan `key`, para la pista de cada tecla de la
/// pantalla: p. ej. `"Teclado PC: F9"`, o para `(`
/// `"Teclado PC: ( \u{b7} con SHIFT: <"`. Sale de [`SPECIAL`] y de la
/// tabla de caracteres. `None` si ninguna (no pasa con ninguna tecla,
/// lo comprueban los tests).
#[must_use]
pub fn hint(key: Key) -> Option<String> {
    let mut plain: Vec<String> = Vec::new();
    let mut shifted: Vec<String> = Vec::new();
    let mut add = |stroke: KeyStroke, label: String| {
        if stroke.key == key {
            let list = if stroke.shift { &mut shifted } else { &mut plain };
            if !list.contains(&label) {
                list.push(label);
            }
        }
    };
    for special in SPECIAL {
        match special.action {
            SpecialAction::Hold(stroke) | SpecialAction::Tap { strokes: &[stroke], .. } => {
                add(stroke, special.host_label.to_owned());
            }
            SpecialAction::Brk => add(KeyStroke::plain(Key::On), special.host_label.to_owned()),
            SpecialAction::Tap { .. } => {}
        }
    }
    if key == Key::On {
        add(KeyStroke::plain(Key::On), "Ctrl+C".to_owned());
    }
    for c in ' '..='~' {
        if c.is_ascii_lowercase() {
            continue; // la letra ya sale en mayúscula
        }
        if let Some(stroke) = key_for_char(c) {
            add(stroke, char_label(c));
        }
    }
    if plain.is_empty() && shifted.is_empty() {
        return None;
    }
    let mut text = format!("Teclado PC: {}", plain.join(" / "));
    if !shifted.is_empty() {
        text.push_str(" \u{b7} con SHIFT: ");
        text.push_str(&shifted.join(" / "));
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Key as E, Modifiers};

    fn key(key: E, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers }
    }

    fn text(t: &str) -> Event {
        Event::Text(t.to_owned())
    }

    fn id(k: E) -> HostKeyId {
        k as HostKeyId
    }

    const NONE: Modifiers = Modifiers::NONE;

    #[test]
    fn text_after_key_is_the_same_keystroke() {
        // Teclado español: Shift+8 = '('.
        let mut map = HostKeymap::default();
        let actions = map.translate(&[key(E::Num8, true, Modifiers::SHIFT), text("(")]);
        assert_eq!(
            actions,
            [HostAction::Down {
                id: id(E::Num8),
                stroke: KeyStroke::plain(Key::LeftParen),
                policy: ShiftPolicy::AddIfNeeded
            }]
        );
        assert_eq!(map.translate(&[key(E::Num8, false, NONE)]), [HostAction::Up { id: id(E::Num8) }]);
    }

    #[test]
    fn shifted_function_key_and_altgr() {
        let mut map = HostKeymap::default();
        assert_eq!(
            map.translate(&[key(E::F1, true, Modifiers::SHIFT)]),
            [HostAction::Down {
                id: id(E::F1),
                stroke: KeyStroke::shifted(Key::F1),
                policy: ShiftPolicy::AddIfNeeded
            }]
        );
        // AltGr+2 = '@' en Linux.
        assert_eq!(
            map.translate(&[key(E::Num2, true, Modifiers::ALT), text("@")]),
            [HostAction::Down {
                id: id(E::Num2),
                stroke: KeyStroke::shifted(Key::Equals),
                policy: ShiftPolicy::AddIfNeeded
            }]
        );
    }

    #[test]
    fn orphan_text_and_paste_are_typed() {
        let mut map = HostKeymap::default();
        assert_eq!(map.translate(&[text("*")]), [HostAction::TypeText("*".to_owned())]);
        assert_eq!(
            map.translate(&[Event::Paste("10 PRINT\r\n".to_owned())]),
            [HostAction::TypeText("10 PRINT\r\n".to_owned())]
        );
    }

    #[test]
    fn repeats_and_releases_are_paired_by_physical_key() {
        let mut map = HostKeymap::default();
        let down = map.translate(&[key(E::A, true, NONE), text("a")]);
        let stroke = KeyStroke::plain(Key::A);
        let policy = ShiftPolicy::AddIfNeeded;
        assert_eq!(down, [HostAction::Down { id: id(E::A), stroke, policy }], "may\u{fa}scula por defecto");
        // Repetición del SO (otra pulsación del mismo id, con su texto).
        assert_eq!(
            map.translate(&[key(E::A, true, NONE), text("a")]),
            [HostAction::Repeat { id: id(E::A), stroke, policy }]
        );
        // Suelta con otra tecla lógica pero la misma física.
        let release = Event::Key {
            key: E::Q,
            physical_key: Some(E::A),
            pressed: false,
            repeat: false,
            modifiers: NONE,
        };
        assert_eq!(map.translate(&[release]), [HostAction::Up { id: id(E::A) }]);
        assert!(map.translate(&[key(E::A, false, NONE)]).is_empty(), "ya suelta");
    }

    #[test]
    fn exact_case_uses_shift_for_lowercase() {
        let mut map = HostKeymap { exact_case: true, ..HostKeymap::default() };
        assert_eq!(
            map.translate(&[key(E::B, true, NONE), text("b")]),
            [HostAction::Down { id: id(E::B), stroke: KeyStroke::shifted(Key::B), policy: ShiftPolicy::Exact }]
        );
    }

    #[test]
    fn special_keys() {
        let mut map = HostKeymap::default();
        let ctrl = Modifiers::CTRL;
        assert_eq!(
            map.translate(&[Event::Copy]),
            [HostAction::Brk { id: COPY_ID, down: true }, HostAction::Brk { id: COPY_ID, down: false }]
        );
        assert_eq!(map.translate(&[key(E::F12, true, ctrl)]), [HostAction::Tap(vec![KeyStroke::plain(Key::Off)])]);
        assert!(map.translate(&[key(E::F12, true, ctrl)]).is_empty(), "OFF no se repite");
        assert!(map.translate(&[key(E::F12, false, NONE)]).is_empty());
        assert_eq!(
            map.translate(&[key(E::Backspace, true, NONE)]),
            [HostAction::Tap(vec![KeyStroke::plain(Key::Left), KeyStroke::shifted(Key::Left)])]
        );
        assert_eq!(
            map.translate(&[key(E::Backspace, true, NONE)]),
            [HostAction::TapRepeat(vec![KeyStroke::plain(Key::Left), KeyStroke::shifted(Key::Left)])],
            "Retroceso s\u{ed} se repite"
        );
        assert_eq!(
            map.translate(&[key(E::Escape, true, NONE), key(E::Escape, false, NONE)]),
            [HostAction::Brk { id: id(E::Escape), down: true }, HostAction::Brk { id: id(E::Escape), down: false }]
        );
        let home = map.translate(&[key(E::Home, true, Modifiers::SHIFT)]);
        assert!(
            matches!(home.as_slice(), [HostAction::Down { stroke, .. }] if *stroke == KeyStroke::shifted(Key::Cl)),
            "Shift+Inicio = CA: {home:?}"
        );
        // Shift+F8 no tiene fila propia: es F8.
        let sml = map.translate(&[key(E::F8, true, Modifiers::SHIFT)]);
        assert!(matches!(sml.as_slice(), [HostAction::Down { stroke, .. }] if *stroke == KeyStroke::plain(Key::Sml)));
        // Otros atajos con Ctrl no son para la PC-1500.
        assert!(map.translate(&[key(E::Plus, true, ctrl)]).is_empty());
        assert_eq!(map.translate(&[Event::WindowFocused(false)]), [HostAction::ReleaseAll]);
    }

    #[test]
    fn unmappable_char_is_reported() {
        let mut map = HostKeymap::default();
        assert_eq!(map.translate(&[key(E::Minus, true, Modifiers::SHIFT), text("_")]), [HostAction::Ignored('_')]);
        assert!(map.translate(&[key(E::Minus, false, NONE)]).is_empty());
    }

    #[test]
    fn every_printable_char_maps_or_is_ignored() {
        for c in ' '..='~' {
            let mut map = HostKeymap::default();
            let actions = map.translate(&[key(E::Num0, true, NONE), text(&c.to_string())]);
            let ok = match actions.as_slice() {
                [HostAction::Down { .. }] => key_for_char(c).is_some(),
                [HostAction::Ignored(ignored)] => *ignored == c && key_for_char(c).is_none(),
                _ => false,
            };
            assert!(ok, "{c:?}: {actions:?}");
        }
    }

    #[test]
    fn capture_keeps_ctrl_shortcuts_and_copies_releases() {
        let mut events = vec![
            key(E::Tab, true, NONE),
            text("a"),
            key(E::Plus, true, Modifiers::CTRL),
            key(E::F12, true, Modifiers::CTRL),
            key(E::Tab, false, NONE),
            Event::Paste("x".to_owned()),
            Event::Copy,
            Event::WindowFocused(false),
            Event::PointerGone,
        ];
        let stolen = capture_events(&mut events);
        assert_eq!(
            stolen,
            [
                key(E::Tab, true, NONE),
                text("a"),
                key(E::F12, true, Modifiers::CTRL),
                key(E::Tab, false, NONE),
                Event::Paste("x".to_owned()),
                Event::Copy,
                Event::WindowFocused(false),
            ]
        );
        assert_eq!(
            events,
            [
                key(E::Plus, true, Modifiers::CTRL),
                key(E::Tab, false, NONE),
                Event::WindowFocused(false),
                Event::PointerGone,
            ]
        );
    }

    #[test]
    fn text_edit_registry_excludes_registered_fields() {
        let mut registry = TextEditRegistry::default();
        let field = egui::Id::new("ruta");
        let button = egui::Id::new("boton");
        registry.register(field);
        assert!(registry.allows_capture(None));
        assert!(registry.allows_capture(Some(button)));
        assert!(!registry.allows_capture(Some(field)));
        registry.clear();
        assert!(registry.allows_capture(Some(field)));
    }

    #[test]
    fn hold_policy_switches_above_real_time() {
        assert_eq!(hold_policy(false, 0.02), HoldPolicy::Hold);
        assert_eq!(hold_policy(false, 1.0), HoldPolicy::Hold);
        assert_eq!(hold_policy(false, 2.0), HoldPolicy::Tap);
        assert_eq!(hold_policy(true, 0.5), HoldPolicy::Tap);
    }

    #[test]
    fn every_key_has_a_hint() {
        for k in Key::ALL {
            assert!(hint(k).is_some(), "{k:?}");
        }
        assert_eq!(hint(Key::Rsv).as_deref(), Some("Teclado PC: F9"));
        assert_eq!(hint(Key::Off).as_deref(), Some("Teclado PC: Ctrl+F12"));
        assert_eq!(hint(Key::LeftParen).as_deref(), Some("Teclado PC: ( \u{b7} con SHIFT: <"));
        assert_eq!(hint(Key::On).as_deref(), Some("Teclado PC: Esc / F12 / Fin / Ctrl+C"));
        assert_eq!(hint(Key::Cl).as_deref(), Some("Teclado PC: Inicio \u{b7} con SHIFT: Shift+Inicio"));
    }

    #[test]
    fn help_tables_cover_everything() {
        let chars = char_table();
        assert_eq!(chars.len(), 95);
        assert_eq!(chars.iter().filter(|(_, seq)| seq.is_none()).count(), 8, "' \\ _ ` {{ | }} ~");
        assert_eq!(action_label(SpecialAction::Tap { strokes: BACKSPACE, repeat: true }), "\u{2190}, SHIFT + \u{2190}");
        // Ninguna fila de SPECIAL repetida (misma tecla y modificadores).
        for (i, a) in SPECIAL.iter().enumerate() {
            for b in &SPECIAL[i + 1..] {
                assert!(!(a.key == b.key && a.mods == b.mods), "{}", a.host_label);
            }
        }
    }

    /// Con el emulador de verdad: a 1x una tecla se mantiene y su suelta
    /// cuenta; a más de 1x cada pulsación es un toque fijo y las sueltas
    /// se ignoran.
    #[test]
    fn apply_follows_the_hold_policy() {
        let stroke = KeyStroke::plain(Key::Left);
        let policy = ShiftPolicy::AddIfNeeded;

        let mut pc = Pc1500::new_deterministic();
        apply(&mut pc, &HostAction::Down { id: 1, stroke, policy }, HoldPolicy::Hold);
        apply(&mut pc, &HostAction::Repeat { id: 1, stroke, policy }, HoldPolicy::Hold);
        assert_eq!(pc.pending_keystrokes(), 1, "la repetici\u{f3}n la hace la ROM");
        pc.run_frames(30);
        assert!(pc.is_key_pressed(Key::Left), "mantenida");
        apply(&mut pc, &HostAction::Up { id: 1 }, HoldPolicy::Hold);
        pc.run_frames(1);
        assert!(!pc.is_key_pressed(Key::Left));

        let mut fast = Pc1500::new_deterministic();
        apply(&mut fast, &HostAction::Down { id: 1, stroke, policy }, HoldPolicy::Tap);
        apply(&mut fast, &HostAction::Repeat { id: 1, stroke, policy }, HoldPolicy::Tap);
        assert_eq!(fast.pending_keystrokes(), 1, "repetici\u{f3}n con cola: descartada");
        fast.run_frames(30);
        assert_eq!(fast.pending_keystrokes(), 0, "se sueltan solas");
        assert!(!fast.is_key_pressed(Key::Left));
        apply(&mut fast, &HostAction::Repeat { id: 1, stroke, policy }, HoldPolicy::Tap);
        assert_eq!(fast.pending_keystrokes(), 1, "sin cola, cada repetici\u{f3}n es un toque");
        apply(&mut fast, &HostAction::Up { id: 1 }, HoldPolicy::Tap);
        fast.run_frames(30);
        assert_eq!(fast.pending_keystrokes(), 0);

        assert_eq!(
            apply(&mut fast, &HostAction::TypeText("A_".to_owned()), HoldPolicy::Tap).as_deref(),
            Some("car\u{e1}cter sin tecla en la PC-1500: _")
        );
        apply(&mut fast, &HostAction::Brk { id: 7, down: true }, HoldPolicy::Tap);
        assert_eq!(fast.pending_keystrokes(), 0, "BRK cancela el texto");
        fast.run_frames(3);
        assert!(!fast.is_key_pressed(Key::On), "a m\u{e1}s de 1x, BRK es un toque");
    }

    /// Retroceso mantenido 2 s a 1x (primera repetición del SO a los
    /// 500 ms y luego cada ~33 ms, es decir, cada 3 frames de ~11,5 ms):
    /// la cola del alimentador no crece y, al soltar, la máquina deja de
    /// borrar enseguida en vez de seguir comiéndose la línea.
    #[test]
    fn held_backspace_does_not_flood_the_feeder() {
        const LINE: &str = "ABCDEFGHIJKLMNOPQRST";
        for policy in [HoldPolicy::Hold, HoldPolicy::Tap] {
            let mut pc = Pc1500::new_deterministic();
            assert!(pc.boot().is_some(), "la ROM no lleg\u{f3} al prompt");
            assert_eq!(pc.type_text(LINE), Ok(()), "letras");
            assert!(pc.lcd_text().contains(LINE), "{:?}", pc.lcd_text());

            let mut map = HostKeymap::default();
            let mut press = |machine: &mut Pc1500, pressed: bool| {
                for action in map.translate(&[key(E::Backspace, pressed, Modifiers::NONE)]) {
                    apply(machine, &action, policy);
                }
            };
            press(&mut pc, true);
            pc.run_frames(43);
            let mut repeats = 0;
            while repeats < 43 {
                press(&mut pc, true);
                assert!(pc.pending_keystrokes() <= 2, "cola acotada: {}", pc.pending_keystrokes());
                pc.run_frames(3);
                repeats += 1;
            }
            press(&mut pc, false);
            assert!(pc.pending_keystrokes() <= 2, "como mucho el retroceso en curso");

            let mut drain = 0;
            while pc.pending_keystrokes() > 0 {
                assert!(drain < 30, "sigue borrando tras soltar");
                pc.run_frames(1);
                drain += 1;
            }
            pc.run_frames(10);
            let left = pc.lcd_text().chars().filter(char::is_ascii_uppercase).count();
            let deleted = LINE.len() - left;
            // 172 frames a ~24 frames por retroceso: unos 7 u 8.
            assert!((3..=10).contains(&deleted), "{policy:?}: borrados {deleted}: {:?}", pc.lcd_text());
        }
    }
}
