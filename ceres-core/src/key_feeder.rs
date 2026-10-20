//! Key feeder: turns what the host's keyboard does (or the mouse on the keys on screen, or a pasted
//! text) into presses and releases of PC-1500 keys measured in EMULATED TIME.
//!
//! # Why it is needed
//! The ROM does not see "key events", it sees the matrix pressed or not at each wake-up of its wait
//! loop (`WAIT_4_KB`, $E243) and has its own debouncing: a key has to be down for several frames
//! and then the keyboard empty for as many before the next one (see [`KEY_HOLD_FRAMES`] and
//! [`KEY_RELEASE_FRAMES`], measured with the harness). The GUI used to press and release the keys
//! with a timer of 150 ms of HOST time: at 0.05x that is ~0.65 emulated frames and the ROM lost the
//! keys; at 50x, dozens of frames. Here everything is counted in emulated frames
//! ([`crate::TICKS_PER_FRAME`] cycles), so a key press lasts the same for the ROM at any speed,
//! paused (it does not move on) or in the tests.
//!
//! # How time is measured
//! [`KeyFeeder::tick`] moves the state machine one step per emulated frame.
//! `Pc1500::run_frame_hooks` calls it from inside [`Pc1500::run_for_ticks`] each time
//! [`crate::TICKS_PER_FRAME`] cycles are up, NOT once per call: the GUI calls `run_for_ticks` with
//! budgets of any size (~433 cycles per repaint at 0.02x, millions in Max mode) and the pace of the
//! keys must not depend on that.
//!
//! # Two kinds of steps
//! - [`StepKind::Live`]: a physical key of the host (or of the mouse). It starts as soon as its
//!   turn comes, without waiting for the ROM, like a real finger, and stays down WHILE the host
//!   keeps it down (at least [`KEY_HOLD_FRAMES`]): that is how the ROM's own key repeat on the
//!   arrows and the games with `INKEY$` work. If another live key comes in the meantime, the new
//!   one wins ("rollover", as on PC keyboards).
//! - [`StepKind::Typed`]: a character of a pasted text ([`Pc1500::queue_text`]). A key press of
//!   minimum length, and before each one it waits for the ROM to be ready for the next key
//!   ([`Pc1500::is_ready_for_key`]) for 2 frames in a row, or at most
//!   [`TYPED_IDLE_TIMEOUT_FRAMES`]: so a pasted listing survives the time the ROM takes to process
//!   each ENTER.
//!
//! `ON/BRK` does not go through the queue: it is not in the matrix (it goes to PB7, see
//! `Keyboard::on_pulse`) and it is precisely the key that interrupts whatever is going on, so it is
//! pressed at once and also cancels the pasted text pending.
//!
//! The feeder only touches the keys it presses itself: what is pressed with
//! [`Pc1500::press`]/[`Pc1500::release`] (e.g. the harness's synchronous `tap`) it neither sees nor
//! releases. Do not mix both at the same time.

use std::collections::VecDeque;

use crate::{
    Key, Pc1500,
    harness::{KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES},
    keyboard::{KeyStroke, Keyboard, key_for_char},
};

/// Identifier of a physical key of the host.
///
/// The GUI chooses it: e.g. the physical key's code, or one reserved for the mouse. It is used to
/// pair the release with its press even if the modifiers or the logical key change in between.
pub type HostKeyId = u32;

/// Maximum of emulated frames that a pasted character waits for the ROM.
///
/// It is ~4.6 s at 1x: how long each character of [`Pc1500::queue_text`] waits for the ROM to be
/// ready for the keyboard. After that time it is typed anyway: so a running program that reads the
/// keyboard with `INKEY$` (and never stops in `WAIT_4_KB`) also gets the text, if slowly.
pub const TYPED_IDLE_TIMEOUT_FRAMES: usize = 400;

/// Minimum emulated frames that `ON/BRK` is held down.
///
/// The ROM only needs the edge (see `Keyboard::on_pulse`), so a little is enough.
pub const ON_MIN_HOLD_FRAMES: usize = 2;

