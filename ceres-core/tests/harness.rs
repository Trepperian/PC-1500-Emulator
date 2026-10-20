//! Tests de integración del arnés headless (`ceres_core::harness`):
//! arrancan la ROM real, teclean como lo haría una persona y comprueban
//! lo que aparece en el LCD. Sirven a la vez de ejemplo de uso del
//! arnés para los tests de funcionalidades futuras.

use ceres_core::{
    Key, Pc1500,
    harness::{
        BOOT_MAX_FRAMES, EnterLineError, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES, KeyStroke,
        LCD_CHARS, key_for_char, text_to_keystrokes,
    },
};

/// Bit del indicador `PRO` en `$764F` (ver `Pc1500::lcd_symbol_bytes`).
const SYMBOL_PRO: u8 = 0x20;
/// Bit del indicador `RUN` en `$764F`.
const SYMBOL_RUN: u8 = 0x40;

/// Máximo de frames que se espera a que un comando termine.
const COMMAND_MAX_FRAMES: usize = 2000;

/// `BASPRG_ST_H` ($7865): puntero big-endian al inicio del programa.
const BASPRG_ST: u32 = 0x7865;
/// `BASPRG_END_H` ($7867): puntero big-endian al `$FF` que cierra el
/// programa.
const BASPRG_END: u32 = 0x7867;

fn booted() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "la ROM no llegó al prompt");
    pc
}

/// Teclea una línea con `Pc1500::enter_line` (espera a que la ROM la
/// procese y quede lista para la siguiente tecla) y falla el test con
/// el LCD en el mensaje si algo va mal.
fn enter_line(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

#[test]
fn cold_boot_shows_new0_check_message() {
    let mut pc = Pc1500::new_deterministic();
    let frames = pc.run_until_idle(BOOT_MAX_FRAMES);
    assert!(frames.is_some(), "la ROM no llegó a WAIT_4_KB");
    assert!(pc.is_waiting_for_key(), "debería estar en WAIT_4_KB");
    assert_eq!(pc.lcd_text().trim_end(), "NEW0? :CHECK", "mensaje de arranque en frío");
}

#[test]
fn boot_reaches_prompt_in_pro_mode() {
    let pc = booted();
    let text = pc.lcd_text();
    assert_eq!(text.chars().count(), LCD_CHARS, "el LCD tiene 26 celdas");
    assert_eq!(text.trim_end(), ">", "prompt tras CL");
    let [_, symbols] = pc.lcd_symbol_bytes();
    assert_ne!(symbols & SYMBOL_PRO, 0, "el arranque deja el modo PRO");
    assert_eq!(symbols & SYMBOL_RUN, 0, "RUN apagado en modo PRO");
}

#[test]
fn print_expression_shows_result() {
    let mut pc = booted();
    enter_line(&mut pc, "PRINT 1+2\n");
    // La PC-1500 alinea los resultados numéricos a la derecha.
    assert_eq!(pc.lcd_text().trim(), "3", "LCD = [{}]", pc.lcd_text());
}

#[test]
fn program_line_is_stored_and_listed() {
    let mut pc = booted();
    enter_line(&mut pc, "NEW\n");
    enter_line(&mut pc, "10 PRINT 5\n");
    enter_line(&mut pc, "LIST\n");
    assert_eq!(pc.lcd_text().trim_end(), "10:PRINT 5", "LCD = [{}]", pc.lcd_text());

    // La línea tokenizada en RAM: [nº de línea BE][longitud][cuerpo + $0D],
    // con PRINT = $F0 $97.
    let start = u32::from(pc.read_word_be(BASPRG_ST));
    assert_eq!(
        pc.read_bytes(start, 7),
        [0x00, 0x0A, 0x04, 0xF0, 0x97, b'5', 0x0D],
        "línea 10 tokenizada"
    );
}

#[test]
fn program_runs_in_run_mode() {
    let mut pc = booted();
    enter_line(&mut pc, "NEW\n");
    enter_line(&mut pc, "10 PRINT 5\n");
    enter_line(&mut pc, "20 PRINT \"HI\",3*4\n");
    pc.tap(KeyStroke::plain(Key::Mode));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "MODE");
    let [_, symbols] = pc.lcd_symbol_bytes();
    assert_ne!(symbols & SYMBOL_RUN, 0, "MODE pasa a RUN");

    enter_line(&mut pc, "RUN\n");
    assert_eq!(pc.lcd_text().trim(), "5", "primer PRINT");
    // PRINT en un programa espera a ENTER para continuar.
    enter_line(&mut pc, "\n");
    assert_eq!(pc.lcd_text().trim_end(), "HI                      12", "segundo PRINT");
    enter_line(&mut pc, "\n");
    assert_eq!(pc.lcd_text().trim_end(), ">", "fin del programa");
}

