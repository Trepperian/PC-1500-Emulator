//! Tests of the keyboard against the real ROM: the matrix of `keyboard.rs` compared with the ROM's
//! tables, that all the characters with a key can be typed, and the GUI's key feeder
//! (`Pc1500::host_key_down`/`host_key_up`/`host_tap`/`queue_text`), which presses and releases in
//! emulated time.
//!
//! The feeder's tests do not use the harness's synchronous `tap`/`type_text` for what is being
//! tested (only to prepare the machine): the feeder runs by itself, inside `run_for_ticks`, once
//! per emulated frame.

use core::iter;

use ceres_core::{
    Key, KeyStroke, Pc1500, TICKS_PER_FRAME, TYPED_IDLE_TIMEOUT_FRAMES,
    harness::{KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES, key_for_char},
    keyboard::MATRIX,
};

/// `KB_LAYOUT_NORM` ($FE80): code of each position of the matrix without `SHIFT`; `KB_LAYOUT_SH`
/// ($FEC0) right after it, with `SHIFT`.
const KB_LAYOUT_NORM: u32 = 0xFE80;
/// The `SHIFT` bit of the indicator byte $764E.
const SYMBOL_SHIFT: u8 = 0x02;
/// `BASPRG_ST_H` ($7865): big-endian pointer to the start of the program.
const BASPRG_ST: u32 = 0x7865;
/// `BASPRG_END_H` ($7867): big-endian pointer to the final `$FF`.
const BASPRG_END: u32 = 0x7867;
/// Maximum of frames waited for the ROM after typing something.
const COMMAND_MAX_FRAMES: usize = 2000;

fn booted() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "the ROM did not reach the prompt");
    pc
}

fn enter_line(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

/// The first `len` cells of the LCD as text (`lcd_cells`, so that an unrecognized `?` does not pass
/// for a real `?`).
fn lcd_prefix(pc: &Pc1500, len: usize) -> String {
    pc.lcd_cells()
        .iter()
        .take(len)
        .map(|c| c.unwrap_or('\u{FFFD}'))
        .collect()
}

/// Lets the machine run until the feeder finishes and the ROM is ready for the next key.
fn finish_feeding(pc: &mut Pc1500, max_frames: usize) {
    assert!(
        pc.run_until(max_frames, |pc| pc.pending_keystrokes() == 0).is_some(),
        "the feeder did not finish; LCD = [{}]",
        pc.lcd_text()
    );
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "LCD = [{}]", pc.lcd_text());
}

/// Presses and releases the live key `id` at the same instant (the shortest a host can do).
fn live_tap(pc: &mut Pc1500, id: u32, stroke: KeyStroke) {
    pc.host_key_down(id, stroke);
    pc.host_key_up(id);
}

#[test]
fn matrix_matches_rom() {
    let pc = Pc1500::new_deterministic();
    let rom = pc.read_bytes(KB_LAYOUT_NORM, 128);
    for (row, keys) in MATRIX.iter().enumerate() {
        for (bit, key) in keys.iter().enumerate() {
            // `KEY_2_ASCII` ($E42C): index = 8 × row + (7 − bit).
            let index = 8 * row + 7 - bit;
            assert_eq!(Some(rom[index]), key.rom_code(false), "{key:?} without SHIFT");
            assert_eq!(Some(rom[64 + index]), key.rom_code(true), "{key:?} with SHIFT");
        }
    }
}

#[test]
fn matrix_is_complete() {
    assert_eq!(Key::ALL.len(), 65, "64 of the matrix + ON/BRK");
    for key in Key::ALL {
        let count = MATRIX.iter().flatten().filter(|&&k| k == key).count();
        let expected = usize::from(key != Key::On);
        assert_eq!(count, expected, "{key:?} in the matrix");
        assert_eq!(key.matrix_pos().is_some(), key != Key::On, "{key:?}");
    }
    assert_eq!(Key::On.rom_code(false), None);
}

