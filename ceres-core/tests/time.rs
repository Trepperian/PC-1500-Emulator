//! Tests de `TIME` (lectura y `TIME=`) y de la base de tiempos de `WAIT`
//! contra la ROM real, con el reloj `µPD1990AC` en
//! `RtcTimeBase::Emulated`: una hora fija que avanza con los ciclos
//! emulados, así que todo es determinista (ver `pd1990ac.rs`).
//!
//! `TIME` devuelve un número `MMDDHH.MMSS` (mes, día, hora . minutos,
//! segundos) que la ROM muestra normalizado: sin ceros a la izquierda ni
//! ceros finales en la parte decimal (ver [`rom_time_text`]).

use core::iter;
use std::fs;

use ceres_core::{Key, Pc1500, RtcDateTime, RtcTimeBase, harness::KeyStroke};

/// Máximo de frames que se espera a que un comando termine.
const COMMAND_MAX_FRAMES: usize = 2000;

/// Frames emulados por segundo: `FRAME_DURATION` son 15000 ciclos a
/// 1.3 MHz, ~11.54 ms.
const FRAMES_PER_SECOND: usize = 87;

/// 28 de febrero, 23:59:50, miércoles: a 10 s del cambio de mes.
const FEB_28: RtcDateTime = RtcDateTime {
    month: 2,
    day: 28,
    hour: 23,
    minute: 59,
    second: 50,
    weekday: 3,
};

/// 31 de diciembre, 23:59:58, sábado: a 2 s del cambio de año.
const DEC_31: RtcDateTime = RtcDateTime {
    month: 12,
    day: 31,
    hour: 23,
    minute: 59,
    second: 58,
    weekday: 6,
};

/// Máquina con el reloj en `start`, arrancada hasta el prompt `>`.
fn booted_at(start: RtcDateTime) -> Pc1500 {
    let mut pc = Pc1500::new();
    pc.set_rtc_time_base(RtcTimeBase::Emulated(start));
    assert!(pc.boot().is_some(), "la ROM no lleg\u{f3} al prompt");
    pc
}

/// `Pc1500::enter_line`, fallando el test con el LCD si algo va mal.
fn enter(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

/// Pulsa `CL` (borra el resultado o el error) y espera al prompt.
fn clear(pc: &mut Pc1500) {
    pc.tap(KeyStroke::plain(Key::Cl));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "CL");
}

/// El texto con el que la ROM muestra el resultado de `TIME` para la
/// hora `t`: el número `MMDDHH.MMSS` normalizado (`BCMD_TIME`, $DE82, le
/// pone exponente 5 y lo normaliza con `VEJ (E8)`), así que sin ceros a
/// la izquierda, sin ceros finales en los decimales y sin punto si los
/// decimales son todos 0.
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

/// Teclea `TIME` + ENTER y devuelve lo que muestra el LCD, junto con la
/// hora del chip justo antes de pulsar ENTER y justo después de que
/// termine el comando: lo leído tiene que ser una de las dos (el comando
/// tarda mucho menos de 1 s).
fn read_time(pc: &mut Pc1500) -> (String, RtcDateTime, RtcDateTime) {
    if let Err(c) = pc.type_text("TIME") {
        panic!("sin tecla: {c:?}");
    }
    let before = pc.rtc_now();
    pc.tap(KeyStroke::plain(Key::Enter));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "TIME no termin\u{f3}");
    let after = pc.rtc_now();
    (pc.lcd_text().trim().to_owned(), before, after)
}

/// [`read_time`] comprobando que el LCD muestra la hora del chip.
fn read_time_checked(pc: &mut Pc1500) -> String {
    let (shown, before, after) = read_time(pc);
    assert!(
        shown == rom_time_text(before) || shown == rom_time_text(after),
        "TIME muestra [{shown}]; el chip estaba entre {before} y {after}"
    );
    shown
}

#[test]
fn time_reads_the_running_clock() {
    let mut pc = booted_at(FEB_28);
    // El arranque y el tecleo de `TIME` duran menos de 1 s emulado:
    // 28 feb 23:59:50, mostrado sin el 0 final.
    assert_eq!(read_time_checked(&mut pc), "22823.595", "TIME");
    // El número tal como queda en ARX ($7A00-$7A06) al acabar
    // `BCMD_TIME`: exponente 4 (2.2823595 x 10^4), signo +, mantisa BCD.
    assert_eq!(
        pc.read_bytes(0x7A00, 7),
        [0x04, 0x00, 0x22, 0x82, 0x35, 0x95, 0x00],
        "ARX tras BCMD_TIME"
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
        "TIME= escribe la hora (con el d\u{ed}a de la semana a 0)"
    );
    clear(&mut pc);
    // 435 frames = 5.02 s emulados, más ~0.5 s de tecleo de `TIME`.
    pc.run_frames(435);
    assert_eq!(read_time_checked(&mut pc), "101912.305", "5 s despu\u{e9}s");
}