#[test]
fn every_supported_char_is_echoed() {
    // Todos los caracteres que `key_for_char` sabe teclear (salvo ENTER),
    // en trozos de como mucho 25 caracteres (con 26 el cursor ya no
    // cabe y el LCD se desplaza a la izquierda), borrando con CL entre
    // trozos.
    let chunks = [
        "ABCDEFGHIJKLM",
        "NOPQRSTUVWXYZ",
        "0123456789 .-+*/=()",
        "!\"#$%&,<>?:;@^[]",
        "abcdefghijklm",
        "nopqrstuvwxyz",
    ];
    let mut pc = booted();
    for chunk in chunks {
        pc.type_text(chunk).expect("todos estos caracteres tienen tecla");
        // `lcd_cells` y no `lcd_text`, para que un `?` mal reconocido no
        // pase por un `?` auténtico.
        let cells = pc.lcd_cells();
        let shown: String = cells
            .iter()
            .take(chunk.chars().count())
            .map(|c| c.unwrap_or('\u{FFFD}'))
            .collect();
        assert_eq!(shown, chunk, "eco en el LCD");
        pc.tap(KeyStroke::plain(Key::Cl));
    }
}

#[test]
fn repeated_keys_are_not_lost() {
    let mut pc = booted();
    pc.type_text("AABB1122  ..").expect("caracteres soportados");
    assert!(pc.lcd_text().starts_with("AABB1122  .."), "LCD = [{}]", pc.lcd_text());
}

#[test]
fn default_timing_keeps_a_safety_margin() {
    // Medido: hace falta suelta >= 2 frames y pulsación + suelta >= 6.
    // Si con un frame menos en cada lado ya no funciona, la ROM o el
    // emulador han cambiado y hay que volver a medir los tiempos.
    let text = "QWERTYUIOPASDFGHJKL1234";
    let mut pc = booted();
    pc.type_text_with_timing(text, KEY_HOLD_FRAMES - 1, KEY_RELEASE_FRAMES - 1)
        .expect("caracteres soportados");
    assert!(pc.lcd_text().starts_with(text), "LCD = [{}]", pc.lcd_text());
}

#[test]
fn unsupported_chars_are_reported() {
    assert_eq!(key_for_char('_'), None, "'_' no tiene tecla");
    assert_eq!(key_for_char('A'), Some(KeyStroke::plain(Key::A)), "A");
    assert_eq!(key_for_char('a'), Some(KeyStroke::shifted(Key::A)), "a");
    assert_eq!(key_for_char(':'), Some(KeyStroke::shifted(Key::Asterisk)), ":");
    assert_eq!(text_to_keystrokes("10 A=1\r\n").map(|k| k.len()), Ok(7), "CRLF");
    assert_eq!(text_to_keystrokes("PRINT '1'"), Err('\''), "comilla simple");

    // Un `Err` no teclea nada.
    let mut pc = booted();
    assert_eq!(pc.type_text("AB{"), Err('{'), "llave");
    pc.run_frames(KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES);
    assert_eq!(pc.lcd_text().trim_end(), ">", "LCD intacto");
}

/// Las líneas del test de tecleo de varias líneas: `(número, cuerpo)`.
/// Los cuerpos no tienen palabras clave ni espacios, así que la ROM los
/// guarda tal cual (sin tokens) y el resultado esperado se puede
/// construir byte a byte.
///
/// La línea larga es la línea 170 de `jackpot` (41 caracteres), con la
/// que se descubrió el problema: con varias líneas cortas delante, la
/// ROM tarda justo lo que hace falta en procesarla como para volver a
/// `WAIT_4_KB` con el bit 0 de `CURS_CTRL` todavía a 1 (ver
/// `Pc1500::is_ready_for_key`). Con el antiguo `type_text` +
/// `run_until_idle` esta lista exacta volvía con ese bit a 1 tras las
/// líneas 1000, 1060 y 1090, y perdía el primer `1` de las siguientes
/// (1010, 1070 y 1100 llegaban como `010`, `070` y `100`, machacando o
/// creando otras líneas). Medido al escribir este test; la
/// condición depende del momento exacto de los despertares del
/// temporizador, así que el test solo exige que `enter_lines` funcione,
/// no que el método antiguo falle.
fn multiline_program() -> Vec<(u16, String)> {
    let short = (1..=9).map(|i| (i * 10, format!("G={i}")));
    let mixed = (0..=10).map(|i| {
        let body = if i % 3 == 0 {
            "SI=112:DO=105:RE=93:MI=82:SO=68:TP=80".to_owned()
        } else {
            format!("A={i}:B={}:C={}", i * 11, i * 111)
        };
        (1000 + i * 10, body)
    });
    short.chain(mixed).collect()
}

