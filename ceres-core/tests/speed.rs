//! Tests of the emulation API by budget of cycles ([`Pc1500::run_for_ticks`], [`Pc1500::ticks`])
//! that the GUI's real-time scheduler uses, of the fast forward of the CPU halted in `HLT`
//! ([`Pc1500::set_fast_halt`]) and of the "per frame" hooks (the brake on the automatic power-off,
//! the latch of `DISP`) not depending on the pieces the emulation is split in. All against the real
//! ROM, with the harness.
//!
//! At the end there is a performance benchmark (`#[ignore]`, it only makes sense in release):
//!
//! ```text
//! cargo test --release -p ceres-core --test speed -- --ignored --nocapture
//! ```

use std::{fs, path::Path, time::Instant};

use ceres_core::{
    CPU_HZ, Key, LCD_PERSISTENCE_TICKS, Pc1500, TICKS_PER_FRAME,
    harness::KeyStroke,
};

/// Cycles per repaint of the GUI at 0.02x on a 60 Hz monitor (`1.3e6 × 0.02 / 60`): the smallest
/// piece that reaches `run_for_ticks` in normal use.
const SLOW_CHUNK: usize = 433;

/// The ROM's power-off counter, $7B0A-$7B0C.
const APOW_CTR: u32 = 0x7B0A;

/// User RAM with the CE-161 ($0000-$47FF) and the system RAM ($7600-$7FFF): everything the ROM can
/// change.
const RAM_RANGES: [(u32, usize); 2] = [(0x0000, 0x4800), (0x7600, 0x0A00)];

/// Maximum of frames waited for the ROM to go back to the keyboard.
const READY_MAX_FRAMES: usize = 600;

fn booted() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "the ROM did not reach the prompt");
    pc
}

fn ram(pc: &Pc1500) -> Vec<u8> {
    RAM_RANGES
        .iter()
        .flat_map(|&(addr, len)| pc.read_bytes(addr, len))
        .collect()
}

