//! "Headless" harness (no GUI) to drive a whole [`Pc1500`] from code: boot the real ROM up to
//! BASIC's prompt, press keys for an exact number of emulated frames, type ASCII text by turning it
//! into the sequence of the PC-1500's physical keys (with `SHIFT` where needed) and read back what
//! the LCD shows as text.
//!
//! It exists to check automatically (`cargo test`) any new feature of the emulator — BREAK, TIME,
//! loading and saving programs, keyboard mapping... — against the behaviour of the authentic ROM,
//! instead of having to check it by hand in the GUI. The pieces that do not depend on the tests
//! (e.g. [`text_to_keystrokes`], useful to "paste" text in the GUI) are public on purpose.
//!
//! # Timing
//! Everything is measured in calls to [`Pc1500::step_frame`] ("frames"), not in host time: a frame
//! is always [`crate::FRAME_DURATION`] (~11.5 ms) of emulated time, so these numbers are
//! deterministic and independent of how fast the computer running the tests is. See
//! [`KEY_HOLD_FRAMES`] and [`KEY_RELEASE_FRAMES`] for how the minimums were measured.
//!
//! # Clock (`TIME`)
//! [`Pc1500::new`] starts the `µPD1990AC` clock at the host's local time, which moves on in real
//! time: what the GUI wants, but not a test (what `TIME` reads, and the seed of `RANDOM`, would
//! depend on when and at what speed it runs). Tests must create the machine with
//! [`Pc1500::new_deterministic`] (a clock fixed at [`HARNESS_RTC_START`] that moves on with the
//! emulated cycles), or choose another time with [`Pc1500::set_rtc_time_base`] right after
//! [`Pc1500::new`].
//!
//! # Power and automatic power-off
//! The `OFF` key and the ROM's automatic power-off (after ~5 emulated minutes waiting for the
//! keyboard without anything pressed) really switch the machine off: [`Pc1500::is_powered_on`]
//! becomes `false`, the CPU stops running and the functions that wait for the ROM
//! ([`Pc1500::run_until_idle`], [`Pc1500::enter_line`]...) no longer end. [`Pc1500::power_on`]
//! switches it on again. A test that leaves the machine idle for a long time (more than ~25000
//! frames) must disable it first with [`Pc1500::set_auto_power_off`]`(false)`.
//!
//! # Typing whole lines
//! To enter program lines or commands one after another, use [`Pc1500::enter_line`] /
//! [`Pc1500::enter_lines`], not [`Pc1500::type_text`] + [`Pc1500::run_until_idle`]: the ROM being
//! idle in `WAIT_4_KB` does not mean it will take the next key. See [`Pc1500::is_ready_for_key`]
//! for the exact cause (the "release debounce" of bit 0 of `CURS_CTRL`).

use core::{array, error::Error, fmt};

use crate::{Key, Pc1500, RtcDateTime, RtcTimeBase, display::DISPLAY_WIDTH};

/// Time at which the clock of [`Pc1500::new_deterministic`] starts: January 1 at 12:00:00, a Sunday
/// (day of the week 0). Far from any change of day, so that a test that does not look at the clock
/// never sees anything odd happen.
pub const HARNESS_RTC_START: RtcDateTime = RtcDateTime {
    month: 1,
    day: 1,
    hour: 12,
    minute: 0,
    second: 0,
    weekday: 0,
};

/// Number of characters that fit on the PC-1500's LCD: 156 columns of pixels / 6 columns per
/// character (5 of glyph + 1 of spacing).
pub const LCD_CHARS: usize = 26;

/// Columns of pixels each character takes on the LCD: 5 columns of glyph (the ROM's 5x7 matrix)
/// followed by 1 empty column of spacing. Checked on the cold start message `NEW0? :CHECK`, whose
/// "N" takes exactly columns 0-4.
pub const LCD_CELL_WIDTH: usize = 6;

