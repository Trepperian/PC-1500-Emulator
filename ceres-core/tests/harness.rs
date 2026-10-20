//! Integration tests of the headless harness (`ceres_core::harness`): they boot the real ROM, type
//! as a person would and check what appears on the LCD. They are at the same time an example of how
//! to use the harness for the tests of future features.

use ceres_core::{
    Key, Pc1500,
    harness::{
        BOOT_MAX_FRAMES, EnterLineError, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES, KeyStroke,
        LCD_CHARS, key_for_char, text_to_keystrokes,
    },
};

/// Bit of the `PRO` indicator in `$764F` (see `Pc1500::lcd_symbol_bytes`).
const SYMBOL_PRO: u8 = 0x20;
/// Bit of the `RUN` indicator in `$764F`.
const SYMBOL_RUN: u8 = 0x40;

/// Maximum of frames waited for a command to end.
const COMMAND_MAX_FRAMES: usize = 2000;

/// `BASPRG_ST_H` ($7865): big-endian pointer to the start of the program.
const BASPRG_ST: u32 = 0x7865;
/// `BASPRG_END_H` ($7867): big-endian pointer to the `$FF` that closes the program.
const BASPRG_END: u32 = 0x7867;

fn booted() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "the ROM did not reach the prompt");
    pc
}

/// Types a line with `Pc1500::enter_line` (it waits for the ROM to process it and be ready for the
/// next key) and fails the test with the LCD in the message if something goes wrong.
fn enter_line(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

#[test]
fn cold_boot_shows_new0_check_message() {
    let mut pc = Pc1500::new_deterministic();
    let frames = pc.run_until_idle(BOOT_MAX_FRAMES);
    assert!(frames.is_some(), "the ROM did not reach WAIT_4_KB");
    assert!(pc.is_waiting_for_key(), "it should be in WAIT_4_KB");
    assert_eq!(pc.lcd_text().trim_end(), "NEW0? :CHECK", "cold start message");
}

#[test]
fn boot_reaches_prompt_in_pro_mode() {
    let pc = booted();
    let text = pc.lcd_text();
    assert_eq!(text.chars().count(), LCD_CHARS, "the LCD has 26 cells");
    assert_eq!(text.trim_end(), ">", "prompt after CL");
    let [_, symbols] = pc.lcd_symbol_bytes();
    assert_ne!(symbols & SYMBOL_PRO, 0, "the boot leaves PRO mode");
    assert_eq!(symbols & SYMBOL_RUN, 0, "RUN off in PRO mode");
}

#[test]
fn print_expression_shows_result() {
    let mut pc = booted();
    enter_line(&mut pc, "PRINT 1+2\n");
    // The PC-1500 right-aligns numeric results.
    assert_eq!(pc.lcd_text().trim(), "3", "LCD = [{}]", pc.lcd_text());
}

#[test]
fn program_line_is_stored_and_listed() {
    let mut pc = booted();
    enter_line(&mut pc, "NEW\n");
    enter_line(&mut pc, "10 PRINT 5\n");
    enter_line(&mut pc, "LIST\n");
    assert_eq!(pc.lcd_text().trim_end(), "10:PRINT 5", "LCD = [{}]", pc.lcd_text());

    // The tokenized line in RAM: [line no. BE][length][body + $0D], with PRINT = $F0 $97.
    let start = u32::from(pc.read_word_be(BASPRG_ST));
    assert_eq!(
        pc.read_bytes(start, 7),
        [0x00, 0x0A, 0x04, 0xF0, 0x97, b'5', 0x0D],
        "line 10 tokenized"
    );
}

#[test]
fn program_runs_in_run_mode() {
    let mut pc = booted();
    enter_line(&mut pc, "NEW\n");
    enter_line(&mut pc, "10 PRINT 5\n");
    enter_line(&mut pc, "20 PRINT \"HI\",3*4\n");
    pc.tap(KeyStroke::plain(Key::Mode));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "MODE");
    let [_, symbols] = pc.lcd_symbol_bytes();
    assert_ne!(symbols & SYMBOL_RUN, 0, "MODE goes to RUN");

    enter_line(&mut pc, "RUN\n");
    assert_eq!(pc.lcd_text().trim(), "5", "primer PRINT");
    // PRINT in a program waits for ENTER to go on.
    enter_line(&mut pc, "\n");
    assert_eq!(pc.lcd_text().trim_end(), "HI                      12", "second PRINT");
    enter_line(&mut pc, "\n");
    assert_eq!(pc.lcd_text().trim_end(), ">", "end of the program");
}

