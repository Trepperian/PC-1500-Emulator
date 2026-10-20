//! Tests de la carga y el guardado de programas BASIC desde el host
//! (`ceres_core::basic`): la tabla de palabras clave leída de la ROM, el
//! tokenizador (contra la propia ROM tecleando en la máquina emulada y
//! contra los `.bin` del corpus), los formatos de archivo y la carga en la
//! máquina (editar, `LIST`, `RUN`, `CONT` después de cargar).
//!
//! Los programas de `tests/data/` son copias del corpus del compilador
//! (`SHARP_PC-1500_Compiler/test/basic/*.bas`) y de los volcados
//! tokenizados de la raíz del repositorio, para no depender de rutas de
//! fuera.

use ceres_core::{
    Key, Pc1500,
    basic::{
        AsciiOptions, BASPRG_EDT, BASPRG_END, BASPRG_ST, BREAKPARAM, BasicError, BasicFormat,
        BasicLoadError, DATA_PTR, Eol, KeywordTable, Keywords, Program, ProgramLine, SaveFormat,
        TOKEN_REM, VAR_START, WarningKind, decode_program, detect_format, tokenize_body,
    },
    harness::KeyStroke,
};

const MOLE_BAS: &str = include_str!("data/mole.bas");
const JACKPOT_BAS: &str = include_str!("data/jackpot.bas");
const BATHYSCAPH_BAS: &str = include_str!("data/bathyscaph.bas");
const MOLE_BIN: &[u8] = include_bytes!("data/mole.bin");
const JACKPOT_BIN: &[u8] = include_bytes!("data/jackpot.bin");
const BATHYSCAPH_BIN: &[u8] = include_bytes!("data/bathyscaph.bin");

/// Bit del indicador `RUN` en `$764F` (ver `Pc1500::lcd_symbol_bytes`).
const SYMBOL_RUN: u8 = 0x40;

fn word(pc: &Pc1500, addr: u16) -> u16 {
    pc.read_word_be(u32::from(addr))
}

fn byte(pc: &Pc1500, addr: u16) -> u8 {
    pc.read_byte(u32::from(addr))
}

fn booted() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "la ROM no lleg\u{f3} al prompt");
    pc
}

/// Arranca y hace `NEW0`: programa vacío en `RAM_ST_H`:$C5 = $00C5 (la RAM
/// del CE-161 empieza en $0000).
fn booted_empty() -> Pc1500 {
    let mut pc = booted();
    enter_line(&mut pc, "NEW0");
    assert_eq!(word(&pc, BASPRG_ST), 0x00C5, "NEW0 pone el inicio en $00C5");
    pc
}

