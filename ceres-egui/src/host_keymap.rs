//! From the PC's keyboard (egui's events) to PC-1500 keys.
//!
//! Everything here is pure logic, without GUI, so that it can be tested with lists of synthetic
//! events: [`HostKeymap::translate`] turns events into [`HostAction`]s, and [`apply`] passes them
//! to the emulator's key feeder (`Pc1500::host_key_down`...), which is what presses and releases in
//! EMULATED time. The GUI (`pc1500_app.rs`) only takes the keyboard events before egui sees them
//! ([`capture_events`], from `raw_input_hook`) and paints.
//!
//! # Characters by text, the rest by table
//! Printable characters are taken from the `Event::Text` that egui-winit sends right after the
//! `Event::Key` of the same key press, not from the key: so the host keyboard's layout makes no
//! difference (on a Spanish keyboard `(` is Shift+8, and egui does not even have an `egui::Key`
//! variant for `(`). What is not text (arrows, F1..., Esc...) comes from the fixed table
//! [`SPECIAL`]. The host's `SHIFT` is not the PC-1500's `SHIFT` (which is a prefix, see
//! `ceres_core::KeyStroke`): it only decides which character the host produces, and the
//! Shift+special key combinations of [`SPECIAL`]. The PC-1500's `SHIFT` is F11 or the key on
//! screen.
//!
//! The GUI's help (the "Keyboard" window) and the hints of each key ([`hint`]) are generated from
//! the same tables, so they cannot get out of step with the real behaviour.

use std::collections::HashMap;

use ceres_core::{
    HostKeyId, Key, KeyStroke, Pc1500, ShiftPolicy, harness::key_for_char,
};
use eframe::egui::{self, Event};

/// Identifier of "host key" for the mouse on the keys on screen. Outside the range of
/// `egui::Key as u32`.
pub const MOUSE_ID: HostKeyId = 0xFFFF_0000;

/// Identifier for the `ON/BRK` of Ctrl+C (`Event::Copy`, which brings no `Event::Key`).
pub const COPY_ID: HostKeyId = 0xFFFF_0001;

/// Modifiers with which a row of [`SPECIAL`] is looked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mods {
    /// Without Ctrl (with or without Shift if there is no `Shift` row of its own).
    None,
    /// Shift without Ctrl.
    Shift,
    /// Ctrl (or Cmd).
    Ctrl,
}

/// What a special key of the host does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialAction {
    /// A PC-1500 key that is held as long as it is held on the host (see `Pc1500::host_key_down`).
    Hold(KeyStroke),
    /// One-off key presses of fixed length. With `repeat`, the operating system's key repeat
    /// repeats them (Backspace).
    Tap { strokes: &'static [KeyStroke], repeat: bool },
    /// `ON/BRK`.
    Brk,
}