/// The `SHIFT` bit of the indicator byte $764E (see [`Pc1500::lcd_symbol_bytes`]): the `SHIFT`
/// prefix is "latched".
const SYMBOL_SHIFT: u8 = 0x02;
/// The `SML` (lowercase) bit of the indicator byte $764E.
const SYMBOL_SML: u8 = 0x08;
/// Address of the indicator byte with `SHIFT`/`SML`/`DEF`.
const SYMBOLS_ADDR: u32 = 0x764E;

/// How the feeder decides whether `SHIFT` has to be pressed (as a prefix, see [`KeyStroke`]) before
/// the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftPolicy {
    /// What a person does with the host's keyboard: press `SHIFT` only if the key press carries it
    /// and the `SHIFT` indicator is not latched already. A `SHIFT` that the user latched on purpose
    /// (F11 or the key on screen) still counts for the next key, and in `SML` mode letters come out
    /// as they would on the real machine.
    AddIfNeeded,
    /// Type EXACTLY the character asked for (pasted text, or letters with "match upper/lower
    /// case"): it takes into account the `SML` mode (which swaps upper and lower case for letters),
    /// and a latched `SHIFT` that is not wanted is removed by pressing it again.
    Exact,
}

/// A key in the feeder's queue.
#[derive(Clone, Copy, Debug)]
struct Step {
    kind: StepKind,
    stroke: KeyStroke,
    policy: ShiftPolicy,
}

#[derive(Clone, Copy, Debug)]
enum StepKind {
    /// Physical key of the host (`id`, or `None` for the one-off key presses of
    /// [`Pc1500::host_tap`]): it is held until the host releases it (`host_released`).
    Live {
        id: Option<HostKeyId>,
        host_released: bool,
    },
    /// Character of a pasted text: a minimum key press, waiting first for the ROM to be ready.
    Typed,
}

/// At which point of the key press of the queue's first step the feeder is. The number is how many
/// frames it has been in that phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Nothing pressed; the queue's first step (if any) has not started yet.
    Idle,
    /// `SHIFT` (prefix) pressed.
    ShiftDown(usize),
    /// `SHIFT` released, waiting for the ROM to see the keyboard empty.
    ShiftGap(usize),
    /// The key of the queue's first step, pressed.
    Down(usize),
    /// The key was already released and its step left the queue; waiting for the ROM to see the
    /// keyboard empty before the next one.
    Gap(usize),
}

/// `ON/BRK` pressed from the host.
#[derive(Clone, Copy, Debug)]
struct OnHold {
    id: HostKeyId,
    frames: usize,
    host_released: bool,
}

/// What the feeder needs to know about the machine at each frame.
#[derive(Clone, Copy, Debug)]
pub struct FeedEnv {
    /// The indicator byte $764E (bits `SHIFT` and `SML`).
    pub symbols: u8,
    /// [`Pc1500::is_ready_for_key`].
    pub rom_ready: bool,
    /// [`Pc1500::is_powered_on`].
    pub powered_on: bool,
}

/// The feeder's state machine. See the module's comment.
#[derive(Debug)]
pub struct KeyFeeder {
    queue: VecDeque<Step>,
    phase: Phase,
    on: Option<OnHold>,
    /// Frames in a row (counted at each [`KeyFeeder::tick`]) with the ROM ready for the keyboard.
    ready_streak: usize,
    /// Frames that the pasted character at the front of the queue has been waiting for the ROM.
    waited: usize,
}