#[test]
fn every_supported_char_is_echoed() {
    // All the characters `key_for_char` knows how to type (except ENTER), in pieces of at most 25
    // characters (with 26 the cursor no longer fits and the LCD scrolls left), clearing with CL
    // between pieces.
    let chunks = [
        "ABCDEFGHIJKLM",
        "NOPQRSTUVWXYZ",
        "0123456789 .-+*/=()",
        "!\"#$%&,<>?:;@^[]",
        "abcdefghijklm",
        "nopqrstuvwxyz",
    ];
    let mut pc = booted();
    for chunk in chunks {
        pc.type_text(chunk).expect("all these characters have a key");
        // `lcd_cells` and not `lcd_text`, so that a badly recognized `?` does not pass for a real
        // `?`.
        let cells = pc.lcd_cells();
        let shown: String = cells
            .iter()
            .take(chunk.chars().count())
            .map(|c| c.unwrap_or('\u{FFFD}'))
            .collect();
        assert_eq!(shown, chunk, "echo on the LCD");
        pc.tap(KeyStroke::plain(Key::Cl));
    }
}

#[test]
fn repeated_keys_are_not_lost() {
    let mut pc = booted();
    pc.type_text("AABB1122  ..").expect("supported characters");
    assert!(pc.lcd_text().starts_with("AABB1122  .."), "LCD = [{}]", pc.lcd_text());
}

#[test]
fn default_timing_keeps_a_safety_margin() {
    // Measured: release >= 2 frames and press + release >= 6 are needed. If with one frame less on
    // each side it no longer works, the ROM or the emulator have changed and the timing has to be
    // measured again.
    let text = "QWERTYUIOPASDFGHJKL1234";
    let mut pc = booted();
    pc.type_text_with_timing(text, KEY_HOLD_FRAMES - 1, KEY_RELEASE_FRAMES - 1)
        .expect("supported characters");
    assert!(pc.lcd_text().starts_with(text), "LCD = [{}]", pc.lcd_text());
}

#[test]
fn unsupported_chars_are_reported() {
    assert_eq!(key_for_char('_'), None, "'_' has no key");
    assert_eq!(key_for_char('A'), Some(KeyStroke::plain(Key::A)), "A");
    assert_eq!(key_for_char('a'), Some(KeyStroke::shifted(Key::A)), "a");
    assert_eq!(key_for_char(':'), Some(KeyStroke::shifted(Key::Asterisk)), ":");
    assert_eq!(text_to_keystrokes("10 A=1\r\n").map(|k| k.len()), Ok(7), "CRLF");
    assert_eq!(text_to_keystrokes("PRINT '1'"), Err('\''), "single quote");

    // An `Err` types nothing.
    let mut pc = booted();
    assert_eq!(pc.type_text("AB{"), Err('{'), "brace");
    pc.run_frames(KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES);
    assert_eq!(pc.lcd_text().trim_end(), ">", "LCD intact");
}

/// The lines of the test of typing several lines: `(number, body)`. The bodies have no keywords or
/// spaces, so the ROM keeps them as they are (without tokens) and the expected result can be built
/// byte by byte.
///
/// The long line is line 170 of `jackpot` (41 characters), with which the problem was discovered:
/// with several short lines before it, the ROM takes just long enough to process it to go back to
/// `WAIT_4_KB` with bit 0 of `CURS_CTRL` still at 1 (see `Pc1500::is_ready_for_key`). With the old
/// `type_text` + `run_until_idle` this exact list came back with that bit at 1 after lines 1000,
/// 1060 and 1090, and lost the first `1` of the following ones (1010, 1070 and 1100 arrived as
/// `010`, `070` and `100`, overwriting or creating other lines). Measured when writing this test;
/// the condition depends on the exact moment of the timer's wake-ups, so the test only requires
/// that `enter_lines` works, not that the old method fails.
fn multiline_program() -> Vec<(u16, String)> {
    let short = (1..=9).map(|i| (i * 10, format!("G={i}")));
    let mixed = (0..=10).map(|i| {
        let body = if i % 3 == 0 {
            "SI=112:DO=105:RE=93:MI=82:SO=68:TP=80".to_owned()
        } else {
            format!("A={i}:B={}:C={}", i * 11, i * 111)
        };
        (1000 + i * 10, body)
    });
    short.chain(mixed).collect()
}

