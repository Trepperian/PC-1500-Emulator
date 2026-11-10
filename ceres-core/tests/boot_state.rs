//! The state the emulator starts in: a machine initialized as the manual says after changing the
//! batteries (IM p. 2 and 142), whose RAM `src/after_new0.rs` keeps. These tests check that it is
//! still what the ROM leaves; `regenerate_the_ram_after_new0` writes it again (run it with
//! `--ignored` if the ROM, the memory map or the harness's timing ever change). The window, opened
//! with no program, starts instead as with new batteries ([`Pc1500::new_blank`], `NEW0? :CHECK`).

use std::fmt::Write as _;

use ceres_core::{
    Key, Pc1500, RtcTimeBase,
    harness::{HARNESS_RTC_START, KeyStroke},
};

/// First address of the system RAM.
const SYSTEM_RAM_BEGIN: u32 = 0x7600;
/// Bytes of system RAM the image keeps (`&7600-&7BFF`): the ROM uses nothing above.
const SYSTEM_RAM_LEN: usize = 0x600;
/// Last address of the PC-1500A's system RAM.
const PC1500A_SYSTEM_RAM_END: u32 = 0x7FFF;
/// `BASPRG_ST_H` ($7865) and `BASPRG_END_H` ($7867): start and end of BASIC's program.
const BASPRG_ST: u32 = 0x7865;
const BASPRG_END: u32 = 0x7867;
/// `VAR_START` ($7899): where the main area's variables begin, from the end of RAM down.
const VAR_START: u32 = 0x7899;
/// A-Z (8 bytes each) and the four 16-byte areas of A$-Z$ (Tech p. 96-97).
const FIXED_AREAS: [(u32, usize); 4] = [(0x7900, 26 * 8), (0x78C0, 4 * 16), (0x7650, 11 * 16), (0x7750, 11 * 16)];

fn blank(pc1500a: bool) -> Pc1500 {
    let mut pc = if pc1500a { Pc1500::new_pc1500a_blank() } else { Pc1500::new_blank() };
    pc.set_rtc_time_base(RtcTimeBase::Emulated(HARNESS_RTC_START));
    pc
}

fn initialized(pc1500a: bool) -> Pc1500 {
    let mut pc = if pc1500a { Pc1500::new_pc1500a() } else { Pc1500::new() };
    pc.set_rtc_time_base(RtcTimeBase::Emulated(HARNESS_RTC_START));
    pc
}