/// Frames a key has to be held down for the ROM to register it reliably.
///
/// The ROM's keyboard wait loop (`WAIT_4_KB`, $E243) sleeps in `HLT` ($E2A8) between timer
/// interrupts; when it detects a key it converts it with `KEY_2_ASCII` ($E42C), processes it, and
/// takes no other until it has seen the keyboard empty for a while. Measured typing a text of 23
/// different characters (and another with repeated keys, `AABB1122`) with all the combinations of 1
/// to 7 frames of press (`h`) and of release (`r`): it works if and only if `r >= 2` and
/// `h + r >= 6` (with less, keys are lost, e.g. `h=r=2` types only one in two). Holding the key
/// down much longer (tried up to 80 frames) gives no key repeat for character keys. The defaults, 4
/// + 4 = 8 frames per key (~92 emulated ms), leave 2 frames of margin over the minimum on both
/// sides; `tests/harness.rs` checks that the minimum `3 + 3` still works, to notice if a future
/// change makes it worse.
pub const KEY_HOLD_FRAMES: usize = 4;

/// Frames to leave with NO key down after releasing one, before pressing the next: the ROM has to
/// see the keyboard empty to "rearm" the detection (otherwise the next key press is taken as the
/// same key held and is lost). See [`KEY_HOLD_FRAMES`] for the measurement.
pub const KEY_RELEASE_FRAMES: usize = 4;

/// Maximum of frames that [`Pc1500::boot`] waits for the ROM to reach the keyboard wait loop. In
/// practice the cold start takes 36 frames (~0.4 emulated s: memory test, clearing the LCD and the
/// `NEW0? :CHECK` message) and the whole [`Pc1500::boot`], with the `CL`, 46 (measured with the
/// virtual table of `CLOAD`/`CSAVE` at $9800, whose initialization vector the ROM calls at boot:
/// without it, they were 35 and 45); the limit only keeps a test from hanging if the emulator
/// breaks.
pub const BOOT_MAX_FRAMES: usize = 600;

/// Maximum of frames that [`Pc1500::enter_line`] waits for the ROM to finish processing a line and
/// be ready for the keyboard again (~34 emulated s). Entering a program line or running a simple
/// direct command takes 2 to ~10 frames; the limit only keeps a test from hanging if something
/// breaks. For commands that take longer (e.g. `RUN` of a long program), use [`Pc1500::type_text`]
/// and [`Pc1500::run_until_ready`] with a limit of their own.
pub const LINE_MAX_FRAMES: usize = 3000;

/// `CURS_CTRL` ($7B0E), the ROM's cursor/keyboard control byte. See [`Pc1500::is_ready_for_key`].
const CURS_CTRL: u32 = 0x7B0E;

/// Bit 0 of `CURS_CTRL`: "there is a key not yet seen released". See [`Pc1500::is_ready_for_key`].
const CURS_CTRL_KEY_DOWN: u8 = 0x01;

/// The ROM's 5x7 character table (`$FCA0`, "Character set table"): 96 glyphs for the codes $20-$7F,
/// 5 bytes per glyph, one byte per column with bit 0 = top row and bit 6 = bottom row (the same
/// layout that `display.rs` decodes). `CHAR_2_ADDR` ($EE48) uses it: address = $FC00 + 5 × code.
const FONT_TABLE: u32 = 0xFCA0;
const FONT_FIRST_CHAR: u8 = 0x20;
const FONT_CHARS: u8 = 96;

// `KeyStroke`, `key_for_char` and `text_to_keystrokes` live in `keyboard.rs` (the GUI's key feeder
// uses them too, see `key_feeder.rs`); they are re-exported here so that the harness's API does not
// change.
pub use crate::keyboard::{KeyStroke, key_for_char, text_to_keystrokes};