impl KeyFeeder {
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            phase: Phase::Idle,
            on: None,
            ready_streak: 0,
            waited: 0,
        }
    }

    /// `true` if the queue's first step has already started (it has `SHIFT` or its key down).
    const fn front_active(&self) -> bool {
        matches!(
            self.phase,
            Phase::ShiftDown(_) | Phase::ShiftGap(_) | Phase::Down(_)
        )
    }

    /// See [`Pc1500::host_key_down_with`].
    pub fn key_down(
        &mut self,
        kb: &mut Keyboard,
        id: HostKeyId,
        stroke: KeyStroke,
        policy: ShiftPolicy,
    ) {
        if stroke.key == Key::On {
            self.on_down(kb, id, false);
            return;
        }
        let already_down = self.queue.iter().any(|step| {
            matches!(step.kind, StepKind::Live { id: Some(other), host_released: false } if other == id)
        });
        if !already_down {
            self.queue.push_back(Step {
                kind: StepKind::Live { id: Some(id), host_released: false },
                stroke,
                policy,
            });
        }
    }

    /// See [`Pc1500::host_key_up`].
    pub fn key_up(&mut self, id: HostKeyId) {
        if let Some(on) = &mut self.on
            && on.id == id
        {
            on.host_released = true;
        }
        for step in &mut self.queue {
            if let StepKind::Live { id: Some(other), host_released } = &mut step.kind
                && *other == id
            {
                *host_released = true;
            }
        }
    }

    /// See [`Pc1500::host_tap_with`].
    pub fn tap(&mut self, kb: &mut Keyboard, strokes: &[KeyStroke], policy: ShiftPolicy) {
        for &stroke in strokes {
            if stroke.key == Key::On {
                self.on_down(kb, TAP_ON_ID, true);
            } else {
                self.queue.push_back(Step {
                    kind: StepKind::Live { id: None, host_released: true },
                    stroke,
                    policy,
                });
            }
        }
    }

    /// See [`Pc1500::queue_text`].
    pub fn queue_text(&mut self, text: &str) -> Vec<char> {
        let mut skipped = Vec::new();
        for c in text.chars().filter(|&c| c != '\r') {
            match key_for_char(c) {
                Some(stroke) => self.queue.push_back(Step {
                    kind: StepKind::Typed,
                    stroke,
                    policy: ShiftPolicy::Exact,
                }),
                None => skipped.push(c),
            }
        }
        skipped
    }

    /// `ON/BRK` pressed: at once, without a queue (see the module's comment), and cancelling the
    /// pasted text pending.
    fn on_down(&mut self, kb: &mut Keyboard, id: HostKeyId, host_released: bool) {
        // Another host key that is also ON/BRK (Esc and F12, e.g.) while `ON` is still down
        // (because of the first one, or still within its minimum of frames): the last one pressed
        // rules, so `ON` is released when that one is released, even if the previous one is still
        // down (the release of the previous one no longer counts). There is no new pulse either:
        // the ROM only sees the rising edge of `ON` (`on_pulse`), so two BREAKs faster than
        // `ON_MIN_HOLD_FRAMES` count as one, which for a BREAK makes no difference.
        if let Some(on) = &mut self.on {
            on.id = id;
            on.host_released = host_released;
        } else {
            kb.press(Key::On);
            self.on = Some(OnHold { id, frames: 0, host_released });
        }
        self.cancel_typing(kb);
    }

    /// See [`Pc1500::cancel_typing`].
    pub fn cancel_typing(&mut self, kb: &mut Keyboard) {
        let active_typed = self.front_active()
            && matches!(self.queue.front(), Some(Step { kind: StepKind::Typed, .. }));
        if active_typed {
            self.abort_front(kb);
        }
        self.queue.retain(|step| !matches!(step.kind, StepKind::Typed));
        // The wait for the ROM is that of the pasted character at the front, which is no longer
        // there: let the next pasted text start counting from zero. If the front is now a live key,
        // `waited` does not affect it.
        self.waited = 0;
    }

    /// Releases whatever the queue's first step (already started) has down, takes it out of the
    /// queue and moves to [`Phase::Gap`].
    fn abort_front(&mut self, kb: &mut Keyboard) {
        match self.phase {
            Phase::ShiftDown(_) => kb.release(Key::Shift),
            Phase::Down(_) => {
                if let Some(step) = self.queue.front() {
                    kb.release(step.stroke.key);
                }
            }
            Phase::Idle | Phase::ShiftGap(_) | Phase::Gap(_) => {}
        }
        self.queue.pop_front();
        self.phase = Phase::Gap(0);
    }

    /// See [`Pc1500::release_all_host_keys`].
    pub fn release_all(&mut self) {
        if let Some(on) = &mut self.on {
            on.host_released = true;
        }
        for step in &mut self.queue {
            if let StepKind::Live { host_released, .. } = &mut step.kind {
                *host_released = true;
            }
        }
    }

    /// Steps in the queue, including the one being typed.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// One emulated frame. See the module's comment.
    pub fn tick(&mut self, kb: &mut Keyboard, env: FeedEnv) {
        self.ready_streak = if env.rom_ready { self.ready_streak + 1 } else { 0 };

        if let Some(on) = &mut self.on {
            on.frames += 1;
            if on.host_released && on.frames >= ON_MIN_HOLD_FRAMES {
                kb.release(Key::On);
                self.on = None;
            }
        }

        // With the machine off nobody is reading the keyboard: the pasted text pending is dropped
        // instead of waiting for the time limit key by key.
        if !env.powered_on {
            self.cancel_typing(kb);
        }

        match self.phase {
            Phase::Idle => self.start_front(kb, env),
            Phase::ShiftDown(frames) => {
                self.phase = if frames + 1 >= KEY_HOLD_FRAMES {
                    kb.release(Key::Shift);
                    Phase::ShiftGap(0)
                } else {
                    Phase::ShiftDown(frames + 1)
                };
            }
            Phase::ShiftGap(frames) => {
                if frames + 1 >= KEY_RELEASE_FRAMES {
                    if let Some(step) = self.queue.front() {
                        kb.press(step.stroke.key);
                    }
                    self.phase = Phase::Down(0);
                } else {
                    self.phase = Phase::ShiftGap(frames + 1);
                }
            }
            Phase::Down(frames) => {
                let frames = frames + 1;
                if frames >= KEY_HOLD_FRAMES && self.front_may_release() {
                    self.abort_front(kb);
                } else {
                    self.phase = Phase::Down(frames);
                }
            }
            Phase::Gap(frames) => {
                if frames + 1 >= KEY_RELEASE_FRAMES {
                    self.phase = Phase::Idle;
                    self.start_front(kb, env);
                } else {
                    self.phase = Phase::Gap(frames + 1);
                }
            }
        }
    }

    /// `true` if the key of the first step (already down for the minimum) can be released: always
    /// if it is a pasted character; if it is a live key, when the host has released it or when
    /// another, newer live key is waiting (rollover).
    fn front_may_release(&self) -> bool {
        let Some(front) = self.queue.front() else {
            return true;
        };
        match front.kind {
            StepKind::Typed => true,
            StepKind::Live { host_released, .. } => {
                host_released
                    || self
                        .queue
                        .iter()
                        .skip(1)
                        .any(|step| matches!(step.kind, StepKind::Live { .. }))
            }
        }
    }

    /// Starts the queue's first step, if there is one and its turn has come: presses `SHIFT`
    /// (prefix) if needed according to its [`ShiftPolicy`], or the key directly.
    fn start_front(&mut self, kb: &mut Keyboard, env: FeedEnv) {
        let Some(&step) = self.queue.front() else {
            self.waited = 0;
            return;
        };
        if matches!(step.kind, StepKind::Typed)
            && self.ready_streak < 2
            && self.waited < TYPED_IDLE_TIMEOUT_FRAMES
        {
            self.waited += 1;
            return;
        }
        self.waited = 0;

        let latched = env.symbols & SYMBOL_SHIFT != 0;
        let sml = env.symbols & SYMBOL_SML != 0;
        let stroke = step.stroke;
        let tap_shift = stroke.key != Key::Shift
            && match step.policy {
                ShiftPolicy::AddIfNeeded => stroke.shift && !latched,
                ShiftPolicy::Exact => (stroke.shift ^ (stroke.key.is_letter() && sml)) != latched,
            };
        if tap_shift {
            kb.press(Key::Shift);
            self.phase = Phase::ShiftDown(0);
        } else {
            kb.press(stroke.key);
            self.phase = Phase::Down(0);
        }
    }
}