fn enter_line(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

fn tap(pc: &mut Pc1500, stroke: KeyStroke) {
    pc.tap(stroke);
    assert!(pc.run_until_ready(3000).is_some(), "the ROM was not ready after {stroke:?}");
}

fn lcd(pc: &Pc1500) -> String {
    pc.lcd_text().trim_end().to_owned()
}

/// The manual's procedure on a blank machine: the cold start shows `NEW0? :CHECK`, CL clears it,
/// `NEW0` in PRO mode, SHIFT MODE (RESERVE) and `NEW`, MODE twice (RUN, then PRO), and `OFF`.
fn initialize(pc: &mut Pc1500) {
    assert!(pc.run_until_idle(600).is_some(), "cold start");
    assert_eq!(lcd(pc), "NEW0? :CHECK", "a blank machine");
    assert!(pc.boot().is_some(), "CL");
    enter_line(pc, "NEW0");
    tap(pc, KeyStroke::shifted(Key::Mode));
    assert_eq!(pc.lcd_symbol_bytes()[1] & 0x70, 0x10, "RESERVE mode");
    enter_line(pc, "NEW");
    tap(pc, KeyStroke::plain(Key::Mode));
    tap(pc, KeyStroke::plain(Key::Mode));
    assert_eq!(pc.lcd_symbol_bytes()[1] & 0x70, 0x20, "PRO mode");
    pc.tap_key_for(Key::Off, 4, 4);
    pc.run_frames(20);
    assert!(!pc.is_powered_on(), "OFF");
}

/// User RAM and system RAM, from `&0000` and from `&7600`.
fn ram(pc: &Pc1500) -> (Vec<u8>, Vec<u8>) {
    let user_len = usize::from(pc.user_memory_end()) + 1;
    let system_end = if pc.is_pc1500a() { PC1500A_SYSTEM_RAM_END } else { SYSTEM_RAM_BEGIN + SYSTEM_RAM_LEN as u32 - 1 };
    (pc.read_bytes(0, user_len), pc.read_bytes(SYSTEM_RAM_BEGIN, (system_end - SYSTEM_RAM_BEGIN + 1) as usize))
}

/// The addresses where two RAMs differ, with the value in `b`.
fn changes(a: &(Vec<u8>, Vec<u8>), b: &(Vec<u8>, Vec<u8>)) -> Vec<(u16, u8)> {
    let user = a.0.iter().zip(&b.0).enumerate().filter(|(_, (x, y))| x != y).map(|(i, (_, &y))| (i as u16, y));
    let system = a.1.iter().zip(&b.1).enumerate().filter(|(_, (x, y))| x != y).map(|(i, (_, &y))| ((SYSTEM_RAM_BEGIN as usize + i) as u16, y));
    user.chain(system).collect()
}

#[test]
fn the_ram_after_new0_is_the_rom_s() {
    for pc1500a in [false, true] {
        let mut pc = blank(pc1500a);
        initialize(&mut pc);
        let from_the_rom = ram(&pc);
        let at_start = ram(&initialized(pc1500a));
        let differences = changes(&at_start, &from_the_rom);
        assert!(differences.is_empty(), "pc1500a={pc1500a}: {:04X?}", &differences[..differences.len().min(16)]);
    }
}

#[test]
fn it_starts_at_the_prompt_with_nothing_in_memory() {
    for pc1500a in [false, true] {
        let mut pc = initialized(pc1500a);
        assert!(pc.run_until_ready(600).is_some(), "switching on");
        assert_eq!(lcd(&pc), ">", "no NEW0? :CHECK");
        assert_eq!(pc.lcd_symbol_bytes()[1] & 0x70, 0x20, "PRO mode");
        assert_eq!(pc.read_word_be(BASPRG_ST), 0x00C5, "program start");
        assert_eq!(pc.read_word_be(BASPRG_END), 0x00C5, "no program");
        assert_eq!(pc.read_byte(0x00C5), 0xFF, "the end mark");
        assert_eq!(pc.read_word_be(VAR_START), (pc.user_memory_end() + 1) & 0xFF00, "no main-area variables");
        assert!(pc.read_bytes(0x0008, 0xC5 - 8).iter().all(|&b| b == 0), "the reserve area is empty");
        for (at, len) in FIXED_AREAS {
            assert!(pc.read_bytes(at, len).iter().all(|&b| b == 0), "A-Z and A$-Z$ cleared ({at:04X})");
        }
        // A variable read before it is given a value is 0, not `ERROR 15`.
        tap(&mut pc, KeyStroke::plain(Key::Mode));
        enter_line(&mut pc, "PRINT A;B$;\"!\"");
        assert_eq!(lcd(&pc), " 0!");
    }
}

#[test]
fn reset_starts_the_same_way() {
    let mut pc = initialized(false);
    assert!(pc.boot().is_some());
    enter_line(&mut pc, "10 A=5");
    tap(&mut pc, KeyStroke::plain(Key::Mode));
    enter_line(&mut pc, "RUN");
    pc.reset();
    pc.set_rtc_time_base(RtcTimeBase::Emulated(HARNESS_RTC_START));
    assert_eq!(ram(&pc), ram(&initialized(false)));
}

/// `reset_blank` is the real machine's ALL RESET: the RAM as with new batteries, the model kept,
/// and the ROM's `NEW0? :CHECK` on switching on (what the window shows when opened with no program).
#[test]
fn reset_blank_is_as_with_new_batteries() {
    for pc1500a in [false, true] {
        let mut pc = initialized(pc1500a);
        assert!(pc.boot().is_some());
        enter_line(&mut pc, "10 A=5");
        pc.reset_blank();
        pc.set_rtc_time_base(RtcTimeBase::Emulated(HARNESS_RTC_START));
        assert_eq!(pc.is_pc1500a(), pc1500a, "the model is kept");
        assert_eq!(ram(&pc), ram(&blank(pc1500a)), "pc1500a={pc1500a}");
        assert!(pc.run_until_idle(600).is_some(), "cold start");
        assert_eq!(lcd(&pc), "NEW0? :CHECK");
    }
}

/// Writes `src/after_new0.rs` from what the ROM leaves after [`initialize`].
#[test]
#[ignore = "it rewrites src/after_new0.rs"]
fn regenerate_the_ram_after_new0() {
    let mut pc = blank(false);
    initialize(&mut pc);
    let (user, system) = ram(&pc);
    let system = &system[..SYSTEM_RAM_LEN];
    let user_ram: Vec<(u16, u8)> = user.iter().enumerate().filter(|(_, b)| **b != 0).map(|(i, &b)| (i as u16, b)).collect();

    // A PC-1500A, and the same image put over its blank RAM: what is left are its own bytes.
    let mut pc_a = blank(true);
    initialize(&mut pc_a);
    let from_the_rom = ram(&pc_a);
    let blank_a = ram(&blank(true));
    let mut ours = blank_a.clone();
    ours.1[..SYSTEM_RAM_LEN].copy_from_slice(system);
    for &(addr, value) in &user_ram {
        ours.0[usize::from(addr)] = value;
    }
    let pc1500a_changes = changes(&ours, &from_the_rom);

    let mut out = String::new();
    out.push_str(
        "//! The RAM of a PC-1500 initialized as the manual says after changing the batteries (IM p. 2 and\n\
         //! 142): `NEW0` in PRO mode, `NEW` in RESERVE mode, MODE twice (back to PRO) and `OFF`. It is\n\
         //! what the ROM itself leaves, from a blank machine (`Pc1500::new_blank`): generated by\n\
         //! `tests/boot_state.rs` (`regenerate_the_ram_after_new0`, with `--ignored`), which also checks it\n\
         //! is still the ROM's (`the_ram_after_new0_is_the_rom_s`). Do not edit it by hand.\n\
         //!\n\
         //! The bytes `NEW0` gives a meaning to: BASIC's program empty at `&00C5` (its `&FF`), A-Z\n\
         //! (`&7900-&79CF`) and A$-Z$ (`&78C0`, `&7650`, `&7750`) at 0, the reserve area (`&0008-&00C4`) at 0\n\
         //! and the system pointers (`&7860-&78BF`). The rest is what the ROM had in its work areas\n\
         //! (LCD buffer, stack, input buffer...) when it was switched off.\n\n",
    );
    let _ = writeln!(out, "/// Address of [`SYSTEM_RAM`]'s first byte.");
    let _ = writeln!(out, "pub(crate) const SYSTEM_RAM_BEGIN: u32 = {SYSTEM_RAM_BEGIN:#06X};\n");
    let _ = writeln!(out, "/// `&7600-&7BFF`, the system RAM a PC-1500's ROM works in (on a PC-1500A, which has RAM above,");
    let _ = writeln!(out, "/// the ROM also leaves a `&0D` at `&7C00`: see [`PC1500A_CHANGES`]).");
    let _ = writeln!(out, "#[rustfmt::skip]");
    let _ = writeln!(out, "pub(crate) const SYSTEM_RAM: [u8; {SYSTEM_RAM_LEN:#X}] = [");
    for (row, chunk) in system.chunks(16).enumerate() {
        let bytes: Vec<String> = chunk.iter().map(|b| format!("0x{b:02X},")).collect();
        let _ = writeln!(out, "    {} // &{:04X}", bytes.join(" "), SYSTEM_RAM_BEGIN as usize + row * 16);
    }
    let _ = writeln!(out, "];\n");
    let _ = writeln!(out, "/// The bytes of user RAM that are not 0 (a blank machine's).");
    let _ = writeln!(out, "pub(crate) const USER_RAM: &[(u16, u8)] = &[{}];\n", pairs(&user_ram));
    let _ = writeln!(out, "/// Where a PC-1500A's differs: its RAM ends at `&57FF`, not at `&47FF`.");
    let _ = writeln!(out, "pub(crate) const PC1500A_CHANGES: &[(u16, u8)] = &[{}];", pairs(&pc1500a_changes));
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/after_new0.rs");
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{path}: {e}"));
}

fn pairs(list: &[(u16, u8)]) -> String {
    list.iter().map(|(a, b)| format!("(0x{a:04X}, 0x{b:02X})")).collect::<Vec<_>>().join(", ")
}