/// Why [`Pc1500::enter_line`] / [`Pc1500::enter_lines`] failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterLineError {
    /// The text has a character with no key (see [`key_for_char`]). Nothing has been pressed.
    Unmappable(char),
    /// [`Pc1500::enter_line`] got more than one line (a `\n` that is not at the end). Use
    /// [`Pc1500::enter_lines`]. Nothing has been pressed.
    MultipleLines,
    /// The ROM was not ready for the next key within [`LINE_MAX_FRAMES`] frames (a program that
    /// does not end, an `INPUT` that never asks for the keyboard, a broken emulator...). Part of
    /// the text may have been typed.
    NotReady,
}

impl fmt::Display for EnterLineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unmappable(c) => write!(f, "the character '{c}' has no key on the PC-1500"),
            Self::MultipleLines => f.write_str("enter_line takes a single line"),
            Self::NotReady => write!(
                f,
                "the ROM was not ready for the keyboard again in {LINE_MAX_FRAMES} frames"
            ),
        }
    }
}

impl Error for EnterLineError {}

impl Pc1500 {
    /// As [`Pc1500::new`], but with the `µPD1990AC` clock in [`RtcTimeBase::Emulated`], starting at
    /// [`HARNESS_RTC_START`]: a fixed time that moves on with the emulated cycles, without looking
    /// at the host's clock. So everything the machine does (`TIME` and `RANDOM` included) depends
    /// only on what is typed into it. It is the way to create the machine in the tests.
    #[must_use]
    pub fn new_deterministic() -> Self {
        let mut pc = Self::new();
        pc.set_rtc_time_base(RtcTimeBase::Emulated(HARNESS_RTC_START));
        pc
    }

    /// Ejecuta exactamente `frames` llamadas a [`Pc1500::step_frame`].
    pub fn run_frames(&mut self, frames: usize) {
        for _ in 0..frames {
            self.step_frame();
        }
    }

    /// `true` if the ROM is idle waiting for a key: the CPU is halted in the `HLT` of the
    /// `WAIT_4_KB` loop ($E243-$E33E), which is where it spends almost all its time when it shows
    /// the prompt (or when an `INPUT` waits for data). It is only reliable looked at between
    /// frames: the loop wakes up at each timer interrupt to blink the cursor and goes back to sleep
    /// at once.
    #[must_use]
    ///
    /// With the machine off (see [`Pc1500::is_powered_on`]) it is always `false`: do not call
    /// [`Pc1500::run_until_idle`] in that state.
    pub fn is_waiting_for_key(&self) -> bool {
        self.lh5801.bf() && self.lh5801.is_halted() && self.cpu_in_wait_4_kb()
    }

    /// Runs frames until `done(self)` is true, checking it after each frame, up to `max_frames`. It
    /// returns the number of frames run, or `None` if the limit ran out.
    pub fn run_until<F: FnMut(&Self) -> bool>(
        &mut self,
        max_frames: usize,
        mut done: F,
    ) -> Option<usize> {
        for frame in 1..=max_frames {
            self.step_frame();
            if done(self) {
                return Some(frame);
            }
        }
        None
    }

    /// Runs frames until the ROM waits for the keyboard again (see [`Pc1500::is_waiting_for_key`])
    /// for 2 frames in a row — a single one could be a momentary `HLT` in the middle of a command
    /// that has not finished painting its result yet. It returns the frames run or `None` if it
    /// does not happen within `max_frames`.
    pub fn run_until_idle(&mut self, max_frames: usize) -> Option<usize> {
        let mut streak = 0;
        self.run_until(max_frames, |pc| {
            streak = if pc.is_waiting_for_key() { streak + 1 } else { 0 };
            streak >= 2
        })
    }