/// Identifier that [`Pc1500::host_tap`] uses for a one-off `ON/BRK` (pressed and released at once).
const TAP_ON_ID: HostKeyId = HostKeyId::MAX;

impl Pc1500 {
    /// Per-frame hook (see `Pc1500::run_frame_hooks`): one step of the key feeder.
    pub(crate) fn feed_keys(&mut self) {
        let env = FeedEnv {
            symbols: self.read_byte(SYMBOLS_ADDR),
            rom_ready: self.is_ready_for_key(),
            powered_on: self.is_powered_on(),
        };
        self.feeder.tick(&mut self.keyboard, env);
    }

    /// A physical key of the host (identified by `id`) has just been pressed and generates
    /// `stroke`: it is typed as soon as its turn comes and held down on the PC-1500 until
    /// [`Pc1500::host_key_up`] (at least [`KEY_HOLD_FRAMES`] emulated frames, however brief it was
    /// on the host). With [`ShiftPolicy::AddIfNeeded`]; see [`Pc1500::host_key_down_with`].
    pub fn host_key_down(&mut self, id: HostKeyId, stroke: KeyStroke) {
        self.host_key_down_with(id, stroke, ShiftPolicy::AddIfNeeded);
    }

    /// As [`Pc1500::host_key_down`], choosing how `SHIFT` is decided (see [`ShiftPolicy`]).
    ///
    /// A second press of the same `id` without releasing it (the operating system's key repeat) is
    /// ignored: the ROM does its own key repeat. `ON/BRK` ([`Key::On`]) does not wait its turn: it
    /// is pressed at once and cancels the pasted text pending (see [`Pc1500::cancel_typing`]).
    pub fn host_key_down_with(&mut self, id: HostKeyId, stroke: KeyStroke, policy: ShiftPolicy) {
        self.feeder.key_down(&mut self.keyboard, id, stroke, policy);
    }

