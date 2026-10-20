//! Tests de la tecla `ON/BRK` (BREAK), de la tecla `OFF`, del
//! auto-apagado y del encendido con `ON`, contra la ROM real.
//!
//! `ON/BRK` no está en la matriz de teclado: llega a la ROM por la patilla
//! PB7 del LH5810, enganchada en el bit 1 de su `IF`, que la ROM consulta
//! con `CHK_BRK` ($E451) una vez por sentencia BASIC, en la espera de
//! teclado (`WAIT_4_KB`) y en las pausas de `WAIT`/`PRINT`. `OFF` (tecla
//! o auto-apagado) ejecuta la instrucción `OFF` de la CPU, que apaga la
//! máquina hasta que se pulsa `ON`.

use core::iter;
use std::{fs, path::Path};

use ceres_core::{
    Key, Pc1500,
    display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, LCD_RGBA_OFF, Symbol},
    harness::KeyStroke,
};

/// `BREAKPARAM` ($788A): bit 7 = el programa se paró por BREAK/STOP
/// (`BCMD_STOP_7`, $C4D9), bit 6 = había un `INPUT` pendiente, bit 5 =
/// pausa de `PRINT`.
const BREAKPARAM: u32 = 0x788A;
/// `BRK_LINE` ($78AE/$78AF): número de línea (big-endian) donde paró el
/// BREAK, el que muestra `BREAK IN nn` y usa `CONT`.
const BRK_LINE: u32 = 0x78AE;
/// Firma que deja la ROM en $7A10-$7A1F antes de apagarse.
const OFF_SIGNATURE: u32 = 0x7A10;
/// Contador de auto-apagado de la ROM, $7B0A-$7B0C.
const APOW_CTR: u32 = 0x7B0A;
/// `BASPRG_ST_H` ($7865)..`$786A`: punteros al programa BASIC.
const PROGRAM_POINTERS: u32 = 0x7865;
/// Bit `BUSY` en `$764E` (ver `Pc1500::lcd_symbol_bytes`).
const SYMBOL_BUSY: u8 = 0x01;

/// Máximo de frames que se espera a que la ROM atienda un BREAK.
const BREAK_MAX_FRAMES: usize = 500;

/// Arranca, borra el programa del fixture, teclea `program` y, si
/// `run_mode`, pasa a modo RUN con `MODE`.
fn setup(program: &[&str], run_mode: bool) -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "la ROM no lleg\u{f3} al prompt");
    for line in iter::once("NEW").chain(program.iter().copied()) {
        if let Err(err) = pc.enter_line(line) {
            panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
        }
    }
    if run_mode {
        pc.tap(KeyStroke::plain(Key::Mode));
        assert!(pc.run_until_ready(200).is_some(), "MODE no volvi\u{f3} al prompt");
    }
    pc
}

/// `Pc1500::type_text`, fallando el test si algún carácter no tiene tecla.
fn type_keys(pc: &mut Pc1500, text: &str) {
    if let Err(c) = pc.type_text(text) {
        panic!("sin tecla: {c:?}");
    }
}

/// Teclea `RUN` + ENTER y deja correr el programa `frames` frames.
fn run_program(pc: &mut Pc1500, frames: usize) {
    type_keys(pc, "RUN\n");
    pc.run_frames(frames);
}

/// Pulsa `ON` (1 frame) y espera a que la ROM vuelva a estar lista.
fn press_break(pc: &mut Pc1500) {
    pc.tap_key_for(Key::On, 1, 0);
    assert!(
        pc.run_until_ready(BREAK_MAX_FRAMES).is_some(),
        "la ROM no volvi\u{f3} al teclado tras ON; LCD = [{}]",
        pc.lcd_text()
    );
}

fn is_busy(pc: &Pc1500) -> bool {
    pc.lcd_symbol_bytes()[0] & SYMBOL_BUSY != 0
}

/// `true` si el LCD que vería la GUI está entero en blanco, indicadores
/// incluidos.
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
    assert!(is_busy(&pc), "el programa deber\u{ed}a estar en marcha");
    assert!(!pc.is_waiting_for_key());
    // Pulsación y suelta instantáneas, sin ningún frame entre medias: el
    // pulso de ON queda enganchado hasta el siguiente paso de CPU.
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
    assert!(is_busy(&pc), "CONT deber\u{ed}a reanudar el programa");
    press_break(&mut pc);
    assert_eq!(pc.lcd_text().trim(), "BREAK IN 20");
}

#[test]
fn brk_held_long() {
    let mut pc = setup(&["10 GOTO 10"], true);
    run_program(&mut pc, 30);
    // ON mantenida ~2.3 s, y "reafirmada" a mitad (como la
    // autorrepetición del sistema operativo): un solo BREAK. Un segundo
    // pulso caería en el editor (`BCMD_STOP_6` → `WARM_START`) y
    // borraría el mensaje.
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
    assert_eq!(pc.read_byte(BREAKPARAM) & 0xC0, 0xC0, "BREAK + INPUT pendiente");
}