fn enter_line(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

fn enter_lines(pc: &mut Pc1500, text: &str) {
    if let Err(err) = pc.enter_lines(text) {
        panic!("{err}; LCD = [{}]", pc.lcd_text());
    }
}

fn tap(pc: &mut Pc1500, key: Key) {
    pc.tap(KeyStroke::plain(key));
    assert!(pc.run_until_ready(3000).is_some(), "la ROM no qued\u{f3} lista tras {key:?}");
}

fn lcd(pc: &Pc1500) -> String {
    pc.lcd_text().trim_end().to_owned()
}

fn main_tables() -> Vec<KeywordTable> {
    vec![KeywordTable::main_rom().clone()]
}

fn tok(src: &str) -> Vec<u8> {
    tokenize_body(src.as_bytes(), &main_tables()).unwrap_or_else(|e| panic!("{src:?}: {e}"))
}

fn ascii(text: &str, list_format: bool) -> Program {
    let options = AsciiOptions { list_format };
    match Program::from_ascii(text, &Keywords::main_rom(), &options) {
        Ok((program, _)) => program,
        Err(e) => panic!("{e}"),
    }
}

fn tokenized(bytes: &[u8]) -> Program {
    Program::from_tokenized(bytes).unwrap_or_else(|e| panic!("{e}")).0
}

fn load(pc: &mut Pc1500, program: &Program) {
    if let Err(e) = pc.load_basic_program(program) {
        panic!("load_basic_program: {e}; LCD = [{}]", pc.lcd_text());
    }
}

// --- Tabla de palabras clave ---------------------------------------------

#[test]
fn keyword_table_from_rom() {
    let table = KeywordTable::main_rom();
    let entries = table.entries();
    assert_eq!(entries.len(), 85, "entradas de la tabla de $C000");
    assert_eq!(table.base(), 0xC000, "base");
    assert_eq!(table.end(), 0xC34E, "terminador ($C34E = $D0)");
    let first = &entries[0];
    assert_eq!(first.addr, 0xC054, "primera entrada");
    assert_eq!(first.text, b"AREAD", "primera palabra");
    assert_eq!(first.token, 0xF180, "token de AREAD");
    assert_eq!(first.vector, 0xC684, "vector de AREAD");
    // LET_A = $C056: el puntero de la 'A' apunta a la 2.a letra de AREAD.
    assert_eq!(table.first_for_letter(b'A'), Some(0), "primera de la A");
    for letter in [b'H', b'J', b'K', b'Q', b'X', b'Y', b'Z'] {
        assert_eq!(table.first_for_letter(letter), None, "sin palabras con {}", char::from(letter));
    }
    // El bit "última de su letra" está justo en los cambios de letra.
    for (i, entry) in entries.iter().enumerate() {
        let next_letter = entries.get(i + 1).map(|e| e.text[0]);
        assert_eq!(
            entry.last_of_letter,
            next_letter != Some(entry.text[0]),
            "{}",
            String::from_utf8_lossy(&entry.text)
        );
    }
    // Cada puntero de letra apunta a la primera entrada de esa letra.
    for letter in b'A'..=b'Z' {
        let expected = entries.iter().position(|e| e.text[0] == letter);
        assert_eq!(table.first_for_letter(letter), expected, "letra {}", char::from(letter));
    }
    assert_eq!(table.name(TOKEN_REM), Some(&b"REM"[..]), "REM");
    assert_eq!(table.name(0xF097), Some(&b"PRINT"[..]), "PRINT");
    // La rareza de la ROM: una entrada "P    " imposible de teclear.
    assert!(
        entries.iter().any(|e| e.text == b"P    " && e.token == 0xF1A3 && e.addr == 0xC253),
        "entrada ficticia de $C253"
    );
}

#[test]
fn emulated_machine_has_the_main_and_host_tables() {
    let pc = booted();
    let keywords = pc.basic_keywords();
    // La principal y la tabla virtual de `CLOAD`/`CSAVE` en $9800 con
    // PV=0 (ver `host_io.rs`); ninguna ROM de expansión real.
    assert_eq!(keywords.tables().len(), 2, "principal + $9800");
    assert_eq!(&keywords.tables()[0], KeywordTable::main_rom(), "la de $C000 le\u{ed}da de la memoria emulada");
    assert_eq!((keywords.tables()[1].base(), keywords.tables()[1].pv()), (0x9800, false), "la de $9800");
    // CLOAD es de la tabla de $9800; LPRINT solo se conoce al listar.
    assert_eq!(keywords.name(0xF089), Some(&b"CLOAD"[..]), "nombre de CLOAD");
    assert_eq!(keywords.tokenize_body(b"CLOAD").unwrap_or_default(), b"\xF0\x89", "CLOAD tokenizado por $9800");
    assert_eq!(keywords.name(0xF0B9), Some(&b"LPRINT"[..]), "nombre de LPRINT");
    assert_eq!(keywords.tokenize_body(b"LPRINT").unwrap_or_default(), b"L\xF0\x97", "sin CE-150, LPRINT es L + PRINT");
    let with = Keywords::main_rom().with_peripherals();
    assert_eq!(with.tokenize_body(b"CLOAD\"X\"").unwrap_or_default(), b"\xF0\x89\"X\"", "con perif\u{e9}ricos");
    assert_eq!(with.tokenize_body(b"LPRINT A").unwrap_or_default(), b"\xF0\xB9A", "LPRINT");
}

// --- Tokenizador ------------------------------------------------------------

#[test]
fn tokenize_rules() {
    // Abreviaturas: la primera de la tabla que encaje (P. = PRINT, PO. =
    // POKE#... la de la tabla, F1A0).
    assert_eq!(
        tok("P.A;PO.1,2:PR.\"X\""),
        [0xF0, 0x97, 0x41, 0x3B, 0xF1, 0xA0, 0x31, 0x2C, 0x32, 0x3A, 0xF0, 0x97, 0x22, 0x58, 0x22],
        "abreviaturas"
    );
    // Minúsculas sin tokenizar, espacios dentro de cadenas conservados y
    // espacios dentro de una palabra clave saltados.
    let mut expected = b"print\"a b\";".to_vec();
    expected.extend_from_slice(&[0xF1, 0x92]);
    expected.extend_from_slice(b"10");
    assert_eq!(tok("print \"a b\" ; G O T O 10"), expected, "min\u{fa}sculas y G O T O");
    // DATA se tokeniza (solo REM va crudo).
    let mut expected = vec![0xF1, 0x8D, 0xF0, 0x97];
    expected.extend_from_slice(b",AB,\"C D\"");
    assert_eq!(tok("DATA PRINT, A B ,\"C D\""), expected, "DATA");
    // REM: el resto, crudo, con sus espacios y apóstrofos.
    let mut expected = vec![0xF1, 0xAB];
    expected.extend_from_slice(b" PRINT A  B'");
    assert_eq!(tok("REM PRINT A  B'"), expected, "REM");
    // Un GOTO tecleado se queda en GOTO.
    let mut expected = vec![0xF1, 0x96];
    expected.extend_from_slice(b"A=&FF");
    expected.extend_from_slice(&[0xF1, 0x92]);
    expected.extend_from_slice(b"5120");
    assert_eq!(tok("IF A=&FFGOTO 5120"), expected, "IF ... GOTO");
    // Palabras clave dentro de identificadores.
    assert_eq!(tok("ATOB"), [b'A', 0xF1, 0xB1, b'B'], "A TO B");
    // El apóstrofo desaparece también dentro de cadenas.
    assert_eq!(tok("\"IT'S\""), b"\"ITS\"", "ap\u{f3}strofo");
    // ':' tras el número se guarda en el cuerpo (tokenize_body no sabe
    // de números de línea).
    assert_eq!(tok(":A=1"), b":A=1", "dos puntos");
    // Un token dentro de una cadena es el ERROR 28.
    assert_eq!(
        tokenize_body(b"\"\xF0\x97\"", &main_tables()),
        Err(BasicError::TokenInString { line: None }),
        "ERROR 28"
    );
    // Un token ya presente fuera de cadenas se copia.
    assert_eq!(tok("\u{0}").len(), 1, "byte bajo crudo");
    assert_eq!(
        tokenize_body(b"\xF0\x97A", &main_tables()).unwrap_or_default(),
        [0xF0, 0x97, b'A'],
        "token presente"
    );
}

#[test]
fn ascii_reader_rules() {
    // π y √, ':' tras el número en formato LIST, número de línea suelto.
    let program = ascii("10:A=\u{3c0}*\u{221a}4\r\n20 PRINT\"\\x7F\"\n", true);
    let lines = program.lines();
    assert_eq!(lines[0].number, 10, "l\u{ed}nea 10");
    assert_eq!(lines[0].body, [b'A', b'=', 0xF1, 0x5D, b'*', 0xF1, 0x6B, b'4'], "\u{3c0} y \u{221a}");
    assert_eq!(lines[1].body, [0xF0, 0x97, b'"', 0x7F, b'"'], "\\xNN");
    // Sin formato LIST, el ':' se guarda (como al teclear `10:A=1`).
    assert_eq!(ascii("10:A=1", false).lines()[0].body, b":A=1", "fiel a la ROM");
    // Repetidas, orden y avisos.
    let (program, warnings) = Program::from_ascii(
        "' cabecera\n30 B=2\n10 A=1\n10 A=3\n\n20 C$=\"IT'S\"\n",
        &Keywords::main_rom(),
        &AsciiOptions::default(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let numbers: Vec<u16> = program.lines().iter().map(|l| l.number).collect();
    assert_eq!(numbers, [10, 20, 30], "ordenadas");
    assert_eq!(program.lines()[0].body, b"A=3", "gana la \u{fa}ltima");
    let kinds: Vec<&WarningKind> = warnings.iter().map(|w| &w.kind).collect();
    assert!(kinds.contains(&&WarningKind::SkippedNonProgramLines(1)), "{kinds:?}");
    assert!(kinds.contains(&&WarningKind::DuplicateLine), "{kinds:?}");
    assert!(kinds.contains(&&WarningKind::ApostropheDeleted), "{kinds:?}");
    // Errores.
    let err = Program::from_ascii("10 A=\u{e9}", &Keywords::main_rom(), &AsciiOptions::default());
    assert_eq!(
        err.map(|_| ()),
        Err(BasicError::BadCharacter { line: 10, column: 6, ch: '\u{e9}' }),
        "car\u{e1}cter inexistente"
    );
    let err = Program::from_ascii("65280 A=1", &Keywords::main_rom(), &AsciiOptions::default());
    assert!(matches!(err, Err(BasicError::BadLineNumber { number: 65280, .. })), "l\u{ed}nea 65280");
    let err = Program::from_ascii("10 A$=\"\u{3c0}\"", &Keywords::main_rom(), &AsciiOptions::default());
    assert_eq!(err.map(|_| ()), Err(BasicError::TokenInString { line: Some(10) }), "\u{3c0} en cadena");
}

#[test]
fn corpus_bins_match() {
    // Los `.bin` de `tests/data/` se hicieron con el tokenizador a partir de
    // los `.bas` del corpus: los tres son listados exactos.
    for (name, text, bin) in [("mole", MOLE_BAS, MOLE_BIN), ("jackpot", JACKPOT_BAS, JACKPOT_BIN), ("bathyscaph", BATHYSCAPH_BAS, BATHYSCAPH_BIN)] {
        assert_eq!(ascii(text, true).to_tokenized(), bin, "{name}");
    }
}

#[test]
fn roundtrips() {
    let keywords = Keywords::main_rom();
    for (name, bin) in [("mole", MOLE_BIN), ("jackpot", JACKPOT_BIN), ("bathyscaph", BATHYSCAPH_BIN)] {
        let program = tokenized(bin);
        assert_eq!(program.to_tokenized(), bin, "{name}: from/to_tokenized");
        assert_eq!(program.byte_len(), bin.len(), "{name}: byte_len");
        for list_format in [true, false] {
            let options = AsciiOptions { list_format };
            for eol in [Eol::CrLf, Eol::Cr, Eol::Lf] {
                let text = program.to_ascii(eol, &keywords, &options);
                let (back, _) = Program::from_ascii(&text, &keywords, &options).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(back.to_tokenized(), bin, "{name}: texto (LIST={list_format}, {eol:?})");
            }
        }
        let ce_ascii = program.to_ce158_ascii(&keywords);
        assert_eq!(ascii(&ce_ascii, true).to_tokenized(), bin, "{name}: .ASC de la CE-158");
        let ce158 = program.to_ce158("X.BAS", 0x00C5);
        let (back, header, warnings) = Program::from_ce158(&ce158).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(back.to_tokenized(), bin, "{name}: CE-158");
        assert_eq!(header.name, b"X.BAS", "{name}: nombre");
        assert!(warnings.is_empty(), "{name}: {warnings:?}");
    }
    // El listado tiene el aspecto del LCD.
    let mole = tokenized(MOLE_BIN).to_ascii(Eol::Lf, &keywords, &AsciiOptions::default());
    assert!(mole.starts_with("5000 \"MOLE\"\n5010 \"B\":CLEAR \n"), "{mole}");
    assert!(mole.contains("5090 IF A=&FFGOTO 5120\n"), "{mole}");
    let jackpot = tokenized(JACKPOT_BIN).to_ascii(Eol::Lf, &keywords, &AsciiOptions::default());
    assert!(jackpot.starts_with("10 \"A\"REM  * JACKPOT*\n"), "{jackpot}");
}

#[test]
fn tokenized_text_from_ascii_text_roundtrip() {
    // tokenizar -> detokenizar -> tokenizar da lo mismo (los 3 textos).
    let keywords = Keywords::main_rom();
    for text in [MOLE_BAS, JACKPOT_BAS, BATHYSCAPH_BAS] {
        let first = ascii(text, true);
        let again = ascii(&first.to_ascii(Eol::CrLf, &keywords, &AsciiOptions::default()), true);
        assert_eq!(again, first, "ida y vuelta");
    }
}

// --- CE-158 ------------------------------------------------------------------

/// La cabecera de `BATHY.BAS` de Jeff Birt (Sharp_CE-158/Test_Programs),
/// guardado con `CSAVE` en una CE-158X: 933 bytes = esta cabecera + los
/// 906 bytes de `bathyscaph.bin`, sin `$FF`.
const JEFF_BIRT_BATHY_HEADER: [u8; 27] = [
    0x01, b'@', b'C', b'O', b'M', b'B', b'A', b'T', b'H', b'Y', b'.', b'B', b'A', b'S', 0, 0, 0, 0,
    0, 0, 0, 0x00, 0xC5, 0x03, 0x89, 0x04, 0x4F,
];

#[test]
fn ce158_writer_matches_real_csave() {
    let written = tokenized(BATHYSCAPH_BIN).to_ce158("BATHY.BAS", 0x00C5);
    assert_eq!(written.len(), 933, "27 + 906, sin $FF");
    assert_eq!(written[..27], JEFF_BIRT_BATHY_HEADER, "cabecera");
    assert_eq!(&written[27..], BATHYSCAPH_BIN, "programa");
}

#[test]
fn ce158_reader_is_lenient() {
    // La variante del compilador: longitud - 1 contando el $FF, y $FF.
    let mut compiler = JEFF_BIRT_BATHY_HEADER.to_vec();
    compiler[23..25].copy_from_slice(&[0x03, 0x8A]);
    compiler.extend_from_slice(BATHYSCAPH_BIN);
    compiler.push(0xFF);
    let mut jeff = JEFF_BIRT_BATHY_HEADER.to_vec();
    jeff.extend_from_slice(BATHYSCAPH_BIN);
    for (name, file) in [("compilador", &compiler), ("CE-158X", &jeff)] {
        let (program, header, warnings) = Program::from_ce158(file).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(program.to_tokenized(), BATHYSCAPH_BIN, "{name}");
        assert_eq!(header.start, 0x00C5, "{name}: inicio");
        assert!(warnings.is_empty(), "{name}: {warnings:?}");
        assert_eq!(detect_format(Some("txt"), file), BasicFormat::Ce158, "{name}: detecci\u{f3}n");
    }
    // Cortado: se carga lo que haya, con aviso.
    let cut = &jeff[..jeff.len() - 10];
    assert!(Program::from_ce158(cut).is_err(), "l\u{ed}nea cortada");
    assert_eq!(
        Program::from_ce158(b"\x01@CO"),
        Err(BasicError::BadCe158Header("menos de 27 bytes")),
        "demasiado corto"
    );
}

#[test]
fn format_detection() {
    assert_eq!(detect_format(Some("bas"), MOLE_BAS.as_bytes()), BasicFormat::Ascii, "texto .bas");
    assert_eq!(detect_format(None, MOLE_BIN), BasicFormat::Tokenized, "tokenizado sin extensi\u{f3}n");
    assert_eq!(detect_format(Some("bas"), JACKPOT_BIN), BasicFormat::Tokenized, "tokenizado .bas");
    assert_eq!(detect_format(Some("BIN"), b"10 PRINT"), BasicFormat::Tokenized, ".bin manda");
    let mut with_ff = MOLE_BIN.to_vec();
    with_ff.push(0xFF);
    assert_eq!(detect_format(Some("dat"), &with_ff), BasicFormat::Tokenized, "con $FF");
    let decoded = decode_program(&with_ff, BasicFormat::Tokenized, &Keywords::main_rom(), &AsciiOptions::default())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(decoded.program.to_tokenized(), MOLE_BIN, "$FF final ignorado");
    // Texto con bytes Latin-1 en los comentarios.
    let latin1 = b"' \xA9 Sharp\r\n10 PRINT 1\r\n";
    assert_eq!(detect_format(Some("bas"), latin1), BasicFormat::Ascii, "Latin-1");
    let decoded = decode_program(latin1, BasicFormat::Ascii, &Keywords::main_rom(), &AsciiOptions::default())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(decoded.program.lines()[0].body, b"\xF0\x971", "Latin-1 le\u{ed}do");
}

// --- Contra la ROM -----------------------------------------------------------

/// Un programa corto que toca casi todas las reglas del tokenizador y se
/// puede teclear con el arnés (sin apóstrofos ni π).
const TYPED_PROGRAM: &str = "\
10 REM  PRUEBA Uno
20 P.\"HOLA\";:A=5
30 FOR I=1 TO 3:B=B+I:NEXT I
40 IF A=5 GOTO 60
50 DATA 1, X Y ,\"A B\"
60 C$=CHR$ 65:print\"x\"
70 G O S U B 100:END
100 RETURN
";

#[test]
fn rom_typed_equals_host() {
    let mut pc = booted_empty();
    enter_lines(&mut pc, TYPED_PROGRAM);
    let start = word(&pc, BASPRG_ST);
    let end = word(&pc, BASPRG_END);
    let typed = pc.read_bytes(u32::from(start), usize::from(end - start));
    assert_eq!(byte(&pc, end), 0xFF, "$FF en BASPRG_END");
    // Fiel a la ROM: lo que se teclea, tal cual.
    let host = ascii(TYPED_PROGRAM, false);
    assert_eq!(host.to_tokenized(), typed, "ROM vs host");
    assert_eq!(pc.basic_program_bytes(), typed, "basic_program_bytes");
}

#[test]
fn fixture_unchanged_and_fixed() {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.run_until_idle(600).is_some(), "arranque");
    assert_eq!(lcd(&pc), "NEW0? :CHECK", "pantalla de arranque");
    // bathyscaph, donde `NEW0` pone el programa con el CE-161.
    let end = 0x00C5 + u16::try_from(BATHYSCAPH_BIN.len()).unwrap_or(u16::MAX);
    assert_eq!(word(&pc, BASPRG_ST), 0x00C5, "BASPRG_ST");
    assert_eq!(word(&pc, BASPRG_EDT), 0x00C5, "BASPRG_EDT");
    assert_eq!(word(&pc, BASPRG_END), end, "BASPRG_END");
    assert_eq!(byte(&pc, end), 0xFF, "$FF de fin");
    assert_eq!(word(&pc, VAR_START), 0x4800, "VAR_START");
    assert_eq!(pc.basic_program_bytes(), BATHYSCAPH_BIN, "el fixture es bathyscaph.bin");

    let mut pc = booted();
    enter_line(&mut pc, "1020 A=2");
    assert_eq!(word(&pc, BASPRG_END), end + 7, "la l\u{ed}nea nueva ocupa 7 bytes");
    enter_line(&mut pc, "LIST 1015");
    assert!(lcd(&pc).starts_with("1015:"), "LIST 1015: [{}]", lcd(&pc));
    tap(&mut pc, Key::Down);
    assert_eq!(lcd(&pc), "1020:A=2", "la l\u{ed}nea nueva en su sitio");
    // Es la última: abajo no hay nada más (antes, sin el $FF, aparecía
    // basura), y arriba vuelve la 1015.
    tap(&mut pc, Key::Down);
    assert_eq!(lcd(&pc), "1020:A=2", "fin del programa");
    tap(&mut pc, Key::Up);
    assert!(lcd(&pc).starts_with("1015:DATA"), "arriba, la 1015: [{}]", lcd(&pc));
}

#[test]
fn host_load_then_list_edit() {
    let mut pc = booted_empty();
    let report = pc.load_basic_program(&tokenized(JACKPOT_BIN)).unwrap_or_else(|e| panic!("{e}"));
    let end = 0x00C5 + u16::try_from(JACKPOT_BIN.len()).unwrap_or(u16::MAX);
    assert_eq!((report.start, report.end), (0x00C5, end), "inicio y fin");
    assert_eq!(report.bytes, JACKPOT_BIN.len(), "bytes");
    assert_eq!(report.free, usize::from(0x4800 - end - 1), "libres");
    assert_eq!(word(&pc, BASPRG_ST), 0x00C5, "BASPRG_ST");
    assert_eq!(word(&pc, BASPRG_EDT), 0x00C5, "BASPRG_EDT");
    assert_eq!(word(&pc, BASPRG_END), end, "BASPRG_END");
    assert_eq!(byte(&pc, end), 0xFF, "$FF");
    assert_eq!(word(&pc, VAR_START), 0x4800, "VAR_START");
    assert_eq!(word(&pc, DATA_PTR), 0x80C8, "DATA_PTR");

    enter_line(&mut pc, "LIST");
    assert_eq!(lcd(&pc), "10:\"A\"REM  * JACKPOT*", "LIST");
    tap(&mut pc, Key::Down);
    assert!(lcd(&pc).starts_with("20:REM  *AUTEUR"), "Down: [{}]", lcd(&pc));

    tap(&mut pc, Key::Cl);
    enter_line(&mut pc, "15 A=5");
    assert_eq!(word(&pc, BASPRG_END), end + 7, "+7");
    enter_line(&mut pc, "765 B=6");
    assert_eq!(word(&pc, BASPRG_END), end + 14, "+7");
    enter_line(&mut pc, "20");
    assert_eq!(word(&pc, BASPRG_END), end + 14 - 30, "-30");
    enter_line(&mut pc, "LIST 10");
    tap(&mut pc, Key::Down);
    assert_eq!(lcd(&pc), "15:A=5", "la 15 tras la 10");
    tap(&mut pc, Key::Down);
    assert!(lcd(&pc).starts_with("100:REM  * INIT"), "y la 100: [{}]", lcd(&pc));
    tap(&mut pc, Key::Cl);
    enter_line(&mut pc, "LIST 765");
    assert_eq!(lcd(&pc), "765:B=6", "la 765");

    // Lo que hay en RAM es lo esperado.
    let mut expected = tokenized(JACKPOT_BIN).lines().to_vec();
    expected.retain(|l| l.number != 20);
    expected.push(ProgramLine { number: 15, body: b"A=5".to_vec() });
    expected.push(ProgramLine { number: 765, body: b"B=6".to_vec() });
    expected.sort_by_key(|l| l.number);
    assert_eq!(pc.basic_program(), Ok(Program::from_lines(expected)), "programa editado");
}

#[test]
fn host_load_matches_typed_state() {
    const PROGRAM: &str = "10 A=1\n20 PRINT A\n30 DATA 5\n40 GOTO 10\n";
    let mut typed = booted_empty();
    enter_lines(&mut typed, PROGRAM);
    let mut loaded = booted_empty();
    load(&mut loaded, &ascii(PROGRAM, false));
    assert!(loaded.run_until_ready(100).is_some(), "lista tras cargar");
    let typed_sys = typed.read_bytes(0x7860, 0x60);
    let loaded_sys = loaded.read_bytes(0x7860, 0x60);
    let differ: Vec<u32> = (0x7860..)
        .zip(typed_sys.iter().zip(&loaded_sys))
        .filter(|(_, (a, b))| a != b)
        .map(|(addr, _)| addr)
        .collect();
    // Medido en el prototipo: solo cambian el estado del editor de la
    // última línea tecleada ($787D, $7880) y su búsqueda ($78A6-$78A9).
    for addr in &differ {
        assert!(
            [0x787D, 0x7880, 0x78A6, 0x78A7, 0x78A8, 0x78A9].contains(addr),
            "${addr:04X} difiere: tecleado {:02X}, cargado {:02X}",
            typed.read_byte(*addr),
            loaded.read_byte(*addr)
        );
    }
    assert_eq!(typed.basic_program_bytes(), loaded.basic_program_bytes(), "mismo programa");
}

#[test]
fn host_load_run() {
    let mut pc = booted_empty();
    load(&mut pc, &tokenized(MOLE_BIN));
    tap(&mut pc, Key::Mode);
    assert_ne!(pc.lcd_symbol_bytes()[1] & SYMBOL_RUN, 0, "modo RUN");
    pc.type_text("RUN\n").unwrap_or_else(|c| panic!("{c}"));
    let shown = pc.run_until(2000, |pc| pc.lcd_text().contains("GAME START"));
    assert!(shown.is_some(), "RUN de mole: [{}]", pc.lcd_text());
    assert!(!pc.lcd_text().contains("ERROR"), "sin errores");
}

#[test]
fn cont_after_load_errors() {
    let mut pc = booted_empty();
    enter_line(&mut pc, "10 STOP");
    tap(&mut pc, Key::Mode);
    enter_line(&mut pc, "RUN");
    assert_eq!(byte(&pc, BREAKPARAM), 0x80, "BREAKPARAM tras STOP");
    assert!(pc.is_at_basic_prompt(), "tras STOP se puede cargar");
    load(&mut pc, &ascii("10 PRINT 7\n", false));
    assert_eq!(byte(&pc, BREAKPARAM), 0, "BREAKPARAM a 0");
    enter_line(&mut pc, "CONT");
    assert!(lcd(&pc).contains("ERROR 1"), "CONT no sigue en el programa viejo: [{}]", lcd(&pc));
    tap(&mut pc, Key::Cl);
    enter_line(&mut pc, "RUN");
    assert_eq!(lcd(&pc).trim_start(), "7", "el cargado s\u{ed} funciona");
}

#[test]
fn not_at_prompt() {
    for (program, breakparam) in [("10 INPUT A\n", 0x50), ("10 PRINT 5\n20 END\n", 0x20)] {
        let mut pc = booted_empty();
        enter_lines(&mut pc, program);
        tap(&mut pc, Key::Mode);
        enter_line(&mut pc, "RUN");
        assert_eq!(byte(&pc, BREAKPARAM), breakparam, "{program:?}: BREAKPARAM");
        assert!(pc.is_waiting_for_key(), "{program:?}: esperando teclado");
        assert!(!pc.is_at_basic_prompt(), "{program:?}: no es el prompt");
        let before = pc.read_bytes(0x0000, 0x4800);
        let before_sys = pc.read_bytes(0x7860, 0x60);
        assert_eq!(
            pc.load_basic_program(&tokenized(MOLE_BIN)),
            Err(BasicLoadError::NotAtPrompt),
            "{program:?}"
        );
        assert_eq!(pc.read_bytes(0x0000, 0x4800), before, "{program:?}: RAM intacta");
        assert_eq!(pc.read_bytes(0x7860, 0x60), before_sys, "{program:?}: sistema intacto");
    }
}

#[test]
fn too_big() {
    let mut pc = booted_empty();
    // 400 líneas de 50 bytes: 20000 bytes, más que $4800 - $00C5.
    let lines = (1..=400)
        .map(|n| ProgramLine { number: n, body: vec![b'A'; 46] })
        .collect();
    let program = Program::from_lines(lines);
    let before = pc.read_bytes(0x0000, 0x4800);
    let before_sys = pc.read_bytes(0x7860, 0x60);
    assert_eq!(
        pc.load_basic_program(&program),
        Err(BasicLoadError::TooBig { need: 20001, avail: 0x4800 - 0x00C5 }),
        "no cabe"
    );
    assert_eq!(pc.read_bytes(0x0000, 0x4800), before, "RAM intacta");
    assert_eq!(pc.read_bytes(0x7860, 0x60), before_sys, "sistema intacto");
    // El mayor que cabe, sí: 364 líneas de 50 bytes y una de 34, con el
    // $FF justo en $47FF.
    let mut lines: Vec<ProgramLine> =
        (1..=364).map(|n| ProgramLine { number: n, body: vec![b'A'; 46] }).collect();
    lines.push(ProgramLine { number: 365, body: vec![b'A'; 30] });
    let fits = Program::from_lines(lines);
    assert_eq!(fits.byte_len() + 1, 0x4800 - 0x00C5, "justo");
    let report = pc.load_basic_program(&fits).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((report.end, report.free), (0x47FF, 0), "lleno");
}

#[test]
fn extract() {
    let mut pc = booted_empty();
    enter_lines(&mut pc, TYPED_PROGRAM);
    assert_eq!(pc.basic_program(), Ok(ascii(TYPED_PROGRAM, false)), "basic_program");
}

#[test]
fn files_roundtrip_through_the_machine() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let mut pc = booted();
    load(&mut pc, &tokenized(MOLE_BIN));
    for (name, format) in [
        ("mole.bas", SaveFormat::Ce158),
        ("mole.asc", SaveFormat::AsciiCr),
        ("mole.txt", SaveFormat::AsciiCrLf),
        ("mole.bin", SaveFormat::Tokenized),
        ("MOLE", SaveFormat::Ce158),
    ] {
        let path = dir.path().join(name);
        let report = pc.save_basic_file(&path, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(report.format, format, "{name}: formato por extensi\u{f3}n");
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(report.bytes, bytes.len(), "{name}: bytes");
        let mut other = booted_empty();
        let loaded = other.load_basic_file(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(loaded.bytes, MOLE_BIN.len(), "{name}: tama\u{f1}o");
        assert_eq!(other.basic_program_bytes(), MOLE_BIN, "{name}: ida y vuelta");
    }
    let header = std::fs::read(dir.path().join("mole.bas")).unwrap_or_default();
    assert_eq!(&header[..14], b"\x01@COMmole.bas\0", "nombre en la cabecera");
    assert_eq!(&header[21..23], [0x00, 0xC5], "inicio del programa");
    let asc = std::fs::read_to_string(dir.path().join("mole.asc")).unwrap_or_default();
    assert!(asc.starts_with(" 5000 \"MOLE\"\r 5010 \"B\":CLEAR \r"), "{asc:?}");
    assert!(asc.ends_with("\r\r"), "CR extra al final, como la CE-158");
    let mut pc = booted();
    assert!(
        matches!(pc.load_basic_file(&dir.path().join("nope.bas")), Err(BasicLoadError::Io(_))),
        "archivo inexistente"
    );
}

#[test]
fn load_right_after_cold_boot() {
    // Lo que hace la GUI tras arrancar o pulsar Reset: cargar en cuanto la
    // ROM espera teclado, todavía con `NEW0? :CHECK` en pantalla.
    let mut pc = Pc1500::new_deterministic();
    let ready = pc.run_until(600, Pc1500::is_at_basic_prompt);
    assert!(ready.is_some(), "la ROM no lleg\u{f3} al prompt");
    assert_eq!(lcd(&pc), "NEW0? :CHECK", "arranque en fr\u{ed}o");
    let report = pc.load_basic_program(&tokenized(JACKPOT_BIN)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.start, 0x00C5, "en el sitio del programa de arranque");
    assert!(pc.run_until_ready(100).is_some(), "lista");
    tap(&mut pc, Key::Cl);
    assert_eq!(lcd(&pc), ">", "CL quita el mensaje");
    enter_line(&mut pc, "LIST");
    assert_eq!(lcd(&pc), "10:\"A\"REM  * JACKPOT*", "LIST del cargado");
    assert_eq!(pc.basic_program_bytes(), JACKPOT_BIN, "en RAM");
}
