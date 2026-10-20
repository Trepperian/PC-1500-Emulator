//! Tests del LCD visto desde la GUI ([`Pc1500::display`]): los
//! indicadores ("annunciators") con su polaridad real (bit a 1 =
//! encendido), los tres segmentos `DE`/`G`/`RAD`, la matriz de puntos
//! frente a la lectura directa de la RAM del arnés, y el enganche de la
//! señal `DISP` que impide que el LCD parpadee en el bucle ocioso de
//! teclado. Todo contra la ROM real, a través del arnés.
//!
//! Las funciones LCD del arnés (`lcd_text`, `lcd_columns`,
//! `lcd_symbol_bytes`) leen la RAM de pantalla tal cual, sin mirar `DISP`
//! ni `BF`: sirven de oráculo independiente para los píxeles, pero el
//! encendido/apagado del LCD solo se puede comprobar con
//! [`Pc1500::display`].

use std::{collections::HashSet, fs, path::Path};

use ceres_core::{
    Key, Pc1500,
    display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, Symbol},
    harness::KeyStroke,
};

/// Máximo de frames que se espera a que la ROM vuelva al teclado tras
/// una tecla de modo/indicador.
const READY_MAX_FRAMES: usize = 300;

fn booted() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "la ROM no lleg\u{f3} al prompt");
    pc
}

/// Los indicadores que se ven encendidos ahora mismo.
fn sym(pc: &mut Pc1500) -> HashSet<Symbol> {
    let display = pc.display();
    Symbol::ALL.into_iter().filter(|&s| display.is_symbol_on(s)).collect()
}

fn set(symbols: &[Symbol]) -> HashSet<Symbol> {
    symbols.iter().copied().collect()
}

/// Pulsa `stroke` y espera a que la ROM vuelva a estar lista.
fn tap(pc: &mut Pc1500, stroke: KeyStroke) {
    pc.tap(stroke);
    assert!(
        pc.run_until_ready(READY_MAX_FRAMES).is_some(),
        "la ROM no volvi\u{f3} al teclado tras {stroke:?}; LCD = [{}]",
        pc.lcd_text()
    );
}

fn enter(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

/// Arranca y pasa a modo RUN (el arranque deja la máquina en PRO).
fn booted_in_run_mode() -> Pc1500 {
    let mut pc = booted();
    tap(&mut pc, KeyStroke::plain(Key::Mode));
    assert!(sym(&mut pc).contains(&Symbol::Run), "MODE deber\u{ed}a pasar a RUN");
    pc
}

/// Indicadores de modo (RUN/PRO/RESERVE) encendidos.
fn mode_symbols(pc: &mut Pc1500) -> HashSet<Symbol> {
    let mode = set(&[Symbol::Run, Symbol::Pro, Symbol::Reserve]);
    sym(pc).intersection(&mode).copied().collect()
}

/// Indicadores de unidad angular (DE/G/RAD) encendidos.
fn angle_symbols(pc: &mut Pc1500) -> HashSet<Symbol> {
    let angle = set(&[Symbol::De, Symbol::G, Symbol::Rad]);
    sym(pc).intersection(&angle).copied().collect()
}

/// Indicadores de nivel de RESERVE (I/II/III) encendidos.
fn reserve_level_symbols(pc: &mut Pc1500) -> HashSet<Symbol> {
    let levels = set(&[Symbol::RomanI, Symbol::RomanII, Symbol::RomanIII]);
    sym(pc).intersection(&levels).copied().collect()
}

/// Regresión de la polaridad: antes `display.rs` encendía cada indicador
/// con su bit a 0, y al arrancar se veía justo lo contrario de la
/// máquina real (BUSY, SHIFT, KANA, SML, II, III, DEF, RAD, RESERVE y la
/// pila encendidos; I, DEG y PRO apagados).
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

/// `SHIFT` + `MODE` = RESERVE; otro `SHIFT` + `MODE` vuelve a RUN (la
/// rotación $10 → $40 de `BTN_MODE`, $CBB1, depende del flag H de
/// `SHR`).
#[test]
fn shift_mode_reserve() {
    let mut pc = booted();
    tap(&mut pc, KeyStroke::shifted(Key::Mode));
    assert_eq!(mode_symbols(&mut pc), set(&[Symbol::Reserve]));
    tap(&mut pc, KeyStroke::shifted(Key::Mode));
    assert_eq!(mode_symbols(&mut pc), set(&[Symbol::Run]));
}

/// `SHIFT` y `DEF` son prefijos que se conmutan y se excluyen entre sí
/// (`AUTO_OFF_15`/`AUTO_OFF_16`, $E3E8/$E3EF).
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
    assert!(!on.contains(&Symbol::Shift), "DEF apaga SHIFT: {on:?}");
    tap(&mut pc, KeyStroke::plain(Key::Control));
    assert!(!sym(&mut pc).contains(&Symbol::Def));
}