/// A row of the table of special keys.
#[derive(Clone, Copy, Debug)]
pub struct Special {
    pub key: egui::Key,
    pub mods: Mods,
    /// What the host's key is called in the help.
    pub host_label: &'static str,
    pub action: SpecialAction,
    /// Explanation for the help (it can be empty).
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

/// The host's keys that are not text (or are, but with another meaning), in the order they appear
/// in the help. The final table, decided with the user: Esc = `ON/BRK`, `OFF` only with Ctrl+F12,
/// Home = `CL`, Backspace = really delete.
pub const SPECIAL: &[Special] = {
    use KeyStroke as S;
    use egui::Key as E;
    use Key as P;
    use Mods::{Ctrl, None as No, Shift};
    &[
        brk(E::Escape, No, "Esc", "BREAK; switches the machine on when it is off"),
        brk(E::F12, No, "F12", "the same as Esc"),
        brk(E::End, No, "End", "the same as Esc"),
        Special {
            key: E::F12,
            mods: Ctrl,
            host_label: "Ctrl+F12",
            action: SpecialAction::Tap { strokes: OFF, repeat: false },
            note: "switches off (only with Ctrl, so as not to press it by mistake)",
        },
        hold(E::Home, No, "Home", S::plain(P::Cl), "clears the line / the error"),
        hold(E::Home, Shift, "Shift+Home", S::shifted(P::Cl), "CA: clears everything"),
        hold(E::Enter, No, "Enter", S::plain(P::Enter), ""),
        Special {
            key: E::Backspace,
            mods: No,
            host_label: "Backspace",
            action: SpecialAction::Tap { strokes: BACKSPACE, repeat: true },
            note: "deletes to the left (\u{2190} and then DEL)",
        },
        hold(E::Delete, No, "Delete", S::shifted(P::Left), "DEL: deletes under the cursor (no key repeat, as the real one)"),
        hold(E::Insert, No, "Insert", S::shifted(P::Right), "INS: opens a gap"),
        hold(E::ArrowLeft, No, "\u{2190}", S::plain(P::Left), "with the ROM's key repeat"),
        hold(E::ArrowRight, No, "\u{2192}", S::plain(P::Right), ""),
        hold(E::ArrowUp, No, "\u{2191}", S::plain(P::Up), ""),
        hold(E::ArrowDown, No, "\u{2193}", S::plain(P::Down), ""),
        hold(E::ArrowLeft, Shift, "Shift+\u{2190}", S::shifted(P::Left), "DEL"),
        hold(E::ArrowRight, Shift, "Shift+\u{2192}", S::shifted(P::Right), "INS"),
        hold(E::ArrowUp, Shift, "Shift+\u{2191}", S::shifted(P::Up), "[ (the LCD shows \u{221a})"),
        hold(E::ArrowDown, Shift, "Shift+\u{2193}", S::shifted(P::Down), "] (the LCD shows \u{3c0})"),
        hold(E::PageUp, No, "Page Up", S::plain(P::Up), ""),
        hold(E::PageDown, No, "Page Down", S::plain(P::Down), ""),
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
        hold(E::F7, No, "F7", S::plain(P::Control), "DEF (prefix)"),
        hold(E::F8, No, "F8", S::plain(P::Sml), "SML: lowercase on/off"),
        hold(E::F9, No, "F9", S::plain(P::Rsv), "RSV: changes the layer of reserved keys"),
        hold(E::F10, No, "F10", S::plain(P::Rcl), ""),
        Special {
            key: E::F11,
            mods: No,
            host_label: "F11",
            action: SpecialAction::Tap { strokes: SHIFT_PREFIX, repeat: false },
            note: "SHIFT of the PC-1500 (prefix: it affects the next key)",
        },
    ]
};

/// Extra rows of the help that are not keys of [`SPECIAL`] (they come as other egui events):
/// `(host key, effect)`.
pub const EXTRA_HELP: &[(&str, &str)] = &[
    ("Ctrl+C", "ON/BRK (BREAK), as in a terminal"),
    ("Ctrl+V", "types the clipboard's text (exactly, upper and lower case included)"),
];

/// Looks for the row of [`SPECIAL`] of a key with its modifiers. With Ctrl only the `Ctrl` rows
/// count; with Shift, the `Shift` row if there is one and otherwise the normal one (Shift+F8 = F8).
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

/// Ctrl (or Cmd on macOS): the shortcuts left to egui (zoom...).
const fn is_command(modifiers: egui::Modifiers) -> bool {
    modifiers.ctrl || modifiers.command || modifiers.mac_cmd
}

/// What has to be done in the emulator because of a host event (see [`apply`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostAction {
    /// Physical key pressed: hold `stroke` until its [`HostAction::Up`].
    Down { id: HostKeyId, stroke: KeyStroke, policy: ShiftPolicy },
    /// The operating system's key repeat of a held key.
    Repeat { id: HostKeyId, stroke: KeyStroke, policy: ShiftPolicy },
    /// Physical key released.
    Up { id: HostKeyId },
    /// One-off key presses of fixed length.
    Tap(Vec<KeyStroke>),
    /// The operating system's key repeat of a key that translates into taps (Backspace). It is
    /// different from [`HostAction::Tap`] because [`apply`] only queues it if the feeder has
    /// nothing pending: a backspace is two taps (`←` and `SHIFT` + `←`, some 24 emulated frames,
    /// ~276 ms at 1x) and the OS repeats every ~33 ms, so queuing them all the queue would grow
    /// without limit and the machine would go on deleting seconds after the key was released.
    TapRepeat(Vec<KeyStroke>),
    /// Text to type (pasted, or characters that come without a key).
    TypeText(String),
    /// `ON/BRK` pressed (`down`) or released.
    Brk { id: HostKeyId, down: bool },
    /// Release everything (the window lost focus).
    ReleaseAll,
    /// A character that has no key on the PC-1500.
    Ignored(char),
}

