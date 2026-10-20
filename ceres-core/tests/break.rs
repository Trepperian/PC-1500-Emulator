//! Tests of the `ON/BRK` key (BREAK), of the `OFF` key, of the automatic power-off and of switching
//! on with `ON`, against the real ROM.
//!
//! `ON/BRK` is not in the keyboard matrix: it reaches the ROM through the LH5810's PB7 pin, latched
//! in bit 1 of its `IF`, which the ROM checks with `CHK_BRK` ($E451) once per BASIC statement, in
//! the keyboard wait (`WAIT_4_KB`) and in the pauses of `WAIT`/`PRINT`. `OFF` (the key or the
//! automatic power-off) runs the CPU's `OFF` instruction, which switches the machine off until `ON`
//! is pressed.

use core::iter;
use std::{fs, path::Path};

use ceres_core::{
    Key, Pc1500,
    display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, LCD_RGBA_OFF, Symbol},
    harness::KeyStroke,
};

/// `BREAKPARAM` ($788A): bit 7 = the program stopped by BREAK/STOP (`BCMD_STOP_7`, $C4D9), bit 6 =
/// there was an `INPUT` pending, bit 5 = pause of `PRINT`.
const BREAKPARAM: u32 = 0x788A;
/// `BRK_LINE` ($78AE/$78AF): line number (big-endian) where the BREAK stopped, the one
/// `BREAK IN nn` shows and `CONT` uses.
const BRK_LINE: u32 = 0x78AE;
/// Signature the ROM leaves at $7A10-$7A1F before switching off.
const OFF_SIGNATURE: u32 = 0x7A10;
/// The ROM's power-off counter, $7B0A-$7B0C.
const APOW_CTR: u32 = 0x7B0A;
/// `BASPRG_ST_H` ($7865)..`$786A`: pointers to the BASIC program.
const PROGRAM_POINTERS: u32 = 0x7865;
/// The `BUSY` bit in `$764E` (see `Pc1500::lcd_symbol_bytes`).
const SYMBOL_BUSY: u8 = 0x01;

/// Maximum of frames waited for the ROM to serve a BREAK.
const BREAK_MAX_FRAMES: usize = 500;

/// Boots, deletes the boot program, types `program` and, if `run_mode`, goes to RUN mode with
/// `MODE`.
fn setup(program: &[&str], run_mode: bool) -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "the ROM did not reach the prompt");
    for line in iter::once("NEW").chain(program.iter().copied()) {
        if let Err(err) = pc.enter_line(line) {
            panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
        }
    }
    if run_mode {
        pc.tap(KeyStroke::plain(Key::Mode));
        assert!(pc.run_until_ready(200).is_some(), "MODE did not go back to the prompt");
    }
    pc
}

/// `Pc1500::type_text`, failing the test if a character has no key.
fn type_keys(pc: &mut Pc1500, text: &str) {
    if let Err(c) = pc.type_text(text) {
        panic!("no key: {c:?}");
    }
}

/// Types `RUN` + ENTER and lets the program run for `frames` frames.
fn run_program(pc: &mut Pc1500, frames: usize) {
    type_keys(pc, "RUN\n");
    pc.run_frames(frames);
}

/// Presses `ON` (1 frame) and waits for the ROM to be ready again.
fn press_break(pc: &mut Pc1500) {
    pc.tap_key_for(Key::On, 1, 0);
    assert!(
        pc.run_until_ready(BREAK_MAX_FRAMES).is_some(),
        "the ROM did not go back to the keyboard after ON; LCD = [{}]",
        pc.lcd_text()
    );
}

fn is_busy(pc: &Pc1500) -> bool {
    pc.lcd_symbol_bytes()[0] & SYMBOL_BUSY != 0
}

/// `true` if the LCD the GUI would see is all blank, indicators included.
fn display_blank(pc: &mut Pc1500) -> bool {
    let display = pc.display();
    !display.is_on()
        && display.rgba_buffer().chunks_exact(4).all(|p| p == LCD_RGBA_OFF)
        && (0..DISPLAY_WIDTH).all(|x| (0..DISPLAY_HEIGHT).all(|y| !display.is_pixel_on(x, y)))
        && Symbol::ALL.iter().all(|&s| !display.is_symbol_on(s))
}

#[test]
fn brk_during_run() {
    let mut pc = setup(&["10 GOTO 10"], true);
    run_program(&mut pc, 30);
    assert!(is_busy(&pc), "the program should be running");
    assert!(!pc.is_waiting_for_key());
    // Instant press and release, with no frame in between: the ON pulse stays latched until the
    // next CPU step.
    pc.press(Key::On);
    pc.release(Key::On);
    assert!(pc.run_until_ready(BREAK_MAX_FRAMES).is_some(), "LCD = [{}]", pc.lcd_text());
    assert_eq!(pc.lcd_text().trim(), "BREAK IN 10");
    assert_eq!(pc.read_word_be(BRK_LINE), 10);
    assert_ne!(pc.read_byte(BREAKPARAM) & 0x80, 0, "BREAKPARAM bit 7");
    assert!(pc.is_powered_on());
}