/// `SML` conmuta SMALL y nunca KANA: el emulador es el modelo de
/// exportación (PB3 = 1, `AUTO_OFF_17` $E3F6).
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

/// `DEGREE` = DE + G, `RADIAN` = RAD, `GRAD` = G + RAD: tres segmentos
/// distintos (antes `G` y `RAD` compartían un único `Symbol::Rad`).
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

/// En modo RESERVE la tecla ⇕ (`Key::Rsv`) rota el nivel I → II → III
/// → I (`RSV_CHNG`, $CB69), con un único indicador encendido cada vez.
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

/// BUSY encendido mientras corre un programa (y el LCD sigue encendido);
/// un BREAK lo apaga.
#[test]
fn busy_while_running() {
    let mut pc = booted();
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 GOTO 10");
    tap(&mut pc, KeyStroke::plain(Key::Mode));
    if let Err(c) = pc.type_text("RUN\n") {
        panic!("sin tecla: {c:?}");
    }
    pc.run_frames(30);
    assert!(pc.display().is_on());
    assert!(sym(&mut pc).contains(&Symbol::Busy));

    pc.tap_key_for(Key::On, 1, 0);
    assert!(pc.run_until(50, |pc| pc.lcd_symbol_bytes()[0] & 0x01 == 0).is_some());
    assert!(pc.display().is_on());
    assert!(!sym(&mut pc).contains(&Symbol::Busy));
}

/// La matriz de [`Pc1500::display`] coincide punto a punto con la
/// lectura directa de la RAM de pantalla del arnés.
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

/// El bucle ocioso de teclado de la ROM apaga `DISP` ~1.3 ms en cada
/// vuelta (`WAIT_4_KB_17`, $E30D `RDP` ... $E313 `SDP`). Sin el
/// enganche de `DISP`, cada vez que un frame terminaba dentro de ese
/// pulso la GUI pintaba un frame con el LCD en blanco.
#[test]
fn lcd_never_blanks_while_idle() {
    let mut pc = booted();
    let mut disp_off_at_boundary = 0;
    for frame in 0..2000 {
        pc.run_frames(1);
        if !pc.cpu().display_enabled() {
            disp_off_at_boundary += 1;
        }
        assert!(pc.display().is_on(), "LCD apagado en el frame {frame}");
    }
    // Comprueba que el test ejerce de verdad el enganche: algún frame
    // tuvo que terminar con `DISP` desactivado.
    assert!(disp_off_at_boundary > 0, "ning\u{fa}n frame acab\u{f3} dentro del pulso RDP");
}

/// Si `DISP` se queda desactivado un intervalo entero, el LCD sí se ve
/// apagado; el intervalo en el que se desactivó todavía se ve encendido
/// (empezó con `DISP` activo). Y llamar a `display()` dos veces sin
/// emular nada entre medias da lo mismo.
#[test]
fn disp_off_for_a_whole_interval_blanks() {
    let mut pc = booted();
    assert!(pc.display().is_on());
    // $0100: `RIE` (FD BE) para que ninguna interrupción vuelva a la
    // ROM, `RDP` (FD C0) y `BCH -2` (9E 02), un bucle infinito.
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
    assert!(pc.display().is_on(), "el intervalo empez\u{f3} con DISP activo");
    pc.run_frames(1);
    assert!(!pc.display().is_on());
    assert!(!pc.display().is_on(), "segunda muestra sin emular");
    let display = pc.display();
    assert!(Symbol::ALL.iter().all(|&s| !display.is_symbol_on(s)));
    assert!((0..DISPLAY_WIDTH).all(|x| (0..DISPLAY_HEIGHT).all(|y| !display.is_pixel_on(x, y))));
    // La RAM de pantalla sigue intacta: solo está apagado el panel.
    assert_eq!(pc.lcd_symbol_bytes(), [0x40, 0x23]);
}

/// Con la máquina apagada (`OFF`) el panel entero se ve en blanco; al
/// encenderla vuelven los indicadores.
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
