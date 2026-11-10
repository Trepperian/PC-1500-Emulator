//! The LH5801 instructions where the emulator was checked against the Technical Manual (TM), MAME
//! and the ROM, each with a few bytes of machine code at $0100: it follows the manual except in
//! `ADR`, whose flags the ROM needs to stay as they were.

use std::{fs, path::Path};

use ceres_core::Pc1500;

/// Flags of the `T` register.
const C: u8 = 0x01;
const Z: u8 = 0x04;
const V: u8 = 0x08;

/// A machine with `code` loaded at $0100, about to run it.
fn loaded(name: &str, code: &[u8]) -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    let mut file = vec![0x00, 0x01, u8::try_from(code.len()).unwrap_or(u8::MAX), 0x00];
    file.extend_from_slice(code);
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cpu_{name}.lh5"));
    if let Err(err) = fs::write(&path, &file) {
        panic!("{}: {err}", path.display());
    }
    if let Err(err) = pc.load_lh5_file(&path) {
        panic!("{}: {err}", path.display());
    }
    pc
}

/// Runs up to the instruction at `addr`, without running it.
fn run_to(pc: &mut Pc1500, addr: u16) {
    for _ in 0..100 {
        if pc.cpu().p() == addr {
            return;
        }
        pc.step_instruction();
    }
    panic!("the CPU never got to {addr:04X}");
}

/// The cycles of the instruction at `addr`.
fn cycles_at(pc: &mut Pc1500, addr: u16) -> usize {
    run_to(pc, addr);
    let before = pc.ticks();
    pc.step_instruction();
    pc.ticks() - before
}

/// The manual's example (TM p. 27): A=$C3 and X=$0A88 give X=$0B4B, and by the manual C=1 H=0
/// Z=0 V=1. The flags stay as they were: the ROM counts on it (`adr` in `lh5801.rs`; ↓ after `LIST`
/// in `tests/basic.rs`).
#[test]
fn adr_adds_and_keeps_the_flags() {
    // RIE, SEC, LDI A,$C3, LDI XL,$88, LDI XH,$0A, ADR X, HLT
    let code = [0xFD, 0xBE, 0xFB, 0xB5, 0xC3, 0x4A, 0x88, 0x48, 0x0A, 0xFD, 0xCA, 0xFD, 0xB1];
    let mut pc = loaded("adr", &code);
    run_to(&mut pc, 0x0109);
    let flags = pc.cpu().t();
    assert_eq!(flags & (C | V), C, "before: C from SEC and no V, so the manual's V=1 would show");
    pc.step_instruction();
    assert_eq!(pc.cpu().x(), 0x0B4B, "the sum, the carry into XH included");
    assert_eq!(pc.cpu().t(), flags, "the flags as they were");
}

/// ITA: the input port to A, and only Z changes (TM p. 44). With no key down the port reads $FF
/// (the ROM turns it round with `EAI $FF`, $E429).
#[test]
fn ita_changes_only_z() {
    // RIE, SEC, LDI A,$00 (Z=1), ITA, HLT
    let code = [0xFD, 0xBE, 0xFB, 0xB5, 0x00, 0xFD, 0xBA, 0xFD, 0xB1];
    let mut pc = loaded("ita", &code);
    run_to(&mut pc, 0x0105);
    let flags = pc.cpu().t();
    assert_ne!(flags & Z, 0);
    pc.step_instruction();
    assert_eq!(pc.cpu().a(), 0xFF, "no key down");
    assert_eq!(pc.cpu().t(), flags & !Z, "Z for the $FF read, the rest as it was");
}

/// A conditional vector not taken goes on to the next instruction and leaves Z as it was; taken,
/// it jumps as `VMJ` does and clears Z (TM p. 50-53).
#[test]
fn a_conditional_vector_clears_z_only_when_taken() {
    for (taken, carry) in [(false, 0xF9), (true, 0xFB)] {
        // RIE, REC or SEC, LDI A,0 (Z=1), VCS $E0, HLT
        let code = [0xFD, 0xBE, carry, 0xB5, 0x00, 0xC3, 0xE0, 0xFD, 0xB1];
        let mut pc = loaded(if taken { "vcs_taken" } else { "vcs_not_taken" }, &code);
        run_to(&mut pc, 0x0105);
        assert_ne!(pc.cpu().t() & Z, 0);
        pc.step_instruction();
        if taken {
            assert_eq!(pc.cpu().p(), pc.read_word_be(0xFFE0), "to the vector's address");
            assert_eq!(pc.cpu().t() & Z, 0, "Z cleared");
        } else {
            assert_eq!(pc.cpu().p(), 0x0107, "the next instruction");
            assert_ne!(pc.cpu().t() & Z, 0, "Z as it was");
        }
    }
}

/// The cycles of the table of TM p. 55-59 where the emulator used to differ: `ORI #(R),i` and
/// `ADI #(R),i` 17 (14 before), `HLT` 9 (8 before), and `CDV` 8 (it was not implemented: the
/// emulator stopped).
#[test]
fn cycles_as_the_manual() {
    let code = [
        0xFD, 0xBE, // RIE
        0x48, 0x00, 0x4A, 0x00, 0x58, 0x00, 0x5A, 0x00, 0x68, 0x00, 0x6A, 0x00, // X = Y = U = 0
        0xFD, 0x4B, 0x01, // $010E ORI #(X),1
        0xFD, 0x5B, 0x01, // $0111 ORI #(Y),1
        0xFD, 0x6B, 0x01, // $0114 ORI #(U),1
        0xFD, 0x4F, 0x01, // $0117 ADI #(X),1
        0xFD, 0x5F, 0x01, // $011A ADI #(Y),1
        0xFD, 0x8E, // $011D CDV
        0xFD, 0xB1, // $011F HLT
    ];
    let mut pc = loaded("cycles", &code);
    for (addr, name) in [(0x010E, "ORI #(X)"), (0x0111, "ORI #(Y)"), (0x0114, "ORI #(U)"), (0x0117, "ADI #(X)"), (0x011A, "ADI #(Y)")] {
        assert_eq!(cycles_at(&mut pc, addr), 17, "{name}");
    }
    assert_eq!(cycles_at(&mut pc, 0x011D), 8, "CDV");
    assert_eq!(cycles_at(&mut pc, 0x011F), 9, "HLT");
    assert!(pc.cpu().is_halted());
}
