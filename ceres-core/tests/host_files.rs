//! Tests de `CLOAD`/`CSAVE`/`MERGE`/`CHAIN` desde la propia PC-1500
//! emulada (`ceres_core::host_io`): la tabla virtual de $9800 vista por
//! la ROM real (tokenizar, `LIST`, modos), los formatos de archivo que
//! escribe `CSAVE` (con y sin el sufijo `a` de la CE-158), las cargas
//! (de los tres formatos, con `MERGE`, `CHAIN` y `CLOAD?`) y los errores.
//!
//! Cada test usa su propia carpeta temporal como carpeta de archivos
//! ([`Pc1500::set_host_dir`]); sin ella, el núcleo tiene la E/S de
//! archivos desactivada (`ERROR 27`).

use std::{
    fs,
    path::{Path, PathBuf},
};

use ceres_core::{
    HostCommand, HostEvent, Key, Pc1500,
    basic::{
        AsciiOptions, BASPRG_EDT, BASPRG_END, BASPRG_ST, BasicFormat, Program, SaveFormat,
        VAR_START, decode_program,
    },
    harness::KeyStroke,
    host_io::{HOST_TABLE_BASE, TOKEN_CLOAD, TOKEN_CSAVE},
};
use tempfile::TempDir;

const MOLE_BAS: &str = include_str!("data/mole.bas");
const JACKPOT_BAS: &str = include_str!("data/jackpot.bas");
const BATHYSCAPH_BAS: &str = include_str!("data/bathyscaph.bas");
const MOLE_BIN: &[u8] = include_bytes!("data/mole.bin");
const BATHYSCAPH_BIN: &[u8] = include_bytes!("data/bathyscaph.bin");

/// `ERL` ($789B): el código del último error.
const ERL: u32 = 0x789B;

/// La cabecera de `BATHY.BAS` de Jeff Birt (`CSAVE` en una CE-158X).
const JEFF_BIRT_BATHY_HEADER: [u8; 27] = [
    0x01, b'@', b'C', b'O', b'M', b'B', b'A', b'T', b'H', b'Y', b'.', b'B', b'A', b'S', 0, 0, 0, 0,
    0, 0, 0, 0x00, 0xC5, 0x03, 0x89, 0x04, 0x4F,
];

// --- Ayudas -------------------------------------------------------------------

fn word(pc: &Pc1500, addr: u16) -> u16 {
    pc.read_word_be(u32::from(addr))
}

fn byte(pc: &Pc1500, addr: u16) -> u8 {
    pc.read_byte(u32::from(addr))
}

fn lcd(pc: &Pc1500) -> String {
    pc.lcd_text().trim().to_owned()
}

fn tempdir() -> TempDir {
    tempfile::tempdir().unwrap_or_else(|e| panic!("carpeta temporal: {e}"))
}

/// Máquina arrancada, en PRO y sin carpeta, con `mole` como programa:
/// cargado aquí para no depender del programa de arranque del emulador.
fn booted_without_dir() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "la ROM no lleg\u{f3} al prompt");
    let (mole, _) = Program::from_tokenized(MOLE_BIN).unwrap_or_else(|e| panic!("{e}"));
    pc.load_basic_program(&mole).unwrap_or_else(|e| panic!("{e}"));
    pc
}

/// Máquina arrancada con una carpeta temporal propia.
fn booted() -> (Pc1500, TempDir) {
    let mut pc = booted_without_dir();
    let dir = tempdir();
    pc.set_host_dir(Some(dir.path().to_path_buf()));
    (pc, dir)
}