/// What is remembered of a host key while it is down: to release it even if the modifiers change
/// when releasing it, and to know what to do with its key repeat.
#[derive(Clone, Copy, Debug)]
enum Held {
    Live(KeyStroke, ShiftPolicy),
    Tap(&'static [KeyStroke]),
    Brk,
    Nothing,
}

/// The event translator (with the state of which physical keys are still down, which has to be kept
/// by hand because the events are taken before egui processes them, see [`capture_events`]).
#[derive(Debug, Default)]
pub struct HostKeymap {
    down: HashMap<HostKeyId, Held>,
    /// The "Match upper/lower case" checkbox: letters come out with the host's case (lowercase =
    /// `SHIFT` + letter, making up for `SML`). Off (the default), each letter is the PC-1500 key
    /// and nothing more, which is uppercase except with `SML`: what BASIC's keywords want.
    exact_case: bool,
}

impl HostKeymap {
    /// The "Match upper/lower case" checkbox (see `exact_case`).
    pub const fn exact_case_mut(&mut self) -> &mut bool {
        &mut self.exact_case
    }

    /// Translates the events of a repaint. See the module's comment.
    ///
    /// - `Event::Key` pressed followed by `Event::Text`: it is the SAME key press (egui-winit sends
    ///   them so); the text is taken from there and consumed, so as not to type it twice.
    /// - The `id` of a key is its physical key (`physical_key`, or the logical one if there is
    ///   none): the release is paired by it.
    /// - A press of an `id` that is already down is the operating system's key repeat (egui-winit
    ///   always sends `repeat: false` and it is egui that fills it in, but the events are taken
    ///   from it before).
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
                        // Repeat of a key pressed before we started capturing: it is not ours.
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
            // Shift alone, a dead key (the character comes with the next key press)...
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

    /// The key press of a character typed live (see `exact_case`).
    fn char_stroke(&self, c: char) -> Option<(KeyStroke, ShiftPolicy)> {
        if c.is_ascii_alphabetic() && !self.exact_case {
            let stroke = key_for_char(c.to_ascii_uppercase())?;
            return Some((stroke, ShiftPolicy::AddIfNeeded));
        }
        let policy = if c.is_ascii_alphabetic() { ShiftPolicy::Exact } else { ShiftPolicy::AddIfNeeded };
        key_for_char(c).map(|stroke| (stroke, policy))
    }
}

/// See [`HostKeymap::translate`].
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

/// How the keys are held depending on the speed (the user's decision).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldPolicy {
    /// Up to 1x: a host key is held on the PC-1500 as long as it is physically held (with a minimum
    /// in emulated time); the key repeat is done by the ROM, as on the real machine.
    Hold,
    /// Above 1x (and in Max mode): each key press of the host, the operating system's key repeats
    /// included, is a key press of fixed length in emulated time, and the releases are ignored.
    /// Otherwise, at 50x an arrow held for an instant would repeat itself dozens of times.
    Tap,
}

/// See [`HoldPolicy`].
#[must_use]
pub fn hold_policy(max_speed: bool, multiplier: f64) -> HoldPolicy {
    // Margin so that the 1.00x of the logarithmic slider (which can end up at 1.0000001) is still
    // "real time".
    if max_speed || multiplier > 1.0 + 1e-6 { HoldPolicy::Tap } else { HoldPolicy::Hold }
}