    /// `true` if the ROM is not only idle in `WAIT_4_KB` (see [`Pc1500::is_waiting_for_key`]) but
    /// will also take the NEXT key pressed as a new key press.
    ///
    /// # Why `is_waiting_for_key` is not enough
    /// `WAIT_4_KB` ($E243) does not detect key "edges": at each wake-up of the timer it looks
    /// whether ANY key is down, and bit 0 of `CURS_CTRL` ($7B0E) remembers whether there already
    /// was one at the previous wake-up:
    ///
    /// - Key down with bit 0 at 0 (`WAIT_4_KB_7`, $E2AC): a new key. The ROM sets bit 0, keeps the
    ///   code in `KEY_LAST` ($7B0F) and leaves `WAIT_4_KB` returning it.
    /// - Key down with bit 0 at 1: taken as the SAME key held. Without key repeat on (bit 6 at 0,
    ///   the usual except with the arrows) it is ignored (`WAIT_4_KB_16`, $E2FF), and besides the
    ///   release counter is restarted (`UH = $F8`) — whether or not it is a different key.
    /// - No key with bit 0 at 1 (`WAIT_4_KB_10`, $E2C4): `UH` is incremented, and only when it
    ///   reaches 0 (8 wake-ups in a row with no key) is bit 0 cleared ($E2D4).
    ///
    /// That bit 0 is ONLY cleared inside `WAIT_4_KB`: the entry to the routine ($E24A) clears bit 1
    /// but not 0. So if the ROM takes longer to process a key than its press lasts (the usual thing
    /// with ENTER on a long line: tokenizing it, inserting it in the program and painting it
    /// again), when it goes back to `WAIT_4_KB` and sleeps in `HLT` bit 0 is still at 1 although no
    /// key is down any more, and the ROM still needs those 8 wake-ups (~1-2 frames) with no keys
    /// before it takes another. A key pressed in that gap is lost ENTIRELY: measured typing
    /// `jackpot` line by line, after `170 SI=112:DO=105:RE=93:MI=82:SO=68:TP=80` (41 characters)
    /// `run_until_idle` came back with `CURS_CTRL = $01` and the next line `200 REM ...` arrived as
    /// `00 REM ...` (ERROR 1). It is the authentic ROM's behaviour, not a fault of the emulator: on
    /// the real machine the same would happen typing that fast. This is the exact condition that
    /// was missing (it used to be covered up with ~30 fixed frames of waiting "just in case").
    #[must_use]
    pub fn is_ready_for_key(&self) -> bool {
        self.is_waiting_for_key() && self.read_byte(CURS_CTRL) & CURS_CTRL_KEY_DOWN == 0
    }

    /// As [`Pc1500::run_until_idle`], but waiting for [`Pc1500::is_ready_for_key`] (also 2 frames
    /// in a row): after this, the next key pressed is not lost. It returns the frames run or `None`
    /// if it does not happen within `max_frames` — e.g. if a key is held down, because then bit 0
    /// of `CURS_CTRL` is never cleared.
    pub fn run_until_ready(&mut self, max_frames: usize) -> Option<usize> {
        let mut streak = 0;
        self.run_until(max_frames, |pc| {
            streak = if pc.is_ready_for_key() { streak + 1 } else { 0 };
            streak >= 2
        })
    }

    /// Boots the real ROM from a new `Pc1500` until it waits for the keyboard. After a cold start
    /// the LCD shows `NEW0? :CHECK` (the ROM does not recognize the RAM as initialized); the first
    /// key pressed only clears that message, which is why this method also presses `CL`, leaving
    /// the LCD with the `>` prompt.
    ///
    /// The tokenized BASIC program that `MemoryBus::new()` puts in RAM at power-on (bathyscaph, at
    /// $00C5) is still in memory after this (no `NEW` is done): a test that does not want it can
    /// type `NEW\n`.
    ///
    /// It returns the frames it took, or `None` if the ROM did not reach the prompt within
    /// [`BOOT_MAX_FRAMES`].
    pub fn boot(&mut self) -> Option<usize> {
        let boot_frames = self.run_until_idle(BOOT_MAX_FRAMES)?;
        self.tap(KeyStroke::plain(Key::Cl));
        // `run_until_ready` and not `run_until_idle`: so the first key the test presses after
        // `boot()` never falls in the release debounce of the `CL` (see `is_ready_for_key`).
        let after_cl = self.run_until_ready(BOOT_MAX_FRAMES)?;
        Some(boot_frames + KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES + after_cl)
    }

