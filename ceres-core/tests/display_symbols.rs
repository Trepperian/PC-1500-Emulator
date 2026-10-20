//! Tests of the LCD as the GUI sees it ([`Pc1500::display`]): the indicators ("annunciators") with
//! their real polarity (bit at 1 = on), the three segments `DE`/`G`/`RAD`, the dot matrix against
//! the harness's direct reading of the RAM, and the latch of the `DISP` signal that keeps the LCD
//! from flickering in the idle keyboard loop. All against the real ROM, through the harness.
//!
//! The harness's LCD functions (`lcd_text`, `lcd_columns`, `lcd_symbol_bytes`) read the display RAM
//! as it is, without looking at `DISP` or `BF`: they are an independent oracle for the pixels, but
//! the LCD being on or off can only be checked with [`Pc1500::display`].

use std::{collections::HashSet, fs, path::Path};

use ceres_core::{
    Key, Pc1500,
    display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, Symbol},
    harness::KeyStroke,
};

/// Maximum of frames waited for the ROM to go back to the keyboard after a mode/indicator key.
const READY_MAX_FRAMES: usize = 300;

fn booted() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "the ROM did not reach the prompt");
    pc
}

/// The indicators that show lit right now.
fn sym(pc: &mut Pc1500) -> HashSet<Symbol> {
    let display = pc.display();
    Symbol::ALL.into_iter().filter(|&s| display.is_symbol_on(s)).collect()
}

fn set(symbols: &[Symbol]) -> HashSet<Symbol> {
    symbols.iter().copied().collect()
}

/// Presses `stroke` and waits for the ROM to be ready again.
fn tap(pc: &mut Pc1500, stroke: KeyStroke) {
    pc.tap(stroke);
    assert!(
        pc.run_until_ready(READY_MAX_FRAMES).is_some(),
        "the ROM did not go back to the keyboard after {stroke:?}; LCD = [{}]",
        pc.lcd_text()
    );
}