    /// The physical key `id` has been released on the host: its PC-1500 key is released as soon as
    /// it has done its minimum. If it was not down, it does nothing.
    pub fn host_key_up(&mut self, id: HostKeyId) {
        self.feeder.key_up(id);
    }

    /// One-off key presses of fixed length in emulated time (press and release), in order, as live
    /// keys already released: e.g. the host's Backspace key (← and then `DEL`) or, above 1x, each
    /// key press of the host. With [`ShiftPolicy::AddIfNeeded`].
    pub fn host_tap(&mut self, strokes: &[KeyStroke]) {
        self.host_tap_with(strokes, ShiftPolicy::AddIfNeeded);
    }

    /// As [`Pc1500::host_tap`], choosing how `SHIFT` is decided.
    pub fn host_tap_with(&mut self, strokes: &[KeyStroke], policy: ShiftPolicy) {
        self.feeder.tap(&mut self.keyboard, strokes, policy);
    }

    /// Types `text` (e.g. pasted from the clipboard) character by character, exactly
    /// ([`ShiftPolicy::Exact`]: it respects upper and lower case also with `SML`), waiting before
    /// each key for the ROM to be ready (see the comment of the `key_feeder` module). `\r` is
    /// ignored and `\n` is ENTER.
    ///
    /// It returns the characters that cannot be typed (with no key on the PC-1500, see
    /// [`key_for_char`]), which are skipped; the rest is typed all the same.
    pub fn queue_text(&mut self, text: &str) -> Vec<char> {
        self.feeder.queue_text(text)
    }

    /// Drops the pasted text pending (and releases the key of it being typed). The host's live keys
    /// are not touched.
    pub fn cancel_typing(&mut self) {
        self.feeder.cancel_typing(&mut self.keyboard);
    }

    /// Takes all the host's keys as released (when the window loses focus, or when the keyboard
    /// stops being captured): each one is released as soon as it does its minimum, so none gets
    /// stuck or lost. The pasted text pending goes on.
    pub fn release_all_host_keys(&mut self) {
        self.feeder.release_all();
    }