#[test]
fn many_program_lines_are_entered_without_losing_keys() {
    let lines = multiline_program();
    assert_eq!(lines.len(), 20, "20 lines");
    let text = lines
        .iter()
        .map(|(n, body)| format!("{n} {body}\n"))
        .collect::<Vec<_>>()
        .concat();

    let mut pc = booted();
    enter_line(&mut pc, "NEW");
    if let Err(err) = pc.enter_lines(&text) {
        panic!("{err}; LCD = [{}]", pc.lcd_text());
    }
    assert!(!pc.lcd_text().contains("ERROR"), "LCD = [{}]", pc.lcd_text());

    // Expected program area: per line [no. BE][length][body][$0D] (the length counts the $0D), and
    // a final $FF at BASPRG_END.
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
    assert_eq!(
        (end + 1).checked_sub(start).map(|len| len as usize),
        Some(expected.len()),
        "length of the program"
    );
    assert_eq!(pc.read_bytes(start, expected.len()), expected, "bytes of the program");
}

#[test]
fn key_pressed_before_release_is_seen_is_swallowed() {
    // The mechanism that makes `is_ready_for_key` necessary: while the ROM has not seen the
    // keyboard empty (bit 0 of CURS_CTRL at 1), any key is taken as the previous one held and is
    // dropped, even if the CPU is already asleep in WAIT_4_KB.
    let mut pc = booted();
    assert!(pc.is_ready_for_key(), "ready after boot()");

    pc.press(Key::A);
    assert!(pc.run_until_idle(COMMAND_MAX_FRAMES).is_some(), "echo of A");
    assert!(pc.is_waiting_for_key(), "in WAIT_4_KB with A held");
    assert!(!pc.is_ready_for_key(), "with A held it is not ready");

    // Releasing A and pressing B at the same instant: the ROM never sees the keyboard empty, so B
    // is lost.
    pc.release(Key::A);
    pc.tap_key_for(Key::B, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES);
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "ready after releasing");
    assert!(pc.is_ready_for_key(), "ready");

    // Once ready, the next key does go in.
    pc.tap(KeyStroke::plain(Key::C));
    assert!(pc.lcd_text().starts_with("AC"), "B lost, C taken; LCD = [{}]", pc.lcd_text());
}

#[test]
fn enter_line_rejects_bad_input_without_typing() {
    let mut pc = booted();
    assert_eq!(pc.enter_line("10 A=1\n20 B=2"), Err(EnterLineError::MultipleLines), "two lines");
    assert_eq!(pc.enter_line("PRINT '1'"), Err(EnterLineError::Unmappable('\'')), "quote");
    assert_eq!(
        pc.enter_lines("10 A=1\n20 B={\n"),
        Err(EnterLineError::Unmappable('{')),
        "error in the second line"
    );
    pc.run_frames(KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES);
    assert_eq!(pc.lcd_text().trim_end(), ">", "nothing was typed");
    // `\r\n` and the final `\n` are optional.
    enter_line(&mut pc, "PRINT 6*7\r\n");
    assert_eq!(pc.lcd_text().trim(), "42", "CRLF");
}

#[test]
fn colon_separates_statements_only_inside_programs() {
    // The ROM's authentic behaviour, not a fault of the emulator: the "end of statement" routine
    // CHECK_AT_END ($DCC5, VEJ C8) only accepts ':' as a separator if CHECK_CALC_MODE ($DF3B, VEJ
    // D8) says a program is running (CURR_LINE $789C/$789D != 0). In direct mode (CURR_LINE = 0) it
    // jumps straight to comparing with $0D ($DCC7 BZS $DCCD), so ':' falls in the error branch.
    let mut pc = booted();
    enter_line(&mut pc, "NEW");
    enter_line(&mut pc, "PRINT 1:PRINT 2");
    assert_eq!(pc.lcd_text().trim(), "ERROR 1", "':' in direct mode");
    // The error message stays until CL is pressed.
    pc.tap(KeyStroke::plain(Key::Cl));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "CL");
    assert_eq!(pc.lcd_text().trim_end(), ">", "CL clears the error");

    // The same line stored as a program line works.
    enter_line(&mut pc, "10 PRINT 1:PRINT 2");
    pc.tap(KeyStroke::plain(Key::Mode));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "MODE");
    enter_line(&mut pc, "RUN");
    assert_eq!(pc.lcd_text().trim(), "1", "first statement");
    enter_line(&mut pc, "");
    assert_eq!(pc.lcd_text().trim(), "2", "second statement after ':'");
    enter_line(&mut pc, "");
    assert_eq!(pc.lcd_text().trim_end(), ">", "end of the program");
}