fn enter(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

/// Boots and goes to RUN mode (the boot leaves the machine in PRO).
fn booted_in_run_mode() -> Pc1500 {
    let mut pc = booted();
    tap(&mut pc, KeyStroke::plain(Key::Mode));
    assert!(sym(&mut pc).contains(&Symbol::Run), "MODE should go to RUN");
    pc
}

/// Mode indicators (RUN/PRO/RESERVE) lit.
fn mode_symbols(pc: &mut Pc1500) -> HashSet<Symbol> {
    let mode = set(&[Symbol::Run, Symbol::Pro, Symbol::Reserve]);
    sym(pc).intersection(&mode).copied().collect()
}

/// Angle unit indicators (DE/G/RAD) lit.
fn angle_symbols(pc: &mut Pc1500) -> HashSet<Symbol> {
    let angle = set(&[Symbol::De, Symbol::G, Symbol::Rad]);
    sym(pc).intersection(&angle).copied().collect()
}

/// RESERVE level indicators (I/II/III) lit.
fn reserve_level_symbols(pc: &mut Pc1500) -> HashSet<Symbol> {
    let levels = set(&[Symbol::RomanI, Symbol::RomanII, Symbol::RomanIII]);
    sym(pc).intersection(&levels).copied().collect()
}

/// Regression of the polarity: `display.rs` used to light each indicator with its bit at 0, and at
/// boot it showed just the opposite of the real machine (BUSY, SHIFT, KANA, SML, II, III, DEF, RAD,
/// RESERVE and the battery on; I, DEG and PRO off).
#[test]
fn boot_annunciators() {
    let mut pc = booted();
    assert_eq!(pc.lcd_symbol_bytes(), [0x40, 0x23]);
    assert!(pc.display().is_on());
    assert_eq!(sym(&mut pc), set(&[Symbol::RomanI, Symbol::De, Symbol::G, Symbol::Pro]));
}

#[test]
fn mode_cycle() {
    let mut pc = booted();
    tap(&mut pc, KeyStroke::plain(Key::Mode));
    assert_eq!(mode_symbols(&mut pc), set(&[Symbol::Run]));
    assert_eq!(pc.lcd_symbol_bytes()[1] & 0x70, 0x40);
    tap(&mut pc, KeyStroke::plain(Key::Mode));
    assert_eq!(mode_symbols(&mut pc), set(&[Symbol::Pro]));
    assert_eq!(pc.lcd_symbol_bytes()[1] & 0x70, 0x20);
}

/// `SHIFT` + `MODE` = RESERVE; another `SHIFT` + `MODE` goes back to RUN (the rotation $10 → $40 of
/// `BTN_MODE`, $CBB1, depends on the H flag of `SHR`).
#[test]
fn shift_mode_reserve() {
    let mut pc = booted();
    tap(&mut pc, KeyStroke::shifted(Key::Mode));
    assert_eq!(mode_symbols(&mut pc), set(&[Symbol::Reserve]));
    tap(&mut pc, KeyStroke::shifted(Key::Mode));
    assert_eq!(mode_symbols(&mut pc), set(&[Symbol::Run]));
}

/// `SHIFT` and `DEF` are prefixes that toggle and exclude each other (`AUTO_OFF_15`/`AUTO_OFF_16`,
/// $E3E8/$E3EF).
#[test]
fn shift_def_latch() {
    let mut pc = booted();
    tap(&mut pc, KeyStroke::plain(Key::Shift));
    assert!(sym(&mut pc).contains(&Symbol::Shift));
    assert!(!sym(&mut pc).contains(&Symbol::Def));
    tap(&mut pc, KeyStroke::plain(Key::Shift));
    assert!(!sym(&mut pc).contains(&Symbol::Shift));

    tap(&mut pc, KeyStroke::plain(Key::Shift));
    tap(&mut pc, KeyStroke::plain(Key::Control));
    let on = sym(&mut pc);
    assert!(on.contains(&Symbol::Def), "{on:?}");
    assert!(!on.contains(&Symbol::Shift), "DEF turns SHIFT off: {on:?}");
    tap(&mut pc, KeyStroke::plain(Key::Control));
    assert!(!sym(&mut pc).contains(&Symbol::Def));
}

/// `SML` toggles SMALL and never KANA: the emulator is the export model (PB3 = 1, `AUTO_OFF_17`
/// $E3F6).
#[test]
fn sml_toggle() {
    let mut pc = booted();
    tap(&mut pc, KeyStroke::plain(Key::Sml));
    let on = sym(&mut pc);
    assert!(on.contains(&Symbol::Small), "{on:?}");
    assert!(!on.contains(&Symbol::Kana), "{on:?}");
    tap(&mut pc, KeyStroke::plain(Key::Sml));
    let after = sym(&mut pc);
    assert!(!after.contains(&Symbol::Small), "{after:?}");
    assert!(!after.contains(&Symbol::Kana), "{after:?}");
}

/// `DEGREE` = DE + G, `RADIAN` = RAD, `GRAD` = G + RAD: three different segments (`G` and `RAD`
/// used to share a single `Symbol::Rad`).
#[test]
fn angle_units() {
    let mut pc = booted_in_run_mode();
    enter(&mut pc, "RADIAN");
    assert_eq!(angle_symbols(&mut pc), set(&[Symbol::Rad]));
    enter(&mut pc, "GRAD");
    assert_eq!(angle_symbols(&mut pc), set(&[Symbol::G, Symbol::Rad]));
    enter(&mut pc, "DEGREE");
    assert_eq!(angle_symbols(&mut pc), set(&[Symbol::De, Symbol::G]));
}

/// In RESERVE mode the ⇕ key (`Key::Rsv`) rotates the level I → II → III → I (`RSV_CHNG`, $CB69),
/// with a single indicator lit each time.
#[test]
fn reserve_levels() {
    let mut pc = booted();
    tap(&mut pc, KeyStroke::shifted(Key::Mode));
    assert_eq!(mode_symbols(&mut pc), set(&[Symbol::Reserve]));
    assert_eq!(reserve_level_symbols(&mut pc), set(&[Symbol::RomanI]));
    for expected in [Symbol::RomanII, Symbol::RomanIII, Symbol::RomanI] {
        tap(&mut pc, KeyStroke::plain(Key::Rsv));
        assert_eq!(reserve_level_symbols(&mut pc), set(&[expected]));
    }
}

/// BUSY on while a program runs (and the LCD stays on); a BREAK turns it off.
#[test]
fn busy_while_running() {
    let mut pc = booted();
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 GOTO 10");
    tap(&mut pc, KeyStroke::plain(Key::Mode));
    if let Err(c) = pc.type_text("RUN\n") {
        panic!("no key: {c:?}");
    }
    pc.run_frames(30);
    assert!(pc.display().is_on());
    assert!(sym(&mut pc).contains(&Symbol::Busy));

    pc.tap_key_for(Key::On, 1, 0);
    assert!(pc.run_until(50, |pc| pc.lcd_symbol_bytes()[0] & 0x01 == 0).is_some());
    assert!(pc.display().is_on());
    assert!(!sym(&mut pc).contains(&Symbol::Busy));
}

/// The matrix of [`Pc1500::display`] matches dot by dot the harness's direct reading of the display
/// RAM.
#[test]
fn pixels_match_harness() {
    let mut pc = booted_in_run_mode();
    enter(&mut pc, "PRINT 12345");
    assert!(pc.lcd_text().trim_end().ends_with("12345"), "LCD = [{}]", pc.lcd_text());
    let columns = pc.lcd_columns();
    assert!(columns.iter().any(|&c| c != 0));
    let display = pc.display();
    assert!(display.is_on());
    assert_eq!(display.columns(), &columns);
    for (x, column) in columns.iter().enumerate() {
        for y in 0..DISPLAY_HEIGHT {
            assert_eq!(display.is_pixel_on(x, y), (column >> y) & 1 != 0, "({x}, {y})");
        }
    }
}

/// The ROM's idle keyboard loop turns `DISP` off ~1.3 ms at each turn (`WAIT_4_KB_17`, $E30D `RDP`
/// ... $E313 `SDP`). Without the latch of `DISP`, each time a frame ended inside that pulse the GUI
/// painted a frame with the LCD blank.
#[test]
fn lcd_never_blanks_while_idle() {
    let mut pc = booted();
    let mut disp_off_at_boundary = 0;
    for frame in 0..2000 {
        pc.run_frames(1);
        if !pc.cpu().display_enabled() {
            disp_off_at_boundary += 1;
        }
        assert!(pc.display().is_on(), "LCD off at frame {frame}");
    }
    // Checks that the test really exercises the latch: some frame had to end with `DISP` off.
    assert!(disp_off_at_boundary > 0, "no frame ended inside the RDP pulse");
}

/// If `DISP` stays off for a whole interval, the LCD does show off; the interval in which it was
/// turned off still shows on (it started with `DISP` on). And calling `display()` twice without
/// emulating anything in between gives the same.
#[test]
fn disp_off_for_a_whole_interval_blanks() {
    let mut pc = booted();
    assert!(pc.display().is_on());
    // $0100: `RIE` (FD BE) so that no interrupt goes back to the ROM, `RDP` (FD C0) and `BCH -2`
    // (9E 02), an endless loop.
    let code = [0xFD, 0xBE, 0xFD, 0xC0, 0x9E, 0x02];
    let mut file = vec![0x00, 0x01, 0x06, 0x00];
    file.extend_from_slice(&code);
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("display_rdp_loop.lh5");
    if let Err(err) = fs::write(&path, &file) {
        panic!("{}: {err}", path.display());
    }
    if let Err(err) = pc.load_lh5_file(&path) {
        panic!("{}: {err}", path.display());
    }
    pc.run_frames(1);
    assert!(!pc.cpu().display_enabled());
    assert!(pc.display().is_on(), "the interval started with DISP on");
    pc.run_frames(1);
    assert!(!pc.display().is_on());
    assert!(!pc.display().is_on(), "second sample without emulating");
    let display = pc.display();
    assert!(Symbol::ALL.iter().all(|&s| !display.is_symbol_on(s)));
    assert!((0..DISPLAY_WIDTH).all(|x| (0..DISPLAY_HEIGHT).all(|y| !display.is_pixel_on(x, y))));
    // The display RAM is still intact: only the panel is off.
    assert_eq!(pc.lcd_symbol_bytes(), [0x40, 0x23]);
}

/// With the machine off (`OFF`) the whole panel shows blank; when it is switched on the indicators
/// come back.
#[test]
fn power_off_blanks() {
    let mut pc = booted();
    pc.tap_key_for(Key::Off, 4, 4);
    pc.run_frames(20);
    assert!(!pc.is_powered_on());
    let display = pc.display();
    assert!(!display.is_on());
    assert!(Symbol::ALL.iter().all(|&s| !display.is_symbol_on(s)));
    assert!((0..DISPLAY_WIDTH).all(|x| (0..DISPLAY_HEIGHT).all(|y| !display.is_pixel_on(x, y))));

    assert!(pc.power_on().is_some());
    assert!(pc.display().is_on());
    assert!(sym(&mut pc).contains(&Symbol::Pro));
}
