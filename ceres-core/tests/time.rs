//! Tests of `TIME` (reading and `TIME=`) and of the time base of `WAIT` against the real ROM, with
//! the `µPD1990AC` clock in `RtcTimeBase::Emulated`: a fixed time that moves on with the emulated
//! cycles, so everything is deterministic (see `pd1990ac.rs`).
//!
//! `TIME` returns a number `MMDDHH.MMSS` (month, day, hour . minutes, seconds) that the ROM shows
//! normalized: without leading zeros nor trailing zeros in the decimal part (see
//! [`rom_time_text`]).

use core::iter;
use std::fs;

use ceres_core::{Key, Pc1500, RtcDateTime, RtcTimeBase, harness::KeyStroke};

/// Maximum of frames waited for a command to end.
const COMMAND_MAX_FRAMES: usize = 2000;

/// Emulated frames per second: `FRAME_DURATION` is 15000 cycles at 1.3 MHz, ~11.54 ms.
const FRAMES_PER_SECOND: usize = 87;

/// February 28, 23:59:50, Wednesday: 10 s before the change of month.
const FEB_28: RtcDateTime = RtcDateTime {
    month: 2,
    day: 28,
    hour: 23,
    minute: 59,
    second: 50,
    weekday: 3,
};

/// December 31, 23:59:58, Saturday: 2 s before the change of year.
const DEC_31: RtcDateTime = RtcDateTime {
    month: 12,
    day: 31,
    hour: 23,
    minute: 59,
    second: 58,
    weekday: 6,
};

/// Machine with the clock at `start`, booted up to the `>` prompt.
fn booted_at(start: RtcDateTime) -> Pc1500 {
    let mut pc = Pc1500::new();
    pc.set_rtc_time_base(RtcTimeBase::Emulated(start));
    assert!(pc.boot().is_some(), "the ROM did not reach the prompt");
    pc
}

/// `Pc1500::enter_line`, failing the test with the LCD if something goes wrong.
fn enter(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

/// Presses `CL` (clears the result or the error) and waits for the prompt.
fn clear(pc: &mut Pc1500) {
    pc.tap(KeyStroke::plain(Key::Cl));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "CL");
}

/// The text with which the ROM shows the result of `TIME` for the time `t`: the number
/// `MMDDHH.MMSS` normalized (`BCMD_TIME`, $DE82, gives it exponent 5 and normalizes it with
/// `VEJ (E8)`), so without leading zeros, without trailing zeros in the decimals and without a
/// point if the decimals are all 0.
fn rom_time_text(t: RtcDateTime) -> String {
    let int = u32::from(t.month) * 10_000 + u32::from(t.day) * 100 + u32::from(t.hour);
    let frac = format!("{:02}{:02}", t.minute, t.second);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        int.to_string()
    } else {
        format!("{int}.{frac}")
    }
}

/// Types `TIME` + ENTER and returns what the LCD shows, together with the chip's time just before
/// pressing ENTER and just after the command ends: what was read has to be one of the two (the
/// command takes much less than 1 s).
fn read_time(pc: &mut Pc1500) -> (String, RtcDateTime, RtcDateTime) {
    if let Err(c) = pc.type_text("TIME") {
        panic!("no key: {c:?}");
    }
    let before = pc.rtc_now();
    pc.tap(KeyStroke::plain(Key::Enter));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "TIME did not end");
    let after = pc.rtc_now();
    (pc.lcd_text().trim().to_owned(), before, after)
}

/// [`read_time`] checking that the LCD shows the chip's time.
fn read_time_checked(pc: &mut Pc1500) -> String {
    let (shown, before, after) = read_time(pc);
    assert!(
        shown == rom_time_text(before) || shown == rom_time_text(after),
        "TIME shows [{shown}]; the chip was between {before} and {after}"
    );
    shown
}

#[test]
fn time_reads_the_running_clock() {
    let mut pc = booted_at(FEB_28);
    // Booting and typing `TIME` take less than 1 emulated s: Feb 28 23:59:50, shown without the
    // final 0.
    assert_eq!(read_time_checked(&mut pc), "22823.595", "TIME");
    // The number as it is left in ARX ($7A00-$7A06) at the end of `BCMD_TIME`: exponent 4
    // (2.2823595 x 10^4), sign +, BCD mantissa.
    assert_eq!(
        pc.read_bytes(0x7A00, 7),
        [0x04, 0x00, 0x22, 0x82, 0x35, 0x95, 0x00],
        "ARX after BCMD_TIME"
    );
}

#[test]
fn time_set_then_read_after_five_seconds() {
    let mut pc = booted_at(FEB_28);
    enter(&mut pc, "TIME=101912.3045");
    let set = pc.rtc_now();
    assert_eq!(
        (set.month, set.day, set.hour, set.minute, set.second, set.weekday),
        (10, 19, 12, 30, 45, 0),
        "TIME= writes the time (with the day of the week at 0)"
    );
    clear(&mut pc);
    // 435 frames = 5.02 emulated s, plus ~0.5 s of typing `TIME`.
    pc.run_frames(435);
    assert_eq!(read_time_checked(&mut pc), "101912.305", "5 s later");
}