/// Teclea una línea y espera a que la ROM quede lista; si el LCD tenía
/// un `ERROR`, primero lo borra con `CL`.
fn enter(pc: &mut Pc1500, line: &str) {
    if pc.lcd_text().contains("ERROR") {
        tap(pc, Key::Cl);
    }
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

fn tap(pc: &mut Pc1500, key: Key) {
    pc.tap(KeyStroke::plain(key));
    assert!(pc.run_until_ready(3000).is_some(), "la ROM no qued\u{f3} lista tras {key:?}");
}

/// Pasa a modo RUN (desde PRO).
fn run_mode(pc: &mut Pc1500) {
    tap(pc, Key::Mode);
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    path
}

fn read_file(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Los bytes del programa en RAM, desde `BASPRG_ST`.
fn program_at_start(pc: &Pc1500, len: usize) -> Vec<u8> {
    pc.read_bytes(u32::from(word(pc, BASPRG_ST)), len)
}

/// Comprueba que la RAM tiene exactamente `expected` como programa, con
/// su `$FF` y los punteros como los deja `NEW`.
fn assert_loaded(pc: &Pc1500, expected: &[u8], what: &str) {
    let start = word(pc, BASPRG_ST);
    let len = u16::try_from(expected.len()).unwrap_or(u16::MAX);
    assert_eq!(program_at_start(pc, expected.len()), expected, "{what}: bytes");
    assert_eq!(byte(pc, start + len), 0xFF, "{what}: $FF final");
    assert_eq!(word(pc, BASPRG_END), start + len, "{what}: BASPRG_END");
    assert_eq!(word(pc, BASPRG_EDT), start, "{what}: BASPRG_EDT");
}

fn ascii_tokenized(pc: &Pc1500, text: &str) -> Vec<u8> {
    Program::from_ascii(text, &pc.basic_keywords(), &AsciiOptions::default())
        .unwrap_or_else(|e| panic!("{e}"))
        .0
        .to_tokenized()
}

fn mole_lines() -> usize {
    Program::from_tokenized(MOLE_BIN).map(|(p, _)| p.lines().len()).unwrap_or_default()
}

fn last_event(pc: &mut Pc1500) -> HostEvent {
    pc.take_host_events()
        .pop()
        .unwrap_or_else(|| panic!("sin eventos; LCD = [{}]", pc.lcd_text()))
}

// --- La tabla vista por la ROM ----------------------------------------------

#[test]
fn table_tokenizes_and_lists() {
    let mut pc = booted_without_dir();
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 CSAVE \"T1\":CLOAD:MERGE \"X\":CHAIN \"Y\",20");
    assert_eq!(
        program_at_start(&pc, 28),
        [
            0x00, 0x0A, 0x19, 0xF0, 0x95, 0x22, 0x54, 0x31, 0x22, 0x3A, 0xF0, 0x89, 0x3A, 0xF0,
            0x8F, 0x22, 0x58, 0x22, 0x3A, 0xF0, 0xB2, 0x22, 0x59, 0x22, 0x2C, 0x32, 0x30, 0x0D
        ],
        "tokenizado por la ROM"
    );
    enter(&mut pc, "LIST");
    assert_eq!(lcd(&pc), "10:CSAVE \"T1\":CLOAD :MERGE", "LIST");
    // Abreviaturas con '.'.
    enter(&mut pc, "20 CSA.\"A\":CLOA.");
    enter(&mut pc, "LIST 20");
    assert_eq!(lcd(&pc), "20:CSAVE \"A\":CLOAD", "abreviaturas");
}

#[test]
fn keywords_include_the_host_table() {
    let pc = booted_without_dir();
    let keywords = pc.basic_keywords();
    let bases: Vec<(u16, bool)> = keywords.tables().iter().map(|t| (t.base(), t.pv())).collect();
    assert_eq!(bases, [(0xC000, false), (HOST_TABLE_BASE, false)], "principal y $9800 con PV=0");
    assert_eq!(
        keywords.tokenize_body(b"CLOAD\"X\":CSAVE").unwrap_or_default(),
        [0xF0, 0x89, b'"', b'X', b'"', b':', 0xF0, 0x95],
        "el tokenizador del host las conoce, como la ROM"
    );
    assert_eq!(TOKEN_CSAVE, 0xF095, "token de CSAVE");
}

#[test]
fn boot_still_reaches_the_prompt_quickly() {
    // La tabla añade una llamada de inicialización (`RTN`) al arranque:
    // medido, 36 frames hasta `WAIT_4_KB` (35 sin ella) y 46 con el `CL`.
    let mut pc = Pc1500::new_deterministic();
    let frames = pc.boot().unwrap_or_else(|| panic!("sin prompt"));
    assert!(frames <= 60, "{frames} frames");
    assert_eq!(lcd(&pc), ">", "prompt");
}

#[test]
fn host_disabled_gives_error_27() {
    let mut pc = booted_without_dir();
    assert_eq!(pc.host_dir(), None, "desactivada por defecto en el n\u{fa}cleo");
    enter(&mut pc, "CSAVE \"T1\"");
    assert_eq!(lcd(&pc), "ERROR 27", "CSAVE sin carpeta");
    enter(&mut pc, "CLOAD \"T1\"");
    assert_eq!(lcd(&pc), "ERROR 27", "CLOAD sin carpeta");
    assert!(pc.take_host_events().is_empty(), "sin eventos");
    assert_eq!(pc.basic_program_bytes(), MOLE_BIN, "programa intacto");
}

// --- CSAVE ------------------------------------------------------------------

#[test]
fn csave_bin_is_exact() {
    let (mut pc, dir) = booted();
    enter(&mut pc, "CSAVE \"T2.BIN\"");
    assert_eq!(lcd(&pc), ">", "sin error");
    let path = dir.path().join("T2.BIN");
    assert_eq!(read_file(&path), MOLE_BIN, "bytes de la RAM tal cual");
    assert_eq!(
        last_event(&mut pc),
        HostEvent::Saved { path, format: SaveFormat::Tokenized, bytes: MOLE_BIN.len(), lines: mole_lines() },
        "evento"
    );
    assert!(pc.take_host_events().is_empty(), "take_host_events vac\u{ed}a la cola");
}

#[test]
fn csave_default_is_ce158_like_the_real_one() {
    let (mut pc, dir) = booted();
    enter(&mut pc, "CSAVE \"T1\"");
    assert_eq!(lcd(&pc), ">", "sin error");
    let file = read_file(&dir.path().join("T1.BAS"));
    assert_eq!(file.len(), 27 + MOLE_BIN.len(), "cabecera + programa, sin $FF");
    let (program, header, warnings) = Program::from_ce158(&file).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(program.to_tokenized(), MOLE_BIN, "programa");
    assert_eq!(header.name, b"T1", "nombre de la cabecera");
    assert_eq!(header.start, 0x00C5, "inicio");
    assert_eq!(usize::from(header.size_minus_one), MOLE_BIN.len() - 1, "longitud - 1");
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn csave_ascii_suffix_and_extensions() {
    let (mut pc, dir) = booted();
    let keywords = pc.basic_keywords();
    let decode = |path: &Path| {
        let bytes = read_file(path);
        decode_program(&bytes, BasicFormat::Ascii, &keywords, &AsciiOptions::default())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .program
            .to_tokenized()
    };

    // CSAVEa: texto de la CE-158X, CR.
    enter(&mut pc, "CSAVEa \"T1\"");
    assert_eq!(lcd(&pc), ">", "CSAVEa");
    let asc = dir.path().join("T1.ASC");
    let text = String::from_utf8(read_file(&asc)).unwrap_or_default();
    assert!(text.contains(" 5000 \"MOLE\"\r"), "l\u{ed}nea 5000 con CR: {text:?}");
    assert!(!text.contains('\n'), "sin LF");
    assert_eq!(decode(&asc), MOLE_BIN, "ida y vuelta");

    // .txt manda: texto CRLF, con a o sin ella.
    enter(&mut pc, "CSAVE \"T3.TXT\"");
    let txt = dir.path().join("T3.TXT");
    assert!(String::from_utf8(read_file(&txt)).unwrap_or_default().contains("5000 \"MOLE\"\r\n"), "CRLF");
    assert_eq!(decode(&txt), MOLE_BIN, ".txt");

    // .bas con a: texto CRLF.
    enter(&mut pc, "CSAVEa \"T4.BAS\"");
    let bas = dir.path().join("T4.BAS");
    assert!(!read_file(&bas).starts_with(&[0x01, b'@']), "texto, no cabecera");
    assert_eq!(decode(&bas), MOLE_BIN, ".bas con a");

    // Cualquier otra extensión sin a: la regla de la CE-158 (cabecera).
    enter(&mut pc, "CSAVE \"T5.ASC\"");
    assert!(read_file(&dir.path().join("T5.ASC")).starts_with(&[0x01, b'@', b'C', b'O', b'M']), ".asc sin a");
    assert_eq!(lcd(&pc), ">", "sin errores");
}

#[test]
fn csave_overwrites_existing_file_with_other_case() {
    let (mut pc, dir) = booted();
    let old = write_file(dir.path(), "t1.bin", b"viejo");
    enter(&mut pc, "CSAVE \"T1.BIN\"");
    assert_eq!(read_file(&old), MOLE_BIN, "sobrescrito el que ya hab\u{ed}a");
    let count = fs::read_dir(dir.path()).map(Iterator::count).unwrap_or_default();
    assert_eq!(count, 1, "sin duplicados ni temporales");
}

#[test]
fn csave_string_variable_in_program() {
    let (mut pc, dir) = booted();
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 A$=\"ZZ\":CSAVE A$:PRINT 7");
    run_mode(&mut pc);
    enter(&mut pc, "RUN");
    assert_eq!(lcd(&pc), "7", "la l\u{ed}nea sigue tras CSAVE");
    let saved = read_file(&dir.path().join("ZZ.BAS"));
    let (program, _, _) = Program::from_ce158(&saved).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(program.lines().len(), 1, "el programa de una l\u{ed}nea");
}

// --- CLOAD ------------------------------------------------------------------

#[test]
fn cload_round_trips_every_csave_format() {
    let (mut pc, dir) = booted();
    enter(&mut pc, "CSAVE \"T2.BIN\"");
    enter(&mut pc, "CSAVE \"T1\"");
    enter(&mut pc, "CSAVEa \"T1\"");
    assert_eq!(pc.take_host_events().len(), 3, "tres archivos");
    for (name, format) in [
        ("T2.BIN", BasicFormat::Tokenized),
        ("T1", BasicFormat::Ce158),
        ("T1.ASC", BasicFormat::Ascii),
        ("t1.asc", BasicFormat::Ascii),
    ] {
        enter(&mut pc, "NEW");
        assert_eq!(byte(&pc, word(&pc, BASPRG_ST)), 0xFF, "{name}: vac\u{ed}o tras NEW");
        enter(&mut pc, &format!("CLOAD \"{name}\""));
        assert_eq!(lcd(&pc), ">", "{name}: sin error");
        assert_loaded(&pc, MOLE_BIN, name);
        match last_event(&mut pc) {
            HostEvent::Loaded { command, format: got, bytes, lines, .. } => {
                assert_eq!((command, got, bytes, lines), (HostCommand::Cload, format, MOLE_BIN.len(), mole_lines()), "{name}");
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    enter(&mut pc, "LIST");
    assert_eq!(lcd(&pc), "5000:\"MOLE\"", "LIST");
    tap(&mut pc, Key::Down);
    assert_eq!(lcd(&pc), "5010:\"B\":CLEAR", "flecha abajo");
    let _ = dir;
}

#[test]
fn cload_corpus_text_files() {
    let (mut pc, dir) = booted();
    write_file(dir.path(), "bathyscaph.bas", BATHYSCAPH_BAS.as_bytes());
    write_file(dir.path(), "Mole.Bas", MOLE_BAS.as_bytes());
    write_file(dir.path(), "JACKPOT.BAS", JACKPOT_BAS.as_bytes());

    enter(&mut pc, "CLOAD \"BATHYSCAPH\"");
    assert_eq!(lcd(&pc), ">", "bathyscaph");
    assert_loaded(&pc, BATHYSCAPH_BIN, "bathyscaph.bas = bathyscaph.bin");

    let jackpot = ascii_tokenized(&pc, JACKPOT_BAS);
    enter(&mut pc, "CLOAD \"JACKPOT\"");
    assert_loaded(&pc, &jackpot, "jackpot");

    // Comentarios ', CRLF y bytes que no son UTF-8 de la cabecera.
    let mole = ascii_tokenized(&pc, MOLE_BAS);
    enter(&mut pc, "CLOAD \"MOLE\"");
    assert_loaded(&pc, &mole, "mole");
    enter(&mut pc, "LIST");
    assert_eq!(lcd(&pc), "5000:\"MOLE\"", "LIST");
}

#[test]
fn cload_ce158_header_files() {
    let (mut pc, dir) = booted();
    let mut jeff = JEFF_BIRT_BATHY_HEADER.to_vec();
    jeff.extend_from_slice(BATHYSCAPH_BIN);
    write_file(dir.path(), "BATHY.BAS", &jeff);
    // La variante del compilador: longitud - 1 con el $FF, y el $FF.
    let mut compiler = JEFF_BIRT_BATHY_HEADER.to_vec();
    compiler[23..25].copy_from_slice(&[0x03, 0x8A]);
    compiler.extend_from_slice(BATHYSCAPH_BIN);
    compiler.push(0xFF);
    write_file(dir.path(), "COMP.DAT", &compiler);

    enter(&mut pc, "NEW");
    enter(&mut pc, "CLOAD \"BATHY\"");
    assert_eq!(lcd(&pc), ">", "BATHY.BAS");
    assert_loaded(&pc, BATHYSCAPH_BIN, "Jeff Birt");
    enter(&mut pc, "NEW");
    enter(&mut pc, "CLOAD \"COMP.DAT\"");
    assert_loaded(&pc, BATHYSCAPH_BIN, "compilador");
}

#[test]
fn cload_text_with_host_keywords() {
    let (mut pc, dir) = booted();
    write_file(dir.path(), "K.TXT", b"10 CSAVE \"A\":CLOAD\r\n");
    enter(&mut pc, "CLOAD \"K.TXT\"");
    assert_eq!(
        program_at_start(&pc, 12),
        [0x00, 0x0A, 0x09, 0xF0, 0x95, b'"', b'A', b'"', b':', 0xF0, 0x89, 0x0D],
        "tokenizado con la tabla de $9800"
    );
    assert_eq!(TOKEN_CLOAD, 0xF089, "token de CLOAD");
}

#[test]
fn cload_missing_file() {
    let (mut pc, _dir) = booted();
    enter(&mut pc, "CLOAD \"NOPE\"");
    assert_eq!(lcd(&pc), "ERROR 61", "no existe");
    assert_eq!(pc.read_byte(ERL), 61, "ERL");
    assert_eq!(pc.basic_program_bytes(), MOLE_BIN, "programa intacto");
    assert_eq!(last_event(&mut pc).error_code(), Some(61), "evento de error");
    // Y la máquina sigue funcionando.
    enter(&mut pc, "LIST");
    assert_eq!(lcd(&pc), "5000:\"MOLE\"", "LIST tras el error");
}

#[test]
fn cload_and_merge_are_direct_only_chain_is_program_only() {
    let (mut pc, dir) = booted();
    write_file(dir.path(), "Q.BIN", MOLE_BIN);
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 CLOAD \"Q\"");
    enter(&mut pc, "20 MERGE \"Q\"");
    run_mode(&mut pc);
    enter(&mut pc, "RUN");
    assert_eq!(lcd(&pc), "ERROR 1 IN 10", "CLOAD en un programa");
    enter(&mut pc, "GOTO 20");
    assert_eq!(lcd(&pc), "ERROR 1 IN 20", "MERGE en un programa");
    enter(&mut pc, "CHAIN \"Q\"");
    assert_eq!(lcd(&pc), "ERROR 1", "CHAIN en modo directo");
    assert!(pc.take_host_events().is_empty(), "la ROM los rechaza antes de llegar al host");
    // En modo RUN, CLOAD directo sí vale (máscara RUN | PRO).
    enter(&mut pc, "CLOAD \"Q\"");
    assert_eq!(lcd(&pc), ">", "CLOAD directo en RUN");
    assert_loaded(&pc, MOLE_BIN, "Q");
}

#[test]
fn cload_too_large() {
    let (mut pc, dir) = booted();
    // 2500 líneas `n PRINT 1`: 22500 bytes, más que toda la RAM.
    let big: Vec<u8> = (1..=2500_u16)
        .flat_map(|n| {
            let [hi, lo] = n.to_be_bytes();
            [hi, lo, 6, 0xF0, 0x97, b' ', b'1', b' ', 0x0D]
        })
        .collect();
    write_file(dir.path(), "BIG.BIN", &big);
    enter(&mut pc, "CLOAD \"BIG\"");
    assert_eq!(lcd(&pc), "ERROR 42", "no cabe");
    assert_eq!(pc.basic_program_bytes(), MOLE_BIN, "programa intacto");
}

#[test]
fn cload_bad_ascii_line() {
    let (mut pc, dir) = booted();
    write_file(dir.path(), "BAD.ASC", b"10 PRINT 1\rPRINT 2\r");
    enter(&mut pc, "CLOADa \"BAD\"");
    assert_eq!(lcd(&pc), "ERROR 67", "l\u{ed}nea sin n\u{fa}mero");
    assert_eq!(pc.basic_program_bytes(), MOLE_BIN, "programa intacto");
}

#[test]
fn cload_verify() {
    let (mut pc, dir) = booted();
    enter(&mut pc, "CSAVE \"T2.BIN\"");
    enter(&mut pc, "CLOAD? \"T2.BIN\"");
    assert_eq!(lcd(&pc), ">", "coincide");
    let path = dir.path().join("T2.BIN");
    assert_eq!(
        pc.take_host_events().pop(),
        Some(HostEvent::Verified { path, bytes: MOLE_BIN.len() }),
        "evento"
    );
    let mut changed = MOLE_BIN.to_vec();
    // La 'M' de "MOLE" (línea 5000, tras 13 88 07 22).
    changed[4] = b'N';
    write_file(dir.path(), "T2.BIN", &changed);
    enter(&mut pc, "CLOAD?\"T2.BIN\"");
    assert_eq!(lcd(&pc), "ERROR 43", "no coincide");
    assert_eq!(pc.basic_program_bytes(), MOLE_BIN, "CLOAD? no toca la memoria");
}

// --- MERGE y CHAIN ----------------------------------------------------------------

#[test]
fn merge_appends() {
    let (mut pc, dir) = booted();
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 PRINT 1");
    let start = word(&pc, BASPRG_ST);
    let old_end = word(&pc, BASPRG_END);
    write_file(dir.path(), "X.BIN", &[0x00, 0x14, 0x04, 0xF0, 0x97, b'2', 0x0D]);
    enter(&mut pc, "MERGE \"X\"");
    assert_eq!(lcd(&pc), ">", "sin error");
    assert_eq!(word(&pc, BASPRG_ST), start, "BASPRG_ST no cambia");
    assert_eq!(word(&pc, BASPRG_EDT), old_end, "BASPRG_EDT = el $FF de antes");
    assert_eq!(word(&pc, BASPRG_END), old_end + 7, "BASPRG_END");
    run_mode(&mut pc);
    enter(&mut pc, "RUN");
    assert_eq!(lcd(&pc), "1", "primera parte");
    enter(&mut pc, "");
    assert_eq!(lcd(&pc), "2", "lo a\u{f1}adido");
    enter(&mut pc, "");
    assert_eq!(lcd(&pc), ">", "fin");
}

#[test]
fn chain_keeps_variables() {
    let (mut pc, dir) = booted();
    // 10 PRINT A+1
    write_file(dir.path(), "Z.BIN", &[0x00, 0x0A, 0x06, 0xF0, 0x97, b'A', b'+', b'1', 0x0D]);
    // 10 PRINT 1 / 20 PRINT A+2
    write_file(
        dir.path(),
        "Z2.BIN",
        &[0x00, 0x0A, 0x04, 0xF0, 0x97, b'1', 0x0D, 0x00, 0x14, 0x06, 0xF0, 0x97, b'A', b'+', b'2', 0x0D],
    );
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 A=5:CHAIN \"Z\"");
    run_mode(&mut pc);
    enter(&mut pc, "RUN");
    assert_eq!(lcd(&pc), "6", "A sigue valiendo 5");
    enter(&mut pc, "");

    run_mode(&mut pc);
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 A=5:CHAIN \"Z2\",20");
    run_mode(&mut pc);
    enter(&mut pc, "RUN");
    assert_eq!(lcd(&pc), "7", "empieza en la l\u{ed}nea 20");
    enter(&mut pc, "");

    run_mode(&mut pc);
    enter(&mut pc, "NEW");
    enter(&mut pc, "10 CHAIN \"Z2\",30");
    run_mode(&mut pc);
    enter(&mut pc, "RUN");
    assert!(lcd(&pc).starts_with("ERROR 11"), "l\u{ed}nea que no existe: [{}]", lcd(&pc));
}

// --- Nombres y cinta ----------------------------------------------------------------

#[test]
fn bad_names_and_tape() {
    let root = tempdir();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap_or_else(|e| panic!("{e}"));
    let mut pc = booted_without_dir();
    pc.set_host_dir(Some(inner.clone()));
    // (La PC-1500 no tiene '\': ese caso lo cubren los tests de
    // `host_io.rs`.)
    for name in ["../X", "A/B", ".X", "X."] {
        enter(&mut pc, &format!("CSAVE \"{name}\""));
        assert_eq!(lcd(&pc), "ERROR 40", "{name}");
    }
    enter(&mut pc, "CSAVE");
    assert_eq!(lcd(&pc), "ERROR 40", "sin nombre y sin cinta");
    let entries = |dir: &Path| fs::read_dir(dir).map(Iterator::count).unwrap_or_default();
    assert_eq!(entries(&inner), 0, "nada escrito en la carpeta");
    assert_eq!(entries(root.path()), 1, "ni fuera de ella");

    let tape = root.path().join("cinta.bin");
    pc.set_tape(Some(tape.clone()));
    enter(&mut pc, "CSAVE");
    assert_eq!(lcd(&pc), ">", "CSAVE a la cinta");
    assert_eq!(read_file(&tape), MOLE_BIN, "cinta .bin: tokenizado");
    enter(&mut pc, "NEW");
    enter(&mut pc, "CLOAD");
    assert_eq!(lcd(&pc), ">", "CLOAD de la cinta");
    assert_loaded(&pc, MOLE_BIN, "cinta");
}

#[test]
fn reset_keeps_host_config() {
    let (mut pc, dir) = booted();
    let tape = dir.path().join("t.bas");
    pc.set_tape(Some(tape.clone()));
    enter(&mut pc, "CSAVE \"T1\"");
    pc.reset();
    assert_eq!(pc.host_dir(), Some(dir.path()), "carpeta");
    assert_eq!(pc.tape(), Some(tape.as_path()), "cinta");
    assert_eq!(pc.take_host_events().len(), 1, "evento sin recoger");
    assert!(pc.boot().is_some(), "arranca");
    assert_eq!(word(&pc, VAR_START), 0x4800, "m\u{e1}quina nueva");
    enter(&mut pc, "CLOAD \"T1\"");
    assert_eq!(lcd(&pc), ">", "sigue funcionando tras reset");
}