    /// Steps pending in the feeder (live keys and pasted characters), including the one being typed
    /// now.
    #[must_use]
    pub fn pending_keystrokes(&self) -> usize {
        self.feeder.pending()
    }

    /// `true` if `key` is down right now in the emulated matrix (by the feeder or by
    /// [`Pc1500::press`]). To paint the keys.
    #[must_use]
    pub const fn is_key_pressed(&self, key: Key) -> bool {
        self.keyboard.is_pressed(key)
    }
}

#[cfg(test)]
mod tests {
    use super::{FeedEnv, KeyFeeder, ShiftPolicy, TYPED_IDLE_TIMEOUT_FRAMES};
    use crate::{
        Key,
        harness::{KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES},
        keyboard::{KeyStroke, Keyboard},
    };

    const READY: FeedEnv = FeedEnv { symbols: 0, rom_ready: true, powered_on: true };
    const BUSY: FeedEnv = FeedEnv { symbols: 0, rom_ready: false, powered_on: true };

    /// Runs `frames` ticks and returns, for each one, the keys down after the tick.
    fn run(feeder: &mut KeyFeeder, kb: &mut Keyboard, env: FeedEnv, frames: usize) -> Vec<Vec<Key>> {
        (0..frames)
            .map(|_| {
                feeder.tick(kb, env);
                Key::ALL.into_iter().filter(|&k| kb.is_pressed(k)).collect()
            })
            .collect()
    }