#[test]
fn all_shifted_chars_are_typeable_as_prefix() {
    let text = "\"(<>?:;,@^[]!#$%&)abcXYZ";
    let mut pc = booted();
    pc.type_text(text).expect("all have a key");
    assert_eq!(lcd_prefix(&pc, text.len()), text);
    assert_eq!(pc.lcd_symbol_bytes()[0] & SYMBOL_SHIFT, 0, "SHIFT is not left latched");
}

#[test]
fn every_printable_ascii_char_either_maps_or_is_known_unmappable() {
    let unmappable: Vec<char> = (' '..='~').filter(|&c| key_for_char(c).is_none()).collect();
    assert_eq!(unmappable, ['\'', '\\', '_', '`', '{', '|', '}', '~']);
}

/// Documents the ROM (and why `SHIFT` is a prefix, not a modifier): with `SHIFT` held at the same
/// time as `(`, `KEY_2_ASCII` only sees `SHIFT` (row 0, before anything), so `<` does not come out
/// and the `SHIFT` indicator stays latched.
#[test]
fn holding_shift_with_another_key_types_nothing() {
    let mut pc = booted();
    pc.press(Key::Shift);
    pc.press(Key::LeftParen);
    pc.run_frames(6);
    pc.release(Key::LeftParen);
    pc.release(Key::Shift);
    pc.run_frames(KEY_RELEASE_FRAMES * 2);
    assert!(!pc.lcd_text().contains('<'), "LCD = [{}]", pc.lcd_text());
    assert_ne!(pc.lcd_symbol_bytes()[0] & SYMBOL_SHIFT, 0, "SHIFT latched");
}

#[test]
fn feeder_live_tap_shorter_than_the_minimum_is_not_lost() {
    let mut pc = booted();
    pc.host_key_down(1, KeyStroke::plain(Key::A));
    pc.run_frames(1);
    pc.host_key_up(1);
    pc.run_frames(20);
    assert_eq!(lcd_prefix(&pc, 1), "A");
    assert_ne!(pc.lcd_cells()[1], Some('A'), "a single A");
    assert_eq!(pc.pending_keystrokes(), 0);
}

#[test]
fn feeder_burst_within_one_frame_keeps_every_key() {
    let mut pc = booted();
    for (id, key) in [Key::H, Key::E, Key::L, Key::L, Key::O].into_iter().enumerate() {
        live_tap(&mut pc, u32::try_from(id).expect("small"), KeyStroke::plain(key));
    }
    pc.run_frames(80);
    assert_eq!(lcd_prefix(&pc, 6), "HELLO_", "the repeated L is not lost");
}

#[test]
fn feeder_rollover_newest_key_wins() {
    let mut pc = booted();
    pc.host_key_down(1, KeyStroke::plain(Key::H));
    pc.host_key_down(2, KeyStroke::plain(Key::I));
    pc.host_key_up(1);
    pc.host_key_up(2);
    pc.run_frames(40);
    assert_eq!(lcd_prefix(&pc, 2), "HI");
}

#[test]
fn feeder_live_hold_uses_rom_auto_repeat() {
    let mut pc = booted();
    pc.type_text("ABCDEFGHIJ").expect("letters");
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some());
    // Holding ← (on the host) for 120 frames: the ROM repeats it up to column 0.
    pc.host_key_down(2, KeyStroke::plain(Key::Left));
    pc.run_frames(120);
    pc.host_key_up(2);
    live_tap(&mut pc, 3, KeyStroke::plain(Key::Z));
    pc.run_frames(40);
    assert_eq!(lcd_prefix(&pc, 10), "ZBCDEFGHIJ", "Z overwrites column 0");
}

#[test]
fn feeder_live_shifted_key_taps_shift_as_prefix() {
    let mut pc = booted();
    live_tap(&mut pc, 3, KeyStroke::shifted(Key::LeftParen));
    pc.run_frames(40);
    assert_eq!(lcd_prefix(&pc, 1), "<");
    assert_eq!(pc.lcd_symbol_bytes()[0] & SYMBOL_SHIFT, 0, "SHIFT used up");
}