#[test]
fn brk_wait() {
    // `TIME_DELAY` ($E88C) devuelve al ver el BREAK pero deja el bit 1 de
    // `IF` enganchado: es la siguiente frontera de sentencia la que para
    // el programa. (No poner el `WAIT` en la última línea: entonces
    // `CURR_LINE` ya vale $FFxx y el número de `BREAK IN` es basura, que
    // es comportamiento de la ROM, no del emulador.)
    let mut pc = setup(&["10 WAIT 6400:PRINT \"HI\"", "20 GOTO 10"], true);
    run_program(&mut pc, 60);
    assert!(pc.lcd_text().starts_with("HI"), "en la pausa: [{}]", pc.lcd_text());
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
    assert!(!pc.is_powered_on(), "OFF deber\u{ed}a apagar la m\u{e1}quina");
    assert_eq!(
        pc.read_bytes(OFF_SIGNATURE, 16),
        (0x50..=0x5F).collect::<Vec<u8>>(),
        "firma de BTN_OFF"
    );
    assert!(display_blank(&mut pc), "el LCD deber\u{ed}a quedar en blanco");
    assert!(!pc.is_waiting_for_key());

    // Apagada, el tiempo pasa pero no se ejecuta nada.
    let p = pc.cpu().p();
    pc.run_frames(100);
    assert!(!pc.is_powered_on());
    assert_eq!(pc.cpu().p(), p);

    // Encender: `RESET` ve la firma de OFF y va a `COLD_START`
    // conservando el programa. En debug, esto también comprueba que el
    // reset de CPU no hace desbordar los contadores del temporizador.
    assert!(pc.power_on().is_some(), "ON no encendi\u{f3}; LCD = [{}]", pc.lcd_text());
    assert!(pc.is_powered_on());
    assert!(!display_blank(&mut pc));
    assert_eq!(pc.lcd_text().trim_end(), ">");
    assert_eq!(pc.read_bytes(0x0100, 16), program, "programa intacto");
    assert_eq!(pc.read_bytes(PROGRAM_POINTERS, 6), pointers, "punteros intactos");

    // La primera tecla tras encender no se pierde.
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
    // Una sola pulsación larga de ON: enciende, y ni la parte mantenida
    // ni la suelta generan un BREAK (que dejaría el editor en otro
    // estado o pintaría algo distinto del prompt).
    pc.tap_key_for(Key::On, 30, 4);
    assert!(pc.run_until_ready(2000).is_some());
    assert_eq!(pc.lcd_text().trim_end(), ">");
    assert_eq!(pc.read_byte(BREAKPARAM) & 0x80, 0, "sin BREAK");
}

#[test]
fn auto_off_and_resume() {
    let mut pc = setup(&[], false);
    assert!(pc.auto_power_off(), "activado por defecto");
    type_keys(&mut pc, "123");
    assert!(pc.run_until_ready(200).is_some());
    // Adelantar el contador de inactividad hasta justo antes del
    // desbordamiento ($E2E4-$E2F0 → `AUTO_OFF`) en vez de esperar ~25000
    // frames.
    for addr in APOW_CTR..APOW_CTR + 3 {
        pc.write_byte(addr, 0xFF);
    }
    let frames = pc.run_until(500, |pc| !pc.is_powered_on());
    assert!(frames.is_some(), "no se auto-apag\u{f3}");
    assert_eq!(
        pc.read_bytes(OFF_SIGNATURE, 16),
        (0xA0..=0xAF).collect::<Vec<u8>>(),
        "firma de AUTO_OFF"
    );
    assert!((0xE359..=0xE35F).contains(&pc.cpu().p()), "P = {:04X}", pc.cpu().p());
    assert!(display_blank(&mut pc));
    pc.run_frames(100);
    assert!(!pc.is_powered_on(), "sigue apagada");

    // `RESET` ve la firma del auto-apagado y reanuda la sesión
    // (`RESET_22` → `WAIT_4_KB`) con la línea a medio teclear.
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
    assert!(pc.is_powered_on(), "no deber\u{ed}a auto-apagarse");
    assert_eq!(pc.read_byte(APOW_CTR), 0xFE, "contador devuelto al inicio");
    assert!(pc.run_until_ready(10).is_some(), "sigue en el prompt");

    // Y al volver a activarlo, la ROM sigue su cuenta normal.
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

    // Carga en $0100, 1 byte: `NOP` ($38).
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
    assert_eq!(pc.cpu().p(), 0x0101, "ejecuta el programa cargado, sin reset");
}

#[test]
fn new_machine_is_powered_on() {
    let pc = Pc1500::new_deterministic();
    assert!(pc.is_powered_on());
    assert!(pc.auto_power_off());
}