fn enter_line(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

fn type_keys(pc: &mut Pc1500, text: &str) {
    if let Err(c) = pc.type_text(text) {
        panic!("no key: {c:?}");
    }
}

fn wait_ready(pc: &mut Pc1500) {
    assert!(
        pc.run_until_ready(READY_MAX_FRAMES).is_some(),
        "the ROM did not go back to the keyboard; LCD = [{}]",
        pc.lcd_text()
    );
}

/// Runs exactly up to cycle `target` in pieces of `chunk` cycles (the last one, shorter). It ends
/// right at `target` if `target` is an instruction boundary of the same emulation done in other
/// pieces (see `chunked_run_matches_whole_frames`).
fn run_chunked_until(pc: &mut Pc1500, target: usize, chunk: usize) {
    while pc.ticks() < target {
        let budget = chunk.min(target - pc.ticks());
        pc.run_for_ticks(budget);
    }
}

/// Writes a `.lh5` to the tests' temporary folder and loads it.
fn load_lh5(pc: &mut Pc1500, name: &str, load_addr: u16, code: &[u8]) {
    let mut file = load_addr.to_le_bytes().to_vec();
    let len = u16::try_from(code.len()).unwrap_or(u16::MAX);
    file.extend_from_slice(&len.to_le_bytes());
    file.extend_from_slice(code);
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if let Err(err) = fs::write(&path, &file) {
        panic!("{}: {err}", path.display());
    }
    if let Err(err) = pc.load_lh5_file(&path) {
        panic!("{}: {err}", path.display());
    }
}

#[test]
fn run_for_ticks_spends_at_least_the_budget() {
    let mut pc = Pc1500::new_deterministic();
    assert_eq!(pc.ticks(), 0);
    assert_eq!(pc.run_for_ticks(0), 0, "budget 0: runs nothing");
    assert_eq!(pc.ticks(), 0);

    let first = pc.run_for_ticks(TICKS_PER_FRAME);
    assert!(
        (TICKS_PER_FRAME..TICKS_PER_FRAME + 64).contains(&first),
        "ran = {first}: at most one instruction too many"
    );
    assert_eq!(pc.ticks(), first);

    // Small pieces, also with the CPU halted in `HLT` (the prompt).
    assert!(pc.boot().is_some());
    for budget in [1, 2, 3, 41, 42, 43, SLOW_CHUNK] {
        let before = pc.ticks();
        let ran = pc.run_for_ticks(budget);
        assert!((budget..budget + 64).contains(&ran), "budget {budget}: ran {ran}");
        assert_eq!(pc.ticks() - before, ran);
    }
}

#[test]
fn step_frame_is_run_for_ticks_of_one_frame() {
    let mut a = Pc1500::new_deterministic();
    let mut b = Pc1500::new_deterministic();
    for _ in 0..300 {
        a.step_frame();
        b.run_for_ticks(TICKS_PER_FRAME);
        assert_eq!(a.ticks(), b.ticks());
    }
    assert_eq!(a.lcd_text(), b.lcd_text());
    assert_eq!(a.lcd_symbol_bytes(), b.lcd_symbol_bytes());
    assert_eq!(ram(&a), ram(&b));
}

/// Splitting the same emulation in pieces of 433 cycles (0.02x in the GUI) or in whole frames gives
/// exactly the same, with and without the fast forward in `HLT`, even ending at the same cycle.
#[test]
fn chunked_run_matches_whole_frames() {
    for fast_halt in [true, false] {
        let mut a = booted();
        let mut b = booted();
        a.set_fast_halt(fast_halt);
        b.set_fast_halt(fast_halt);
        type_keys(&mut a, "12");
        type_keys(&mut b, "12");
        assert_eq!(a.ticks(), b.ticks());

        a.run_frames(400);
        run_chunked_until(&mut b, a.ticks(), SLOW_CHUNK);

        assert_eq!(a.ticks(), b.ticks(), "fast_halt = {fast_halt}");
        assert_eq!(a.lcd_text(), b.lcd_text());
        assert_eq!(a.lcd_symbol_bytes(), b.lcd_symbol_bytes());
        assert_eq!(ram(&a), ram(&b), "fast_halt = {fast_halt}");
        assert_eq!(a.cpu().p(), b.cpu().p());
    }
}

/// A "snapshot" of the visible state after a step of `halt_script`: cycles, LCD, indicators and PC.
type Shot = (usize, String, [u8; 2], u16);

/// A script that exercises the CPU halted in `HLT` in every way a user can think of: cold start,
/// typing and editing with the arrows' key repeat, a program with `WAIT` (which counts pulses of
/// the `µPD1990AC` clock on PB5), BREAK with `ON` (IR2 from the LH5810, the only wake-up that is
/// not the timer) and `OFF`/`ON`. It returns a "snapshot" (cycles, LCD, indicators, PC) after each
/// step and the final RAM.
fn halt_script(fast_halt: bool) -> (Vec<Shot>, Vec<u8>) {
    let mut pc = Pc1500::new_deterministic();
    pc.set_fast_halt(fast_halt);
    assert_eq!(pc.fast_halt(), fast_halt, "interruptor");
    let mut shots = Vec::new();
    let mut shot = |machine: &Pc1500| {
        shots.push((
            machine.ticks(),
            machine.lcd_text(),
            machine.lcd_symbol_bytes(),
            machine.cpu().p(),
        ));
    };

    let boot_frames = pc.boot();
    assert!(boot_frames.is_some(), "the ROM did not reach the prompt");
    shot(&pc);

    enter_line(&mut pc, "NEW");
    enter_line(&mut pc, "10 A=1");
    enter_line(&mut pc, "LIST");
    shot(&pc);

    type_keys(&mut pc, "ABCDEFGH");
    // 4 frames: a single step of the cursor; 100: the ROM's key repeat.
    pc.tap_key_for(Key::Left, 4, 4);
    type_keys(&mut pc, "X");
    wait_ready(&mut pc);
    shot(&pc);
    pc.tap_key_for(Key::Left, 100, 4);
    type_keys(&mut pc, "Y");
    wait_ready(&mut pc);
    shot(&pc);

    // BREAK at the prompt: clears the line.
    pc.tap_key_for(Key::On, 1, 0);
    wait_ready(&mut pc);
    shot(&pc);

    enter_line(&mut pc, "NEW");
    enter_line(&mut pc, "10 WAIT 30:PRINT \"HI\"");
    enter_line(&mut pc, "20 GOTO 10");
    pc.tap(KeyStroke::plain(Key::Mode));
    wait_ready(&mut pc);
    type_keys(&mut pc, "RUN\n");
    pc.run_frames(200);
    shot(&pc);
    pc.tap_key_for(Key::On, 1, 0);
    wait_ready(&mut pc);
    shot(&pc);

    pc.tap_key_for(Key::Off, 4, 4);
    pc.run_frames(20);
    assert!(!pc.is_powered_on(), "OFF did not switch it off");
    shot(&pc);
    assert!(pc.power_on().is_some(), "ON did not switch it on");
    shot(&pc);
    pc.run_frames(300);
    shot(&pc);

    (shots, ram(&pc))
}

/// The fast forward in `HLT` changes nothing in the emulation: the same script gives the same
/// cycles, LCD, indicators, PC and RAM at each step.
#[test]
fn fast_halt_is_equivalent_to_two_state_steps() {
    let (fast_shots, fast_ram) = halt_script(true);
    let (slow_shots, slow_ram) = halt_script(false);
    assert_eq!(fast_shots.len(), slow_shots.len());
    for (i, (fast, slow)) in fast_shots.iter().zip(&slow_shots).enumerate() {
        assert_eq!(fast, slow, "step {i}");
    }
    assert!(fast_ram == slow_ram, "the final RAM differs");

    // And the script really does something (it does not compare two hung machines).
    assert!(fast_shots[2].1.starts_with("ABCDEFGX"), "LCD = [{}]", fast_shots[2].1);
    assert!(fast_shots[4].1.trim_end() == ">", "LCD = [{}]", fast_shots[4].1);
    assert!(fast_shots[6].1.trim_start().starts_with("BREAK IN 10"), "LCD = [{}]", fast_shots[6].1);
}

/// The fast forward is kept by `reset()` (it is an option of the emulator) and `ticks()` goes back
/// to 0.
#[test]
fn reset_keeps_fast_halt_option() {
    let mut pc = booted();
    pc.set_fast_halt(false);
    pc.reset();
    assert!(!pc.fast_halt());
    assert_eq!(pc.ticks(), 0);
    assert!(Pc1500::new().fast_halt(), "on by default");
}

/// The brake on the automatic power-off is a per-emulated-frame hook: it works the same when the
/// GUI emulates in small pieces with `run_for_ticks`.
#[test]
fn auto_off_hold_works_with_small_chunks() {
    let mut pc = booted();
    pc.set_auto_power_off(false);
    for addr in APOW_CTR..APOW_CTR + 3 {
        pc.write_byte(addr, 0xFF);
    }
    let target = pc.ticks() + 200 * TICKS_PER_FRAME;
    run_chunked_until(&mut pc, target, SLOW_CHUNK);
    assert!(pc.is_powered_on(), "it should not switch itself off");
    assert_eq!(pc.read_byte(APOW_CTR), 0xFE, "counter set back to its start");

    // Enabled, the ROM does switch off, also in pieces.
    pc.set_auto_power_off(true);
    for addr in APOW_CTR..APOW_CTR + 3 {
        pc.write_byte(addr, 0xFF);
    }
    let deadline = pc.ticks() + 500 * TICKS_PER_FRAME;
    while pc.is_powered_on() && pc.ticks() < deadline {
        pc.run_for_ticks(SLOW_CHUNK);
    }
    assert!(!pc.is_powered_on(), "it should switch itself off");
}

/// At 0.02x the GUI looks at the LCD every 433 emulated cycles, less than the `RDP`...`SDP` pulse
/// of the idle keyboard loop (2090 cycles, every ~6.3 million at the prompt): with a latch of
/// `DISP` measured between calls to `display()` the LCD would have shown off for several repaints
/// in a row at each pulse. With the persistence in emulated cycles it never goes off.
#[test]
fn lcd_never_blanks_at_slow_motion() {
    let mut pc = booted();
    let mut windows_without_disp = 0;
    let mut window_start = pc.ticks();
    let mut disp_seen = pc.cpu().display_enabled();
    let end = pc.ticks() + 500 * TICKS_PER_FRAME;
    while pc.ticks() < end {
        // One instruction (or one step of `HLT`) at a time, to see `DISP` after each one as the
        // latch sees it.
        pc.run_for_ticks(1);
        disp_seen |= pc.cpu().display_enabled();
        if pc.ticks() - window_start >= SLOW_CHUNK {
            if !disp_seen {
                windows_without_disp += 1;
            }
            assert!(pc.display().is_on(), "LCD off at cycle {}", pc.ticks());
            window_start = pc.ticks();
            disp_seen = pc.cpu().display_enabled();
        }
    }
    assert!(
        windows_without_disp > 0,
        "no window of {SLOW_CHUNK} cycles fell wholly inside the RDP pulse"
    );
}

/// A program that really turns the LCD off (`RDP` in a loop) does show off once the persistence has
/// gone by, and not before.
#[test]
fn lcd_persistence_is_measured_in_emulated_ticks() {
    let mut pc = booted();
    // `RIE` (FD BE), `RDP` (FD C0), `BCH -2` (9E 02): bucle infinito.
    load_lh5(&mut pc, "speed_rdp_loop.lh5", 0x0100, &[0xFD, 0xBE, 0xFD, 0xC0, 0x9E, 0x02]);
    pc.run_for_ticks(20);
    assert!(!pc.cpu().display_enabled());
    let off_since = pc.ticks();
    pc.run_for_ticks(LCD_PERSISTENCE_TICKS / 2);
    assert!(pc.display().is_on(), "still inside the persistence");
    while pc.ticks() < off_since + LCD_PERSISTENCE_TICKS {
        pc.run_for_ticks(SLOW_CHUNK);
    }
    assert!(!pc.display().is_on());
    assert!(!pc.display().is_on(), "second sample without emulating");
}

/// Emulation speed on the host, as a multiple of real time (×100, so as not to use floating point
/// in the core).
fn speed_x100(ticks: usize, micros: u128) -> u128 {
    let ticks = u128::try_from(ticks).unwrap_or(u128::MAX);
    let hz = u128::try_from(CPU_HZ).unwrap_or(1);
    ticks * 100_000_000 / (hz * micros.max(1))
}

fn bench(pc: &mut Pc1500, frames: usize) -> (u128, u128) {
    let start_ticks = pc.ticks();
    let start = Instant::now();
    for _ in 0..frames {
        pc.step_frame();
    }
    let micros = start.elapsed().as_micros();
    let x100 = speed_x100(pc.ticks() - start_ticks, micros);
    (x100, micros)
}

fn print_bench(label: &str, frames: usize, (x100, micros): (u128, u128)) {
    let fps = u128::try_from(frames).unwrap_or(0) * 1_000_000 / micros.max(1);
    let mhz_x100 = x100 * u128::try_from(CPU_HZ).unwrap_or(0) / 1_000_000;
    println!(
        "{label:<32} {frames} frames in {micros} \u{b5}s: {fps} frames/s = x{}.{:02} real time = {}.{:02} MHz",
        x100 / 100,
        x100 % 100,
        mhz_x100 / 100,
        mhz_x100 % 100,
    );
}

/// Benchmark (release only): idle prompt with and without the fast forward in `HLT` and a busy CPU
/// loop (`NOP` / `BCH -3`).
#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn throughput_benchmark() {
    const FRAMES: usize = 3000;

    let mut idle_fast = booted();
    let fast = bench(&mut idle_fast, FRAMES);
    print_bench("idle, fast forward in HLT", FRAMES, fast);

    let mut idle_slow = booted();
    idle_slow.set_fast_halt(false);
    let slow = bench(&mut idle_slow, FRAMES);
    print_bench("idle, HLT 2 by 2", FRAMES, slow);

    let mut busy = booted();
    // $4000: `NOP` (38), `BCH -3` (9E 03).
    load_lh5(&mut busy, "speed_busy_loop.lh5", 0x4000, &[0x38, 0x9E, 0x03]);
    let active = bench(&mut busy, FRAMES);
    print_bench("busy loop NOP/BCH", FRAMES, active);

    // Generous: the reference laptop gives x465-x511 idle with the fast forward and x58-x132
    // without it, with a lot of noise because of frequency scaling.
    assert!(fast.0 > slow.0, "the fast forward should win when idle");
    assert!(fast.0 >= 150 * 100, "idle with fast forward below x150");
}