#[test]
fn brk_cont() {
    let mut pc = setup(&["10 A=0", "20 A=A+1:GOTO 20"], true);
    run_program(&mut pc, 30);
    press_break(&mut pc);
    assert_eq!(pc.lcd_text().trim(), "BREAK IN 20");

    type_keys(&mut pc, "CONT\n");
    pc.run_frames(30);
    assert!(is_busy(&pc), "CONT should resume the program");
    press_break(&mut pc);
    assert_eq!(pc.lcd_text().trim(), "BREAK IN 20");
}

#[test]
fn brk_held_long() {
    let mut pc = setup(&["10 GOTO 10"], true);
    run_program(&mut pc, 30);
    // ON held for ~2.3 s, and "reasserted" halfway (as the operating system's key repeat): a single
    // BREAK. A second pulse would land in the editor (`BCMD_STOP_6` → `WARM_START`) and clear the
    // message.
    pc.press(Key::On);
    pc.run_frames(50);
    pc.press(Key::On);
    pc.run_frames(150);
    pc.release(Key::On);
    assert!(pc.run_until_ready(BREAK_MAX_FRAMES).is_some());
    assert_eq!(pc.lcd_text().trim(), "BREAK IN 10");
}

#[test]
fn brk_input() {
    let mut pc = setup(&["10 INPUT A", "20 GOTO 20"], true);
    type_keys(&mut pc, "RUN\n");
    assert!(pc.run_until_ready(BREAK_MAX_FRAMES).is_some());
    assert!(pc.lcd_text().starts_with('?'), "LCD = [{}]", pc.lcd_text());
    press_break(&mut pc);
    assert_eq!(pc.lcd_text().trim(), "BREAK IN 10");
    assert_eq!(pc.read_byte(BREAKPARAM) & 0xC0, 0xC0, "BREAK + INPUT pending");
}

#[test]
fn brk_wait() {
    // `TIME_DELAY` ($E88C) returns on seeing the BREAK but leaves bit 1 of `IF` latched: it is the
    // next statement boundary that stops the program. (Do not put the `WAIT` on the last line: then
    // `CURR_LINE` is already $FFxx and the number of `BREAK IN` is garbage, which is the ROM's
    // behaviour, not the emulator's.)
    let mut pc = setup(&["10 WAIT 6400:PRINT \"HI\"", "20 GOTO 10"], true);
    run_program(&mut pc, 60);
    assert!(pc.lcd_text().starts_with("HI"), "in the pause: [{}]", pc.lcd_text());
    press_break(&mut pc);
    assert!(pc.lcd_text().trim_start().starts_with("BREAK IN "), "LCD = [{}]", pc.lcd_text());
    assert_ne!(pc.read_byte(BREAKPARAM) & 0x80, 0, "BREAKPARAM bit 7");
}

#[test]
fn idle_on_clears_the_line() {
    let mut pc = setup(&[], false);
    type_keys(&mut pc, "12345");
    assert!(pc.run_until_ready(200).is_some());
    assert!(pc.lcd_text().starts_with("12345"));
    press_break(&mut pc);
    assert_eq!(pc.lcd_text().trim_end(), ">");
    assert!(pc.is_powered_on());
}

#[test]
fn off_key_powers_off_and_on_restarts_keeping_the_program() {
    let mut pc = setup(&["10 PRINT 7"], true);
    let program = pc.read_bytes(0x0100, 16);
    let pointers = pc.read_bytes(PROGRAM_POINTERS, 6);
    assert!(!display_blank(&mut pc));

    pc.tap_key_for(Key::Off, 4, 4);
    pc.run_frames(20);
    assert!(!pc.is_powered_on(), "OFF should switch the machine off");
    assert_eq!(
        pc.read_bytes(OFF_SIGNATURE, 16),
        (0x50..=0x5F).collect::<Vec<u8>>(),
        "signature of BTN_OFF"
    );
    assert!(display_blank(&mut pc), "the LCD should be left blank");
    assert!(!pc.is_waiting_for_key());

    // Off, time goes by but nothing runs.
    let p = pc.cpu().p();
    pc.run_frames(100);
    assert!(!pc.is_powered_on());
    assert_eq!(pc.cpu().p(), p);

    // Switching on: `RESET` sees the signature of OFF and goes to `COLD_START` keeping the program.
    // In debug, this also checks that the CPU reset does not make the timer's counters overflow.
    assert!(pc.power_on().is_some(), "ON did not switch it on; LCD = [{}]", pc.lcd_text());
    assert!(pc.is_powered_on());
    assert!(!display_blank(&mut pc));
    assert_eq!(pc.lcd_text().trim_end(), ">");
    assert_eq!(pc.read_bytes(0x0100, 16), program, "program intact");
    assert_eq!(pc.read_bytes(PROGRAM_POINTERS, 6), pointers, "pointers intact");

    // The first key after switching on is not lost.
    if let Err(err) = pc.enter_line("RUN") {
        panic!("RUN: {err}; LCD = [{}]", pc.lcd_text());
    }
    assert!(pc.lcd_text().trim().ends_with('7'), "LCD = [{}]", pc.lcd_text());
}