#[test]
fn feb_28_rolls_over_to_march_1() {
    let mut pc = booted_at(FEB_28);
    // No leap years: February 28 is followed by March 1.
    let reached = (0..20 * FRAMES_PER_SECOND).any(|_| {
        pc.run_frames(1);
        pc.rtc_now().month == 3
    });
    assert!(reached, "it did not get to March");
    let now = pc.rtc_now();
    assert_eq!((now.day, now.weekday), (1, 4), "Thursday March 1");
    let shown = read_time_checked(&mut pc);
    assert!(shown.starts_with("30100"), "March 1, 00 h: [{shown}]");
}

#[test]
fn year_rollover() {
    let mut pc = booted_at(DEC_31);
    pc.run_frames(3 * FRAMES_PER_SECOND);
    let shown = read_time_checked(&mut pc);
    assert!(shown.starts_with("10100.000"), "January 1, 00:00:0x: [{shown}]");
    let now = pc.rtc_now();
    assert_eq!((now.month, now.day, now.weekday), (1, 1, 0), "Sunday January 1");
}

#[test]
fn invalid_month_is_error_23() {
    let mut pc = booted_at(FEB_28);
    enter(&mut pc, "TIME=131912.3045");
    assert_eq!(pc.lcd_text().trim(), "ERROR 23", "mes 13");
    assert_eq!(pc.rtc_now().month, 2, "the clock does not change");
    clear(&mut pc);

    // Month 0 the ROM does accept (it only checks < $13).
    enter(&mut pc, "TIME=1012.0005");
    assert_eq!(pc.rtc_now().month, 0, "mes 0");
    clear(&mut pc);
    let shown = read_time_checked(&mut pc);
    assert!(shown.starts_with("1012.000"), "TIME after TIME=1012.0005: [{shown}]");
}

#[test]
fn reset_keeps_the_clock() {
    let mut pc = booted_at(FEB_28);
    pc.set_auto_power_off(false);
    enter(&mut pc, "TIME=101912.3045");
    clear(&mut pc);
    pc.reset();
    // A real cold start...
    assert!(pc.run_until_idle(600).is_some(), "boot after reset");
    assert_eq!(pc.lcd_text().trim(), "NEW0? :CHECK", "cold start");
    // ...that keeps the clock and the automatic power-off option.
    assert!(!pc.auto_power_off(), "automatic power-off");
    clear(&mut pc);
    let shown = read_time_checked(&mut pc);
    assert!(shown.starts_with("101912.30"), "the time set is still there: [{shown}]");
}

/// `TIME_DELAY` ($E88C), the wait of `WAIT`, `PAUSE` and `BEEP`: it sets `TP` to 64 Hz and counts
/// rising edges of PB5, 15.625 ms each. With `U = 63` that is 64 periods: 1 s.
#[test]
fn time_delay_lasts_one_second_per_64_units() {
    // LDI UH,0 ; LDI UL,63 ; SJP $E88C ; HLT
    let code = [0x68_u8, 0x00, 0x6A, 63, 0xBE, 0xE8, 0x8C, 0xFD, 0xB1];
    let mut file = vec![0x00, 0x40, 9, 0x00];
    file.extend_from_slice(&code);
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("time_delay.lh5");
    fs::write(&path, file).expect("write .lh5");

    let mut pc = booted_at(FEB_28);
    pc.load_lh5_file(&path).expect("load .lh5");
    let start = pc.cpu().get_ticks();
    assert!(
        pc.run_until(3 * FRAMES_PER_SECOND, |pc| pc.cpu().is_halted()).is_some(),
        "TIME_DELAY did not end"
    );
    // It is measured in whole frames: up to 15000 cycles too many.
    let cycles = pc.cpu().get_ticks() - start;
    assert!(
        (1_274_000..=1_326_000).contains(&cycles),
        "TIME_DELAY(63) = {cycles} cycles, not ~1.3 M (1 s)"
    );
}

/// `WAIT 64`: each `PRINT` stays on screen for 1 s (64/64) and the program goes on by itself.
#[test]
fn wait_64_shows_each_print_for_one_second() {
    let mut pc = booted_at(FEB_28);
    for line in iter::once("NEW").chain(["10 WAIT 64:PRINT 1:PRINT 2"]) {
        enter(&mut pc, line);
    }
    pc.tap(KeyStroke::plain(Key::Mode));
    assert!(pc.run_until_ready(200).is_some(), "MODE");
    if let Err(c) = pc.type_text("RUN\n") {
        panic!("no key: {c:?}");
    }
    let shows = |text: &'static str| move |m: &Pc1500| m.lcd_text().trim() == text;
    pc.run_until(5 * FRAMES_PER_SECOND, shows("1")).expect("PRINT 1");
    let frames = pc.run_until(5 * FRAMES_PER_SECOND, shows("2")).expect("PRINT 2");
    assert!(
        (FRAMES_PER_SECOND - 6..=FRAMES_PER_SECOND + 6).contains(&frames),
        "PRINT 1 showed for {frames} frames, not ~{FRAMES_PER_SECOND} (1 s)"
    );
}