#[test]
fn feb_28_rolls_over_to_march_1() {
    let mut pc = booted_at(FEB_28);
    // Sin bisiestos: el 28 de febrero va seguido del 1 de marzo.
    let reached = (0..20 * FRAMES_PER_SECOND).any(|_| {
        pc.run_frames(1);
        pc.rtc_now().month == 3
    });
    assert!(reached, "no lleg\u{f3} a marzo");
    let now = pc.rtc_now();
    assert_eq!((now.day, now.weekday), (1, 4), "jueves 1 de marzo");
    let shown = read_time_checked(&mut pc);
    assert!(shown.starts_with("30100"), "1 de marzo, 00 h: [{shown}]");
}

#[test]
fn year_rollover() {
    let mut pc = booted_at(DEC_31);
    pc.run_frames(3 * FRAMES_PER_SECOND);
    let shown = read_time_checked(&mut pc);
    assert!(shown.starts_with("10100.000"), "1 de enero, 00:00:0x: [{shown}]");
    let now = pc.rtc_now();
    assert_eq!((now.month, now.day, now.weekday), (1, 1, 0), "domingo 1 de enero");
}

#[test]
fn invalid_month_is_error_23() {
    let mut pc = booted_at(FEB_28);
    enter(&mut pc, "TIME=131912.3045");
    assert_eq!(pc.lcd_text().trim(), "ERROR 23", "mes 13");
    assert_eq!(pc.rtc_now().month, 2, "el reloj no cambia");
    clear(&mut pc);

    // El mes 0 sí lo acepta la ROM (solo comprueba < $13).
    enter(&mut pc, "TIME=1012.0005");
    assert_eq!(pc.rtc_now().month, 0, "mes 0");
    clear(&mut pc);
    let shown = read_time_checked(&mut pc);
    assert!(shown.starts_with("1012.000"), "TIME tras TIME=1012.0005: [{shown}]");
}

#[test]
fn reset_keeps_the_clock() {
    let mut pc = booted_at(FEB_28);
    pc.set_auto_power_off(false);
    enter(&mut pc, "TIME=101912.3045");
    clear(&mut pc);
    pc.reset();
    // Un arranque en frío de verdad...
    assert!(pc.run_until_idle(600).is_some(), "arranque tras reset");
    assert_eq!(pc.lcd_text().trim(), "NEW0? :CHECK", "arranque en fr\u{ed}o");
    // ...que conserva el reloj y la opción de auto-apagado.
    assert!(!pc.auto_power_off(), "auto-apagado");
    clear(&mut pc);
    let shown = read_time_checked(&mut pc);
    assert!(shown.starts_with("101912.30"), "la hora puesta sigue: [{shown}]");
}

/// `TIME_DELAY` ($E88C), la espera de `WAIT`, `PAUSE` y `BEEP`: pone `TP`
/// a 64 Hz y cuenta flancos de subida de PB5, 15.625 ms cada uno. Con
/// `U = 63` son 64 periodos: 1 s.
#[test]
fn time_delay_lasts_one_second_per_64_units() {
    // LDI UH,0 ; LDI UL,63 ; SJP $E88C ; HLT
    let code = [0x68_u8, 0x00, 0x6A, 63, 0xBE, 0xE8, 0x8C, 0xFD, 0xB1];
    let mut file = vec![0x00, 0x40, 9, 0x00];
    file.extend_from_slice(&code);
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("time_delay.lh5");
    fs::write(&path, file).expect("escribir .lh5");

    let mut pc = booted_at(FEB_28);
    pc.load_lh5_file(&path).expect("cargar .lh5");
    let start = pc.cpu().get_ticks();
    assert!(
        pc.run_until(3 * FRAMES_PER_SECOND, |pc| pc.cpu().is_halted()).is_some(),
        "TIME_DELAY no termin\u{f3}"
    );
    // Se mide a frames enteros: hasta 15000 ciclos de más.
    let cycles = pc.cpu().get_ticks() - start;
    assert!(
        (1_274_000..=1_326_000).contains(&cycles),
        "TIME_DELAY(63) = {cycles} ciclos, no ~1.3 M (1 s)"
    );
}

/// `WAIT 64`: cada `PRINT` se queda en pantalla 1 s (64/64) y el programa
/// sigue solo.
#[test]
fn wait_64_shows_each_print_for_one_second() {
    let mut pc = booted_at(FEB_28);
    for line in iter::once("NEW").chain(["10 WAIT 64:PRINT 1:PRINT 2"]) {
        enter(&mut pc, line);
    }
    pc.tap(KeyStroke::plain(Key::Mode));
    assert!(pc.run_until_ready(200).is_some(), "MODE");
    if let Err(c) = pc.type_text("RUN\n") {
        panic!("sin tecla: {c:?}");
    }
    let shows = |text: &'static str| move |m: &Pc1500| m.lcd_text().trim() == text;
    pc.run_until(5 * FRAMES_PER_SECOND, shows("1")).expect("PRINT 1");
    let frames = pc.run_until(5 * FRAMES_PER_SECOND, shows("2")).expect("PRINT 2");
    assert!(
        (FRAMES_PER_SECOND - 6..=FRAMES_PER_SECOND + 6).contains(&frames),
        "PRINT 1 se vio {frames} frames, no ~{FRAMES_PER_SECOND} (1 s)"
    );
}
