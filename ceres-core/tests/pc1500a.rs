//! The PC-1500A model (`Pc1500::new_pc1500a`): 4 KB more RAM, user RAM up
//! to &57FF with the CE-161, which the BASIC loader and `reset` keep.

use ceres_core::{
    Pc1500,
    basic::{Program, VAR_START},
};

const JACKPOT_BIN: &[u8] = include_bytes!("data/jackpot.bin");

#[test]
fn reset_keeps_the_pc1500a() {
    let mut pc = Pc1500::new_pc1500a();
    pc.reset();
    assert!(pc.is_pc1500a(), "still a PC-1500A after reset");
    assert_eq!(pc.user_memory_end(), 0x57FF);

    let mut pc = Pc1500::new();
    pc.reset();
    assert!(!pc.is_pc1500a(), "still a PC-1500 after reset");
    assert_eq!(pc.user_memory_end(), 0x47FF);
}

/// The ROM finds the end of RAM at boot (&7864 = &58), and a BASIC program
/// loaded from the host gets all of it, as one typed on the machine.
#[test]
fn basic_loads_up_to_the_pc1500a_ram_end() {
    let mut pc = Pc1500::new_pc1500a();
    assert!(pc.boot().is_some(), "the ROM reaches its prompt");
    assert_eq!(pc.read_byte(0x7864), 0x58, "end of RAM found by the ROM");
    let (program, _) = Program::from_tokenized(JACKPOT_BIN).unwrap_or_else(|e| panic!("{e}"));
    let report = pc.load_basic_program(&program).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.start, 0x00C5, "where NEW0 puts BASIC with the CE-161");
    assert_eq!(report.free, usize::from(0x5800 - report.end - 1), "free up to &57FF");
    assert_eq!(pc.read_word_be(u32::from(VAR_START)), 0x5800, "variables from the top of RAM");
    assert_eq!(pc.basic_program_bytes(), JACKPOT_BIN, "the program in RAM");
}