#[test]
fn feeder_respects_an_explicitly_latched_shift() {
    let mut pc = booted();
    // F11 in the GUI: SHIFT alone, as a prefix...
    pc.host_tap(&[KeyStroke::plain(Key::Shift)]);
    pc.run_frames(20);
    assert_ne!(pc.lcd_symbol_bytes()[0] & SYMBOL_SHIFT, 0, "SHIFT latched");
    // ... and then a normal A of the host: the lowercase letter comes out.
    live_tap(&mut pc, 1, KeyStroke::plain(Key::A));
    pc.run_frames(20);
    assert_eq!(lcd_prefix(&pc, 1), "a");
    assert_eq!(pc.lcd_symbol_bytes()[0] & SYMBOL_SHIFT, 0);
}

#[test]
fn queue_text_is_exact_with_sml() {
    let mut pc = booted();
    pc.tap(KeyStroke::plain(Key::Sml));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some());
    assert!(pc.queue_text("Ab").is_empty());
    finish_feeding(&mut pc, 200);
    assert_eq!(lcd_prefix(&pc, 2), "Ab", "SML swaps case, the feeder makes up for it");
}

#[test]
fn queue_text_types_every_mappable_char() {
    let mut pc = booted();
    let text = "!\"#$%&,<>?:;@^[]abc";
    assert!(pc.queue_text(text).is_empty());
    finish_feeding(&mut pc, 1000);
    assert_eq!(lcd_prefix(&pc, text.len()), text);
}

#[test]
fn queue_text_skips_unmappable_chars() {
    let mut pc = booted();
    assert_eq!(pc.queue_text("A_B'"), ['_', '\'']);
    assert_eq!(pc.pending_keystrokes(), 2);
    finish_feeding(&mut pc, 200);
    assert_eq!(lcd_prefix(&pc, 3), "AB_");
}

/// A listing of 20 lines pasted at once: the feeder waits for the ROM to finish each ENTER
/// (tokenizing and inserting the line) before the next key, so no character is lost.
#[test]
fn queue_text_survives_enter_processing() {
    let mut pc = booted();
    enter_line(&mut pc, "NEW");
    let lines: Vec<(u16, String)> = (1..=20_u8)
        .map(|i| (u16::from(i) * 10, format!("{}={i}", char::from(b'A' + i - 1))))
        .collect();
    let text: String = lines.iter().map(|(n, body)| format!("{n} {body}\r\n")).collect();
    assert!(pc.queue_text(&text).is_empty());
    finish_feeding(&mut pc, 20000);
    assert!(!pc.lcd_text().contains("ERROR"), "LCD = [{}]", pc.lcd_text());

    let mut expected = Vec::new();
    for (number, body) in &lines {
        expected.extend(number.to_be_bytes());
        expected.push(u8::try_from(body.len() + 1).expect("short line"));
        expected.extend(body.bytes());
        expected.push(0x0D);
    }
    expected.push(0xFF);
    let start = u32::from(pc.read_word_be(BASPRG_ST));
    let end = u32::from(pc.read_word_be(BASPRG_END));
    assert_eq!(end + 1 - start, u32::try_from(expected.len()).expect("short"), "length");
    assert_eq!(pc.read_bytes(start, expected.len()), expected, "bytes of the program");
}

#[test]
fn release_all_host_keys_never_leaves_a_key_stuck() {
    let mut pc = booted();
    pc.host_key_down(1, KeyStroke::plain(Key::A));
    pc.host_key_down(2, KeyStroke::plain(Key::On));
    pc.run_frames(30);
    pc.release_all_host_keys();
    pc.run_frames(20);
    assert_eq!(lcd_prefix(&pc, 1), "A");
    assert_ne!(pc.lcd_cells()[1], Some('A'), "a single A (no key repeat)");
    let stuck: Vec<Key> = Key::ALL.into_iter().filter(|&k| pc.is_key_pressed(k)).collect();
    assert!(stuck.is_empty(), "stuck keys: {stuck:?}");
    assert_eq!(pc.pending_keystrokes(), 0);
}