/// Applies an action to the emulator. It returns a message for the status line if there is
/// something to tell (characters with no key).
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
            // Up to 1x the key repeat is done by the ROM itself. Above it, each repeat of the OS is
            // a tap, but only if the feeder has nothing pending any more: each tap lasts 8 emulated
            // frames (~46 ms of host time at 2x) and the OS repeats every ~33 ms, so without this a
            // queue would build up and the key would go on "repeating" for a while after it was
            // released.
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
            // With any policy: see [`HostAction::TapRepeat`]. When the key is released, at most the
            // backspace in progress remains.
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

/// Message for characters that could not be typed.
fn skipped_message(skipped: &[char]) -> Option<String> {
    match skipped {
        [] => None,
        [c] => Some(format!("character with no key on the PC-1500: {}", show_char(*c))),
        many => {
            let list: Vec<String> = many.iter().map(|&c| show_char(c)).collect();
            Some(format!("characters with no key on the PC-1500 (left out): {}", list.join(" ")))
        }
    }
}

fn show_char(c: char) -> String {
    if c.is_control() { c.escape_default().collect() } else { c.to_string() }
}

/// Takes out of `events` the keyboard events that are for the emulator and returns them (it is
/// called from `raw_input_hook`, before egui processes them, only if the keyboard is being
/// captured).
///
/// So egui does not see Tab nor the arrows (which would move the focus between controls) nor
/// Space/Enter (which would click the button with the focus). The shortcuts with Ctrl/Cmd stay with
/// egui except Ctrl+F12 (zoom...). Key releases and the loss of focus go to BOTH places: so neither
/// of the two keeps a key stuck.
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

/// Register of the GUI's text fields: while one has the focus the keyboard is not captured (what is
/// typed is for the field, not for the PC-1500).
///
/// ALL the GUI's `TextEdit`s (file paths and any added) have to be created with
/// [`TextEditRegistry::add`] or registered with [`TextEditRegistry::register`] at each repaint, or
/// what is typed in them will also reach the emulator.
#[derive(Debug, Default)]
pub struct TextEditRegistry {
    ids: Vec<egui::Id>,
}

impl TextEditRegistry {
    /// At the start of each repaint, before drawing anything.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    pub fn register(&mut self, id: egui::Id) {
        self.ids.push(id);
    }

    /// `ui.add(edit)` and register it.
    pub fn add(&mut self, ui: &mut egui::Ui, edit: egui::TextEdit<'_>) -> egui::Response {
        let response = ui.add(edit);
        self.register(response.id);
        response
    }

    /// `true` if, with egui's focus at `focused`, the keyboard is for the PC-1500 (no registered
    /// text field has it).
    #[must_use]
    pub fn allows_capture(&self, focused: Option<egui::Id>) -> bool {
        focused.is_none_or(|id| !self.ids.contains(&id))
    }
}

/// `"("` or `"SHIFT + ("`.
#[must_use]
pub fn stroke_label(stroke: KeyStroke) -> String {
    if stroke.shift {
        format!("SHIFT + {}", stroke.key.label())
    } else {
        stroke.key.label().to_owned()
    }
}

/// The "PC-1500" column of a row of the help.
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

/// What a host character is called in the help.
fn char_label(c: char) -> String {
    if c == ' ' { "Space".to_owned() } else { c.to_string() }
}

/// The help's table of characters: each printable ASCII character with the PC-1500 sequence that
/// types it, or `None` if it has no key.
#[must_use]
pub fn char_table() -> Vec<(String, Option<String>)> {
    (' '..='~').map(|c| (char_label(c), key_for_char(c).map(stroke_label))).collect()
}