    /// Switches on a machine that is off (see [`Pc1500::is_powered_on`]) as a person would: presses
    /// `ON` with the default timing and waits for the ROM to finish its `RESET` routine and be
    /// ready for the next key (see [`Pc1500::is_ready_for_key`]). What the LCD then shows depends
    /// on how it was switched off: after the `OFF` key, the `>` prompt (`COLD_START`, with the
    /// program kept); after the automatic power-off, the line being edited, as it was.
    ///
    /// Waiting for [`Pc1500::is_ready_for_key`] (and not only for [`Pc1500::is_waiting_for_key`])
    /// is what is needed here: when switched off with the `OFF` key, the ROM cuts the power with
    /// that key still down, so bit 0 of `CURS_CTRL` ("key not released") is still at 1 on switching
    /// on, and the first key pressed before `WAIT_4_KB` sees the keyboard empty for 8 wake-ups is
    /// lost entirely. There is no need to press `CL` afterwards.
    ///
    /// It returns the frames the ROM took after `ON` was released, or `None` if it was not ready
    /// within [`BOOT_MAX_FRAMES`] (e.g. if the machine was already on and a running program ignores
    /// the BREAK). With the machine on, pressing `ON` is a BREAK.
    pub fn power_on(&mut self) -> Option<usize> {
        self.tap_key_for(Key::On, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES);
        self.run_until_ready(BOOT_MAX_FRAMES)
    }

    /// Presses `key` for `hold` frames and releases it for `release` frames.
    pub fn tap_key_for(&mut self, key: Key, hold: usize, release: usize) {
        self.press(key);
        self.run_frames(hold);
        self.release(key);
        self.run_frames(release);
    }

    /// Types a [`KeyStroke`] with the default timing ([`KEY_HOLD_FRAMES`] /
    /// [`KEY_RELEASE_FRAMES`]), pressing and releasing `SHIFT` first if needed.
    pub fn tap(&mut self, stroke: KeyStroke) {
        self.tap_with_timing(stroke, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES);
    }

    /// The same as [`Pc1500::tap`] but with explicit timing (to measure it, or for programs that
    /// read the keyboard more slowly).
    pub fn tap_with_timing(&mut self, stroke: KeyStroke, hold: usize, release: usize) {
        if stroke.shift {
            self.tap_key_for(Key::Shift, hold, release);
        }
        self.tap_key_for(stroke.key, hold, release);
    }

    /// Types `text` (see [`text_to_keystrokes`]) with the default timing. If it has a character
    /// with no key, it returns `Err(c)` without having pressed anything.
    ///
    /// It does not wait for the ROM to finish processing an ENTER, so with several lines in a row
    /// in the same text the first key of a line can be lost if the previous one takes long to
    /// process (see [`Pc1500::is_ready_for_key`]). To type whole lines use [`Pc1500::enter_line`] /
    /// [`Pc1500::enter_lines`]; `type_text` is for the rest (echo of single characters, answering
    /// an `INPUT` or a `PRINT` that waits for ENTER inside a program that then goes on running...),
    /// followed by [`Pc1500::run_until_ready`].
    pub fn type_text(&mut self, text: &str) -> Result<(), char> {
        self.type_text_with_timing(text, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES)
    }

    /// The same as [`Pc1500::type_text`] but with explicit timing.
    pub fn type_text_with_timing(
        &mut self,
        text: &str,
        hold: usize,
        release: usize,
    ) -> Result<(), char> {
        for stroke in text_to_keystrokes(text)? {
            self.tap_with_timing(stroke, hold, release);
        }
        Ok(())
    }