#[test]
fn break_has_priority_and_cancels_pasted_text() {
    let mut pc = booted();
    for line in ["NEW", "10 GOTO 10"] {
        enter_line(&mut pc, line);
    }
    pc.tap(KeyStroke::plain(Key::Mode));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "MODE");
    pc.type_text("RUN\n").expect("RUN");
    pc.run_frames(20);

    // The program does not wait for the keyboard: the pasted text waits for the ROM...
    assert!(pc.queue_text("PRINT 12345\n").is_empty());
    pc.run_frames(50);
    assert!(pc.pending_keystrokes() > 0, "waiting for the ROM");
    // ... and ON/BRK jumps the queue and cancels it.
    pc.host_key_down(9, KeyStroke::plain(Key::On));
    pc.run_frames(3);
    pc.host_key_up(9);
    assert_eq!(pc.pending_keystrokes(), 0, "pasted text cancelled");
    assert!(pc.run_until_ready(500).is_some(), "LCD = [{}]", pc.lcd_text());
    // Much more than the pasted text's time limit: nothing arrives.
    pc.run_frames(TYPED_IDLE_TIMEOUT_FRAMES + 50);
    assert_eq!(pc.lcd_text().trim(), "BREAK IN 10");
    assert!(!pc.is_key_pressed(Key::On));
}

/// The feeder counts EMULATED frames (every `TICKS_PER_FRAME` cycles, inside `run_for_ticks`), not
/// calls: emulating in small pieces, as the GUI does in slow motion, gives exactly the same as
/// whole frames.
#[test]
fn feeder_timing_does_not_depend_on_chunk_size() {
    let script = |pc: &mut Pc1500, chunk: usize| {
        live_tap(pc, 1, KeyStroke::shifted(Key::F2));
        pc.host_key_down(2, KeyStroke::plain(Key::B));
        let total = 60 * TICKS_PER_FRAME;
        let mut done = 0;
        let mut released = false;
        while done < total {
            done += pc.run_for_ticks(chunk);
            if !released && done >= 25 * TICKS_PER_FRAME {
                pc.host_key_up(2);
                released = true;
            }
        }
    };
    let mut frames = booted();
    script(&mut frames, TICKS_PER_FRAME);
    let mut slow = booted();
    script(&mut slow, 433);

    assert_eq!(lcd_prefix(&frames, 2), "\"B");
    assert_eq!(frames.lcd_cells(), slow.lcd_cells(), "same LCD");
    assert_eq!(
        frames.read_bytes(0x7600, 0x200),
        slow.read_bytes(0x7600, 0x200),
        "same display RAM"
    );
}

/// A `SHIFT` of the feeder and the key never overlap, and the timing is the minimum measured by the
/// harness: 8 frames per key, 16 with `SHIFT`.
#[test]
fn feeder_uses_the_harness_timing() {
    let mut pc = booted();
    pc.host_tap(&[KeyStroke::shifted(Key::Slash), KeyStroke::plain(Key::One)]);
    let mut pressed = Vec::new();
    for _ in 0..3 * (KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES) {
        pc.step_frame();
        pressed.push(Key::ALL.into_iter().filter(|&k| pc.is_key_pressed(k)).collect::<Vec<_>>());
    }
    let expected: Vec<Vec<Key>> = iter::empty()
        .chain(iter::repeat_n(vec![Key::Shift], KEY_HOLD_FRAMES))
        .chain(iter::repeat_n(vec![], KEY_RELEASE_FRAMES))
        .chain(iter::repeat_n(vec![Key::Slash], KEY_HOLD_FRAMES))
        .chain(iter::repeat_n(vec![], KEY_RELEASE_FRAMES))
        .chain(iter::repeat_n(vec![Key::One], KEY_HOLD_FRAMES))
        .chain(iter::repeat_n(vec![], KEY_RELEASE_FRAMES))
        .collect();
    assert_eq!(pressed, expected);
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some());
    assert_eq!(lcd_prefix(&pc, 2), "?1");
}