    #[test]
    fn short_live_press_gets_the_minimum_hold() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::A), ShiftPolicy::AddIfNeeded);
        feeder.key_up(1);
        let frames = run(&mut feeder, &mut kb, BUSY, KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES);
        let held = frames.iter().filter(|keys| keys == &&vec![Key::A]).count();
        assert_eq!(held, KEY_HOLD_FRAMES, "{frames:?}");
        assert_eq!(feeder.pending(), 0);
    }

    #[test]
    fn live_key_is_held_while_the_host_holds_it() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::Left), ShiftPolicy::AddIfNeeded);
        run(&mut feeder, &mut kb, BUSY, 50);
        assert!(kb.is_pressed(Key::Left), "still down");
        // OS key repeat: ignored.
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::Left), ShiftPolicy::AddIfNeeded);
        assert_eq!(feeder.pending(), 1);
        feeder.key_up(1);
        run(&mut feeder, &mut kb, BUSY, 1);
        assert!(!kb.is_pressed(Key::Left), "released in the next frame");
    }

    #[test]
    fn shifted_live_key_taps_shift_first_unless_latched() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.tap(&mut kb, &[KeyStroke::shifted(Key::LeftParen)], ShiftPolicy::AddIfNeeded);
        let frames = run(&mut feeder, &mut kb, BUSY, 2 * (KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES));
        assert_eq!(frames[0], vec![Key::Shift]);
        assert!(frames.iter().all(|keys| keys.len() <= 1), "never two at once");
        assert!(frames.iter().any(|keys| keys == &vec![Key::LeftParen]));

        // With SHIFT already latched it is not pressed again.
        let latched = FeedEnv { symbols: 0x02, ..BUSY };
        feeder.tap(&mut kb, &[KeyStroke::shifted(Key::LeftParen)], ShiftPolicy::AddIfNeeded);
        let after_latch = run(&mut feeder, &mut kb, latched, 1);
        assert_eq!(after_latch[0], vec![Key::LeftParen]);
    }

    #[test]
    fn exact_policy_follows_sml_and_clears_a_stale_latch() {
        let mut kb = Keyboard::new();
        let sml = FeedEnv { symbols: 0x08, ..READY };
        let cases = [
            // (text, environment, SHIFT first?)
            ('A', READY, false),
            ('a', READY, true),
            ('A', sml, true),
            ('a', sml, false),
            ('A', FeedEnv { symbols: 0x02, ..READY }, true),
            ('(', FeedEnv { symbols: 0x02, ..READY }, true),
            ('(', sml, false),
        ];
        for (c, env, shift_first) in cases {
            let mut feeder = KeyFeeder::new();
            assert!(feeder.queue_text(&c.to_string()).is_empty());
            let frames = run(&mut feeder, &mut kb, env, 2);
            let first = frames.iter().find(|keys| !keys.is_empty()).cloned();
            assert_eq!(
                first.as_deref() == Some(&[Key::Shift][..]),
                shift_first,
                "{c:?} with $764E={:#04X}: {frames:?}",
                env.symbols
            );
            run(&mut feeder, &mut kb, env, 20);
            assert_eq!(feeder.pending(), 0);
        }
    }

    #[test]
    fn typed_steps_wait_for_the_rom_until_the_timeout() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.queue_text("A");
        let frames = run(&mut feeder, &mut kb, BUSY, TYPED_IDLE_TIMEOUT_FRAMES);
        assert!(frames.iter().all(Vec::is_empty), "waits for the ROM");
        run(&mut feeder, &mut kb, BUSY, 1);
        assert!(kb.is_pressed(Key::A), "past the limit, it is typed");

        let mut ready_feeder = KeyFeeder::new();
        let mut ready_kb = Keyboard::new();
        ready_feeder.queue_text("A");
        let ready_frames = run(&mut ready_feeder, &mut ready_kb, READY, 2);
        assert_eq!(ready_frames, vec![vec![], vec![Key::A]], "ready for 2 frames");
    }

    #[test]
    fn rollover_releases_the_older_key_after_the_minimum() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::H), ShiftPolicy::AddIfNeeded);
        run(&mut feeder, &mut kb, BUSY, 1);
        feeder.key_down(&mut kb, 2, KeyStroke::plain(Key::I), ShiftPolicy::AddIfNeeded);
        let frames = run(&mut feeder, &mut kb, BUSY, 30);
        assert!(kb.is_pressed(Key::I), "the new one stays held");
        assert!(!kb.is_pressed(Key::H));
        assert!(frames.iter().all(|keys| keys.len() <= 1));
        feeder.key_up(1); // la vieja ya no estaba: no pasa nada
        feeder.key_up(2);
        run(&mut feeder, &mut kb, BUSY, 1);
        assert!(!kb.is_pressed(Key::I));
    }

    #[test]
    fn on_is_immediate_and_cancels_typing() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.queue_text("HELLO");
        run(&mut feeder, &mut kb, READY, 3);
        assert!(kb.is_pressed(Key::H));
        feeder.key_down(&mut kb, 9, KeyStroke::plain(Key::On), ShiftPolicy::AddIfNeeded);
        assert!(kb.is_pressed(Key::On), "at once");
        assert!(kb.take_on_pulse());
        assert!(!kb.is_pressed(Key::H), "the pasted key is released");
        assert_eq!(feeder.pending(), 0, "text cancelled");
        feeder.key_up(9);
        run(&mut feeder, &mut kb, READY, 1);
        assert!(kb.is_pressed(Key::On), "minimum of 2 frames");
        run(&mut feeder, &mut kb, READY, 1);
        assert!(!kb.is_pressed(Key::On));
    }

    #[test]
    fn release_all_lets_everything_finish() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::A), ShiftPolicy::AddIfNeeded);
        feeder.key_down(&mut kb, 2, KeyStroke::plain(Key::On), ShiftPolicy::AddIfNeeded);
        run(&mut feeder, &mut kb, BUSY, 10);
        feeder.release_all();
        run(&mut feeder, &mut kb, BUSY, 1);
        assert!(Key::ALL.iter().all(|&k| !kb.is_pressed(k)));
        assert_eq!(feeder.pending(), 0);
    }

    #[test]
    fn powered_off_machine_drops_typed_text() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.queue_text("AB");
        feeder.tap(&mut kb, &[KeyStroke::plain(Key::C)], ShiftPolicy::AddIfNeeded);
        let off = FeedEnv { powered_on: false, ..BUSY };
        run(&mut feeder, &mut kb, off, 1);
        assert_eq!(feeder.pending(), 1, "only the live key is left");
    }
}