#[test]
fn on_while_off_does_not_also_break() {
    let mut pc = setup(&["10 PRINT 7"], false);
    pc.tap_key_for(Key::Off, 4, 4);
    pc.run_frames(5);
    assert!(!pc.is_powered_on());
    // A single long press of ON: it switches on, and neither the held part nor the release generate
    // a BREAK (which would leave the editor in another state or paint something other than the
    // prompt).
    pc.tap_key_for(Key::On, 30, 4);
    assert!(pc.run_until_ready(2000).is_some());
    assert_eq!(pc.lcd_text().trim_end(), ">");
    assert_eq!(pc.read_byte(BREAKPARAM) & 0x80, 0, "no BREAK");
}

#[test]
fn auto_off_and_resume() {
    let mut pc = setup(&[], false);
    assert!(pc.auto_power_off(), "on by default");
    type_keys(&mut pc, "123");
    assert!(pc.run_until_ready(200).is_some());
    // Move the inactivity counter forward to just before the overflow ($E2E4-$E2F0 → `AUTO_OFF`)
    // instead of waiting ~25000 frames.
    for addr in APOW_CTR..APOW_CTR + 3 {
        pc.write_byte(addr, 0xFF);
    }
    let frames = pc.run_until(500, |pc| !pc.is_powered_on());
    assert!(frames.is_some(), "it did not switch itself off");
    assert_eq!(
        pc.read_bytes(OFF_SIGNATURE, 16),
        (0xA0..=0xAF).collect::<Vec<u8>>(),
        "signature of AUTO_OFF"
    );
    assert!((0xE359..=0xE35F).contains(&pc.cpu().p()), "P = {:04X}", pc.cpu().p());
    assert!(display_blank(&mut pc));
    pc.run_frames(100);
    assert!(!pc.is_powered_on(), "still off");

    // `RESET` sees the signature of the automatic power-off and resumes the session (`RESET_22` →
    // `WAIT_4_KB`) with the line half typed.
    assert!(pc.power_on().is_some());
    assert!(pc.is_powered_on());
    assert!(pc.lcd_text().starts_with("123"), "LCD = [{}]", pc.lcd_text());
    type_keys(&mut pc, "4");
    assert!(pc.run_until_ready(200).is_some());
    assert!(pc.lcd_text().starts_with("1234"), "LCD = [{}]", pc.lcd_text());
}

#[test]
fn auto_off_can_be_disabled() {
    let mut pc = setup(&[], false);
    pc.set_auto_power_off(false);
    assert!(!pc.auto_power_off());
    for addr in APOW_CTR..APOW_CTR + 3 {
        pc.write_byte(addr, 0xFF);
    }
    pc.run_frames(200);
    assert!(pc.is_powered_on(), "it should not switch itself off");
    assert_eq!(pc.read_byte(APOW_CTR), 0xFE, "counter set back to its start");
    assert!(pc.run_until_ready(10).is_some(), "still at the prompt");

    // And when it is enabled again, the ROM goes on with its normal count.
    pc.set_auto_power_off(true);
    for addr in APOW_CTR..APOW_CTR + 3 {
        pc.write_byte(addr, 0xFF);
    }
    assert!(pc.run_until(500, |pc| !pc.is_powered_on()).is_some());
}

#[test]
fn load_lh5_while_off_powers_on_without_reset() {
    let mut pc = setup(&[], false);
    pc.tap_key_for(Key::Off, 4, 4);
    pc.run_frames(5);
    assert!(!pc.is_powered_on());

    // Loaded at $0100, 1 byte: `NOP` ($38).
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("break_nop.lh5");
    if let Err(err) = fs::write(&path, [0x00, 0x01, 0x01, 0x00, 0x38]) {
        panic!("{}: {err}", path.display());
    }
    if let Err(err) = pc.load_lh5_file(&path) {
        panic!("{}: {err}", path.display());
    }
    assert!(pc.is_powered_on());
    assert_eq!(pc.cpu().p(), 0x0100);
    pc.step_cpu();
    assert_eq!(pc.cpu().p(), 0x0101, "runs the program loaded, without a reset");
}

#[test]
fn new_machine_is_powered_on() {
    let pc = Pc1500::new_deterministic();
    assert!(pc.is_powered_on());
    assert!(pc.auto_power_off());
}