/// Which PC keys give `key`, for the hint of each key on screen: e.g. `"PC keyboard: F9"`, or for
/// `(` `"PC keyboard: ( \u{b7} with SHIFT: <"`. It comes from [`SPECIAL`] and from the table of
/// characters. `None` if none (it does not happen with any key, the tests check it).
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
    let mut text = format!("PC keyboard: {}", plain.join(" / "));
    if !shifted.is_empty() {
        text.push_str(" \u{b7} with SHIFT: ");
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
        // Spanish keyboard: Shift+8 = '('.
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
        // AltGr+2 = '@' on Linux.
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
        assert_eq!(down, [HostAction::Down { id: id(E::A), stroke, policy }], "uppercase by default");
        // OS key repeat (another press of the same id, with its text).
        assert_eq!(
            map.translate(&[key(E::A, true, NONE), text("a")]),
            [HostAction::Repeat { id: id(E::A), stroke, policy }]
        );
        // Release with another logical key but the same physical one.
        let release = Event::Key {
            key: E::Q,
            physical_key: Some(E::A),
            pressed: false,
            repeat: false,
            modifiers: NONE,
        };
        assert_eq!(map.translate(&[release]), [HostAction::Up { id: id(E::A) }]);
        assert!(map.translate(&[key(E::A, false, NONE)]).is_empty(), "already released");
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
        assert!(map.translate(&[key(E::F12, true, ctrl)]).is_empty(), "OFF does not repeat");
        assert!(map.translate(&[key(E::F12, false, NONE)]).is_empty());
        assert_eq!(
            map.translate(&[key(E::Backspace, true, NONE)]),
            [HostAction::Tap(vec![KeyStroke::plain(Key::Left), KeyStroke::shifted(Key::Left)])]
        );
        assert_eq!(
            map.translate(&[key(E::Backspace, true, NONE)]),
            [HostAction::TapRepeat(vec![KeyStroke::plain(Key::Left), KeyStroke::shifted(Key::Left)])],
            "Backspace does repeat"
        );
        assert_eq!(
            map.translate(&[key(E::Escape, true, NONE), key(E::Escape, false, NONE)]),
            [HostAction::Brk { id: id(E::Escape), down: true }, HostAction::Brk { id: id(E::Escape), down: false }]
        );
        let home = map.translate(&[key(E::Home, true, Modifiers::SHIFT)]);
        assert!(
            matches!(home.as_slice(), [HostAction::Down { stroke, .. }] if *stroke == KeyStroke::shifted(Key::Cl)),
            "Shift+Home = CA: {home:?}"
        );
        // Shift+F8 has no row of its own: it is F8.
        let sml = map.translate(&[key(E::F8, true, Modifiers::SHIFT)]);
        assert!(matches!(sml.as_slice(), [HostAction::Down { stroke, .. }] if *stroke == KeyStroke::plain(Key::Sml)));
        // Other shortcuts with Ctrl are not for the PC-1500.
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
        let button = egui::Id::new("button");
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
        assert_eq!(hint(Key::Rsv).as_deref(), Some("PC keyboard: F9"));
        assert_eq!(hint(Key::Off).as_deref(), Some("PC keyboard: Ctrl+F12"));
        assert_eq!(hint(Key::LeftParen).as_deref(), Some("PC keyboard: ( \u{b7} with SHIFT: <"));
        assert_eq!(hint(Key::On).as_deref(), Some("PC keyboard: Esc / F12 / End / Ctrl+C"));
        assert_eq!(hint(Key::Cl).as_deref(), Some("PC keyboard: Home \u{b7} with SHIFT: Shift+Home"));
    }

    #[test]
    fn help_tables_cover_everything() {
        let chars = char_table();
        assert_eq!(chars.len(), 95);
        assert_eq!(chars.iter().filter(|(_, seq)| seq.is_none()).count(), 8, "' \\ _ ` {{ | }} ~");
        assert_eq!(action_label(SpecialAction::Tap { strokes: BACKSPACE, repeat: true }), "\u{2190}, SHIFT + \u{2190}");
        // No row of SPECIAL repeated (same key and modifiers).
        for (i, a) in SPECIAL.iter().enumerate() {
            for b in &SPECIAL[i + 1..] {
                assert!(!(a.key == b.key && a.mods == b.mods), "{}", a.host_label);
            }
        }
    }

    /// With the real emulator: at 1x a key is held and its release counts; above 1x each key press
    /// is a fixed tap and the releases are ignored.
    #[test]
    fn apply_follows_the_hold_policy() {
        let stroke = KeyStroke::plain(Key::Left);
        let policy = ShiftPolicy::AddIfNeeded;

        let mut pc = Pc1500::new_deterministic();
        apply(&mut pc, &HostAction::Down { id: 1, stroke, policy }, HoldPolicy::Hold);
        apply(&mut pc, &HostAction::Repeat { id: 1, stroke, policy }, HoldPolicy::Hold);
        assert_eq!(pc.pending_keystrokes(), 1, "the ROM does the repeat");
        pc.run_frames(30);
        assert!(pc.is_key_pressed(Key::Left), "held");
        apply(&mut pc, &HostAction::Up { id: 1 }, HoldPolicy::Hold);
        pc.run_frames(1);
        assert!(!pc.is_key_pressed(Key::Left));

        let mut fast = Pc1500::new_deterministic();
        apply(&mut fast, &HostAction::Down { id: 1, stroke, policy }, HoldPolicy::Tap);
        apply(&mut fast, &HostAction::Repeat { id: 1, stroke, policy }, HoldPolicy::Tap);
        assert_eq!(fast.pending_keystrokes(), 1, "repeat with a queue: dropped");
        fast.run_frames(30);
        assert_eq!(fast.pending_keystrokes(), 0, "they release by themselves");
        assert!(!fast.is_key_pressed(Key::Left));
        apply(&mut fast, &HostAction::Repeat { id: 1, stroke, policy }, HoldPolicy::Tap);
        assert_eq!(fast.pending_keystrokes(), 1, "without a queue, each repeat is a tap");
        apply(&mut fast, &HostAction::Up { id: 1 }, HoldPolicy::Tap);
        fast.run_frames(30);
        assert_eq!(fast.pending_keystrokes(), 0);

        assert_eq!(
            apply(&mut fast, &HostAction::TypeText("A_".to_owned()), HoldPolicy::Tap).as_deref(),
            Some("character with no key on the PC-1500: _")
        );
        apply(&mut fast, &HostAction::Brk { id: 7, down: true }, HoldPolicy::Tap);
        assert_eq!(fast.pending_keystrokes(), 0, "BRK cancels the text");
        fast.run_frames(3);
        assert!(!fast.is_key_pressed(Key::On), "above 1x, BRK is a tap");
    }

    /// Backspace held for 2 s at 1x (first OS repeat at 500 ms and then every ~33 ms, that is,
    /// every 3 frames of ~11.5 ms): the feeder's queue does not grow and, on release, the machine
    /// stops deleting at once instead of going on eating the line.
    #[test]
    fn held_backspace_does_not_flood_the_feeder() {
        const LINE: &str = "ABCDEFGHIJKLMNOPQRST";
        for policy in [HoldPolicy::Hold, HoldPolicy::Tap] {
            let mut pc = Pc1500::new_deterministic();
            assert!(pc.boot().is_some(), "the ROM did not reach the prompt");
            assert_eq!(pc.type_text(LINE), Ok(()), "letters");
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
                assert!(pc.pending_keystrokes() <= 2, "bounded queue: {}", pc.pending_keystrokes());
                pc.run_frames(3);
                repeats += 1;
            }
            press(&mut pc, false);
            assert!(pc.pending_keystrokes() <= 2, "at most the backspace in progress");

            let mut drain = 0;
            while pc.pending_keystrokes() > 0 {
                assert!(drain < 30, "it goes on deleting after release");
                pc.run_frames(1);
                drain += 1;
            }
            pc.run_frames(10);
            let left = pc.lcd_text().chars().filter(char::is_ascii_uppercase).count();
            let deleted = LINE.len() - left;
            // 172 frames at ~24 frames per backspace: some 7 or 8.
            assert!((3..=10).contains(&deleted), "{policy:?}: deleted {deleted}: {:?}", pc.lcd_text());
        }
    }
}