    /// Types ONE line followed by ENTER and waits for the ROM to have processed it and be ready for
    /// the next key again (see [`Pc1500::is_ready_for_key`]), so it can be called once after
    /// another without losing keys. It is for program lines (`"10 PRINT 5"`) and for direct
    /// commands that go back to the prompt or wait for the keyboard (`"LIST"`, `"PRINT 1+2"`,
    /// `"RUN"` of a program that does `PRINT`...). `""` presses only ENTER.
    ///
    /// The final `\n` is optional (`"10 A=1"` and `"10 A=1\n"` are the same, and `\r\n` is accepted
    /// too); any other `\n` is an error ([`EnterLineError::MultipleLines`]): for several lines,
    /// [`Pc1500::enter_lines`]. If the ROM was not ready at the start (e.g. right after a
    /// `type_text`), it first waits until it is.
    ///
    /// It returns the frames the ROM took to be ready after the ENTER (not counting those of the
    /// typing itself).
    pub fn enter_line(&mut self, line: &str) -> Result<usize, EnterLineError> {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let body = body.strip_suffix('\r').unwrap_or(body);
        if body.contains('\n') {
            return Err(EnterLineError::MultipleLines);
        }
        let mut strokes = text_to_keystrokes(body).map_err(EnterLineError::Unmappable)?;
        strokes.push(KeyStroke::plain(Key::Enter));
        self.type_strokes_when_ready(&strokes)
    }

    /// Types several lines with [`Pc1500::enter_line`], one after another (separated by `\n` or
    /// `\r\n`; the final line break is optional and empty lines in between are just an ENTER). It
    /// checks ALL the text before pressing anything, so an [`EnterLineError::Unmappable`] leaves no
    /// lines half done. It returns the sum of the waiting frames of each line.
    pub fn enter_lines(&mut self, text: &str) -> Result<usize, EnterLineError> {
        let lines: Vec<Vec<KeyStroke>> = text
            .lines()
            .map(|line| {
                let mut strokes = text_to_keystrokes(line)?;
                strokes.push(KeyStroke::plain(Key::Enter));
                Ok(strokes)
            })
            .collect::<Result<_, char>>()
            .map_err(EnterLineError::Unmappable)?;
        let mut total = 0;
        for strokes in &lines {
            total += self.type_strokes_when_ready(strokes)?;
        }
        Ok(total)
    }

    /// Core of [`Pc1500::enter_line`]: waits (if needed) for the ROM to be ready, types `strokes`
    /// with the default timing and waits again for it to be ready, returning the frames of that
    /// last wait.
    fn type_strokes_when_ready(&mut self, strokes: &[KeyStroke]) -> Result<usize, EnterLineError> {
        if !self.is_ready_for_key() {
            self.run_until_ready(LINE_MAX_FRAMES)
                .ok_or(EnterLineError::NotReady)?;
        }
        for &stroke in strokes {
            self.tap(stroke);
        }
        self.run_until_ready(LINE_MAX_FRAMES)
            .ok_or(EnterLineError::NotReady)
    }

    /// The 156 columns of pixels of the LCD, read straight from the display RAM with the same
    /// mapping as `display.rs`: each column is a byte with bit 0 = top row. Columns 0-38 = low
    /// nibbles of $7600-$764D, 39-77 = low ones of $7700-$774D, 78-116 = high ones of $7600..,
    /// 117-155 = high ones of $7700... (the nibble of the even address gives rows 0-3 and that of
    /// the odd one rows 4-6).
    ///
    /// It does not look at whether the LCD is on (the CPU's `DISP` flag): it returns what is in the
    /// display RAM.
    #[must_use]
    pub fn lcd_columns(&self) -> [u8; DISPLAY_WIDTH] {
        let mut columns = [0; DISPLAY_WIDTH];
        for ind in (0..0x4D_u32).step_by(2) {
            for (base, first_x) in [(0x7600_u32, 0_usize), (0x7700, 39)] {
                let even = self.read_byte(base | ind);
                let odd = self.read_byte((base | ind) + 1);
                let x = first_x + (ind as usize >> 1);
                columns[x] = ((even & 0x0F) | (odd << 4)) & 0x7F;
                columns[x + 78] = ((even >> 4) | (odd & 0xF0)) & 0x7F;
            }
        }
        columns
    }