#[test]
fn many_program_lines_are_entered_without_losing_keys() {
    let lines = multiline_program();
    assert_eq!(lines.len(), 20, "20 l\u{ed}neas");
    let text = lines
        .iter()
        .map(|(n, body)| format!("{n} {body}\n"))
        .collect::<Vec<_>>()
        .concat();

    let mut pc = booted();
    enter_line(&mut pc, "NEW");
    if let Err(err) = pc.enter_lines(&text) {
        panic!("{err}; LCD = [{}]", pc.lcd_text());
    }
    assert!(!pc.lcd_text().contains("ERROR"), "LCD = [{}]", pc.lcd_text());

    // Área de programa esperada: por línea [nº BE][longitud][cuerpo][$0D]
    // (la longitud cuenta el $0D), y un $FF final en BASPRG_END.
    let mut expected = Vec::new();
    for (number, body) in &lines {
        expected.extend(number.to_be_bytes());
        expected.push(u8::try_from(body.len() + 1).expect("l\u{ed}nea corta"));
        expected.extend(body.bytes());
        expected.push(0x0D);
    }
    expected.push(0xFF);

    let start = u32::from(pc.read_word_be(BASPRG_ST));
    let end = u32::from(pc.read_word_be(BASPRG_END));
    assert_eq!(
        (end + 1).checked_sub(start).map(|len| len as usize),
        Some(expected.len()),
        "longitud del programa"
    );
    assert_eq!(pc.read_bytes(start, expected.len()), expected, "bytes del programa");
}

#[test]
fn key_pressed_before_release_is_seen_is_swallowed() {
    // El mecanismo que hace necesario `is_ready_for_key`: mientras la ROM
    // no haya visto el teclado vacío (bit 0 de CURS_CTRL a 1), cualquier
    // tecla se toma como la anterior mantenida y se descarta, aunque la
    // CPU ya esté dormida en WAIT_4_KB.
    let mut pc = booted();
    assert!(pc.is_ready_for_key(), "lista tras boot()");

    pc.press(Key::A);
    assert!(pc.run_until_idle(COMMAND_MAX_FRAMES).is_some(), "eco de A");
    assert!(pc.is_waiting_for_key(), "en WAIT_4_KB con A mantenida");
    assert!(!pc.is_ready_for_key(), "con A mantenida no est\u{e1} lista");

    // Soltar A y pulsar B en el mismo instante: la ROM nunca ve el
    // teclado vacío, así que B se pierde.
    pc.release(Key::A);
    pc.tap_key_for(Key::B, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES);
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "lista tras soltar");
    assert!(pc.is_ready_for_key(), "lista");

    // Una vez lista, la siguiente tecla sí entra.
    pc.tap(KeyStroke::plain(Key::C));
    assert!(pc.lcd_text().starts_with("AC"), "B perdida, C aceptada; LCD = [{}]", pc.lcd_text());
}

#[test]
fn enter_line_rejects_bad_input_without_typing() {
    let mut pc = booted();
    assert_eq!(pc.enter_line("10 A=1\n20 B=2"), Err(EnterLineError::MultipleLines), "dos l\u{ed}neas");
    assert_eq!(pc.enter_line("PRINT '1'"), Err(EnterLineError::Unmappable('\'')), "comilla");
    assert_eq!(
        pc.enter_lines("10 A=1\n20 B={\n"),
        Err(EnterLineError::Unmappable('{')),
        "error en la segunda l\u{ed}nea"
    );
    pc.run_frames(KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES);
    assert_eq!(pc.lcd_text().trim_end(), ">", "no se tecle\u{f3} nada");
    // `\r\n` y el `\n` final son opcionales.
    enter_line(&mut pc, "PRINT 6*7\r\n");
    assert_eq!(pc.lcd_text().trim(), "42", "CRLF");
}

#[test]
fn colon_separates_statements_only_inside_programs() {
    // Comportamiento auténtico de la ROM, no un fallo del emulador: la
    // rutina de "fin de sentencia" CHECK_AT_END ($DCC5, VEJ C8) solo
    // acepta ':' como separador si CHECK_CALC_MODE ($DF3B, VEJ D8) dice
    // que hay un programa en marcha (CURR_LINE $789C/$789D != 0). En
    // modo directo (CURR_LINE = 0) salta directamente a comparar con
    // $0D ($DCC7 BZS $DCCD), así que ':' cae en la rama de error.
    let mut pc = booted();
    enter_line(&mut pc, "NEW");
    enter_line(&mut pc, "PRINT 1:PRINT 2");
    assert_eq!(pc.lcd_text().trim(), "ERROR 1", "':' en modo directo");
    // El mensaje de error se queda hasta pulsar CL.
    pc.tap(KeyStroke::plain(Key::Cl));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "CL");
    assert_eq!(pc.lcd_text().trim_end(), ">", "CL borra el error");

    // La misma línea guardada como línea de programa funciona.
    enter_line(&mut pc, "10 PRINT 1:PRINT 2");
    pc.tap(KeyStroke::plain(Key::Mode));
    assert!(pc.run_until_ready(COMMAND_MAX_FRAMES).is_some(), "MODE");
    enter_line(&mut pc, "RUN");
    assert_eq!(pc.lcd_text().trim(), "1", "primera sentencia");
    enter_line(&mut pc, "");
    assert_eq!(pc.lcd_text().trim(), "2", "segunda sentencia tras ':'");
    enter_line(&mut pc, "");
    assert_eq!(pc.lcd_text().trim_end(), ">", "fin del programa");
}