    /// The ROM's 5x7 glyph for the code `code` ($20-$7F).
    fn font_glyph(&self, code: u8) -> [u8; 5] {
        let base = FONT_TABLE + 5 * u32::from(code - FONT_FIRST_CHAR);
        let mut glyph = [0; 5];
        for (addr, column) in (base..).zip(glyph.iter_mut()) {
            *column = self.read_byte(addr) & 0x7F;
        }
        glyph
    }

    /// The 26 character cells of the LCD, each one recognized by comparing its 5 glyph columns with
    /// the ROM's character table ($FCA0). `None` if the cell matches no glyph (graphics of
    /// `GPRINT`, katakana, the block of the insert cursor...) or if its spacing column is not
    /// empty.
    ///
    /// The normal cursor is the glyph of `_`, so a cell with the cursor visible reads as `'_'` (and
    /// as `' '` in the off phase of the blink).
    #[must_use]
    pub fn lcd_cells(&self) -> [Option<char>; LCD_CHARS] {
        let columns = self.lcd_columns();
        // Code $7F is the block of the insert cursor, not a printable character: it is left out of
        // the search.
        let glyphs: Vec<(u8, [u8; 5])> = (FONT_FIRST_CHAR..FONT_FIRST_CHAR + FONT_CHARS - 1)
            .map(|code| (code, self.font_glyph(code)))
            .collect();
        array::from_fn(|cell| {
            let start = cell * LCD_CELL_WIDTH;
            if columns[start + 5] != 0 {
                return None;
            }
            let cell_cols = &columns[start..start + 5];
            glyphs
                .iter()
                .find(|(_, glyph)| glyph == cell_cols)
                .map(|&(code, _)| char::from(code))
        })
    }

    /// The LCD as a text of exactly 26 characters (see [`Pc1500::lcd_cells`]), with `'?'` in the
    /// cells not recognized. Note: a real `?` also reads as `'?'`; use [`Pc1500::lcd_cells`] if
    /// they have to be told apart.
    #[must_use]
    pub fn lcd_text(&self) -> String {
        self.lcd_cells().iter().map(|c| c.unwrap_or('?')).collect()
    }

    /// Raw bytes of the LCD's indicators: `[$764E, $764F]`.
    ///
    /// Seen at boot (PRO mode, DEG): `[$40, $23]`. With PockEmul's assignment of bits ($764E: BUSY
    /// $01, SHIFT $02, KANA $04, SML $08, III $10, II $20, I $40, DEF $80; $764F: DE $01, G $02,
    /// RAD $04, RESERVE $10, PRO $20, RUN $40) that is "I", "DEG" and "PRO": a bit at 1 = indicator
    /// on.
    #[must_use]
    pub fn lcd_symbol_bytes(&self) -> [u8; 2] {
        [self.read_byte(0x764E), self.read_byte(0x764F)]
    }

    /// Reads `len` bytes in a row from `addr` (ME0 space if `addr < 0x10000`, ME1 if `0x10000` is
    /// added).
    #[must_use]
    pub fn read_bytes(&self, addr: u32, len: usize) -> Vec<u8> {
        (addr..).take(len).map(|a| self.read_byte(a)).collect()
    }

    /// Reads a 16-bit big-endian word (the order the ROM uses for its pointers in system RAM, e.g.
    /// `BASPRG_ST_H` $7865 / `BASPRG_ST_L` $7866).
    #[must_use]
    pub fn read_word_be(&self, addr: u32) -> u16 {
        u16::from_be_bytes([self.read_byte(addr), self.read_byte(addr + 1)])
    }
}
