//! Programas BASIC desde y hacia el host.
//!
//! El formato de programa en RAM de la PC-1500, un tokenizador y un
//! detokenizador que leen las palabras clave de las propias tablas de la
//! ROM, los formatos de archivo del host (texto ASCII, volcado tokenizado
//! "crudo" y `.BAS` tokenizado con la cabecera de 27 bytes de la CE-158) y
//! la carga/extracción del programa de la máquina emulada.
//!
//! # El programa en RAM
//! Cada línea es `[nº hi][nº lo][len][cuerpo ... $0D]`, donde `len` cuenta
//! el cuerpo INCLUIDO el `$0D` final (la línea 5000 de `mole`: `13 88 07
//! 22 4D 4F 4C 45 22 0D`). Los números válidos van de 1 a 65279 ($FEFF):
//! un byte alto $FF es la marca de fin. Las líneas van en orden creciente
//! y el programa termina con UN byte $FF en la dirección que guarda
//! `BASPRG_END` ($7867/68). La ROM encuentra el final por ese $FF, no por
//! el puntero (`LINE_SEARCH_6` $D2FF: `LDA (SRCH_LINE_H)` / `CPI A,$FF`),
//! y `NEW` lo escribe ($C857 `ORI (X),$FF` y $C859 `BASPRG_END` = la
//! dirección de ese $FF). El formato no tiene direcciones absolutas (los
//! destinos de `GOTO` son dígitos ASCII o etiquetas), así que un programa
//! se puede cargar en cualquier dirección.
//!
//! # Las palabras clave salen de la ROM
//! No hay ninguna lista de palabras clave escrita a mano para tokenizar:
//! [`KeywordTable`] interpreta la tabla de la ROM principal ($C000) byte a
//! byte, y [`Pc1500::basic_keywords`] recorre además, en el MISMO orden
//! que la rutina `TOK_INBUF` de la ROM, todas las tablas de expansión que
//! haya en la máquina emulada ($B800, $B000, ..., $8000 con `PV`=0 y luego
//! con `PV`=1). Así una tabla que se añada al mapa de memoria (p. ej. la
//! tabla virtual de `CLOAD`/`CSAVE` en $9800) se usa sola, sin tocar este
//! módulo. Para listar archivos de máquinas reales con CE-150/CE-158, el
//! detokenizador conoce además los nombres de los tokens de esos
//! periféricos ([`KeywordTable::peripherals`]).
//!
//! # El tokenizador es la ROM
//! [`tokenize_body`] es una traducción literal de `TOK_INBUF` ($F957), con
//! sus rarezas: el apóstrofo se borra en todas partes (incluso dentro de
//! cadenas), los espacios fuera de cadenas desaparecen, los espacios
//! DENTRO de una palabra clave se saltan al compararla (`G O T O` es
//! `GOTO`), las minúsculas nunca se tokenizan, `.` abrevia a la primera
//! palabra de la tabla que encaje (`P.` = `PRINT`), las palabras clave se
//! encuentran también dentro de identificadores (`ATOB` = `A` `TO` `B`) y
//! tras `REM` el resto de la línea se copia tal cual. Se ha comprobado
//! tecleando en la ROM emulada todas las líneas del corpus de programas
//! (3596 líneas, 0 diferencias de bytes).

use core::{error::Error, fmt, fmt::Write as _, iter};
use std::{borrow::Cow, fs, path::Path, sync::OnceLock};

use crate::{Pc1500, memory};

// --- Direcciones de la RAM del sistema que usa este módulo -------------

/// `RAM_ST_H` ($7863): página de inicio de la RAM, la que encuentra el
/// sondeo de `RESET` ($E098). Con la CE-161 del emulador, $01.
pub const RAM_ST_H: u16 = 0x7863;
/// `RAM_END_H` ($7864): página siguiente al final de la RAM. Con la
/// CE-161, $48 (la RAM acaba en $47FF).
pub const RAM_END_H: u16 = 0x7864;
/// `BASPRG_ST` ($7865/66): primera línea del programa BASIC.
pub const BASPRG_ST: u16 = 0x7865;
/// `BASPRG_END` ($7867/68): dirección del $FF que cierra el programa.
pub const BASPRG_END: u16 = 0x7867;
/// `BASPRG_EDT` ($7869/6A): donde empieza a buscar líneas `LINE_SEARCH`
/// ($D2E0). Es igual a `BASPRG_ST` salvo tras un `MERGE`.
pub const BASPRG_EDT: u16 = 0x7869;
/// `BREAKPARAM` ($788A): estado de ejecución. Medido: $00 en el prompt,
/// $50 esperando un `INPUT`, $20 en la pausa de un `PRINT`, $80 tras
/// `STOP`/BREAK (lo que mira `CONT`, $C8CA `BII (BREAKPARAM),$E0`).
pub const BREAKPARAM: u16 = 0x788A;
/// `VAR_START` ($7899/9A): base de las variables dimensionadas, que
/// crecen hacia abajo desde `RAM_END_H`:00 (`DEL_DIM_VAR`, $D091).
pub const VAR_START: u16 = 0x7899;
/// `DATA_PTR` ($78BE/BF): puntero de `READ`, `(inicio + 3) | $8000` tras
/// `INIT_SYS_ADDR_2` ($CFD2-$CFDA).
pub const DATA_PTR: u16 = 0x78BE;

/// Bits de [`BREAKPARAM`] que indican que la ROM NO está en el prompt
/// sino dentro de un programa (`INPUT` $50, pausa de `PRINT` $20). El
/// bit 7 ($80, tras `STOP`/BREAK) no cuenta: el prompt tras un `STOP` es
/// un prompt normal.
const BREAKPARAM_BUSY: u8 = 0x70;

/// Byte bajo de la dirección de inicio que pone `NEW0` (`BCMD_NEW_4`
/// $C840-$C853: `RAM_ST_H`:$C5, justo después del área de teclas
/// reservadas `RAM_ST_H`:$08-$C4).
const NEW0_START_LOW: u8 = 0xC5;

// --- Bytes y tokens -----------------------------------------------------

/// Fin de línea en el búfer de entrada y en el programa.
const CR: u8 = 0x0D;
/// Marca de fin de programa (byte alto de "número de línea" $FF).
const END_MARK: u8 = 0xFF;
/// Mayor número de línea válido: $FF como byte alto es la marca de fin.
pub const MAX_LINE_NUMBER: u16 = 0xFEFF;
/// Firma de una tabla de palabras clave en su primer byte ($C000 = $55).
const TABLE_SIGNATURE: u8 = 0x55;
/// Token de `REM`: tras él, el resto de la línea va crudo.
pub const TOKEN_REM: u16 = 0xF1AB;
/// Token de `PI` (el carácter π del teclado).
pub const TOKEN_PI: u16 = 0xF15D;
/// Token de `SQR` (el carácter √ del teclado).
pub const TOKEN_SQR: u16 = 0xF16B;
/// Cuerpo (con el `$0D`) más largo que se puede editar en la máquina.
///
/// El búfer de entrada es de 80 caracteres y `FIND_NEXT_LINE` ($DF72, `LDI
/// UL,$4C`) solo busca el `$0D` en 77 bytes, de lo que depende `REM`
/// ($C676).
pub const MAX_EDITABLE_BODY: usize = 79;
/// Cabecera de un archivo `.BAS` de la CE-158: 27 bytes.
pub const CE158_HEADER_LEN: usize = 27;
/// Firma de la cabecera CE-158 (bytes 0-4): `$01 "@COM"`.
const CE158_MAGIC: [u8; 5] = [0x01, b'@', b'C', b'O', b'M'];
/// Bytes del nombre en la cabecera CE-158 (bytes 5-20, rellenos con NUL).
const CE158_NAME_LEN: usize = 16;

// --- Errores y avisos ---------------------------------------------------

/// Por qué no se pudo interpretar un programa o una tabla de palabras
/// clave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BasicError {
    /// La tabla de palabras clave en `base` no tiene el formato de la ROM.
    BadKeywordTable { base: u16, reason: &'static str },
    /// Un token (byte >= $E0) dentro de una cadena: el `ERROR 28` de
    /// `TOK_INBUF` ($F9A5, $FA2D `LDI UH,$1C`). `line` es `None` si se
    /// tokenizó un cuerpo suelto.
    TokenInString { line: Option<u16> },
    /// Un carácter del texto que no existe en la PC-1500 (columna desde
    /// 1, en caracteres de la línea de texto).
    BadCharacter { line: u16, column: usize, ch: char },
    /// Un número de línea fuera de 1..=65279 (`text_line` es la línea del
    /// archivo de texto, desde 1).
    BadLineNumber { text_line: usize, number: u64 },
    /// Una línea cuyo cuerpo tokenizado no cabe en el byte de longitud
    /// (más de 254 bytes más el `$0D`).
    LineTooLong { line: u16, len: usize },
    /// Un programa tokenizado mal formado en `offset`.
    BadTokenized { offset: usize, reason: &'static str },
    /// Una cabecera CE-158 que no lo es.
    BadCe158Header(&'static str),
}

impl fmt::Display for BasicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadKeywordTable { base, reason } => {
                write!(f, "tabla de palabras clave en ${base:04X} no v\u{e1}lida: {reason}")
            }
            Self::TokenInString { line: Some(line) } => {
                write!(f, "l\u{ed}nea {line}: token dentro de una cadena (ERROR 28)")
            }
            Self::TokenInString { line: None } => {
                f.write_str("token dentro de una cadena (ERROR 28)")
            }
            Self::BadCharacter { line, column, ch } => write!(
                f,
                "l\u{ed}nea {line}, columna {column}: el car\u{e1}cter '{ch}' no existe en la PC-1500 \
                 (usar \\xNN para un byte crudo)"
            ),
            Self::BadLineNumber { text_line, number } => write!(
                f,
                "l\u{ed}nea {text_line} del texto: n\u{fa}mero de l\u{ed}nea {number} fuera de 1..={MAX_LINE_NUMBER}"
            ),
            Self::LineTooLong { line, len } => write!(
                f,
                "l\u{ed}nea {line}: {len} bytes tokenizados, m\u{e1}s de los 255 que admite una l\u{ed}nea"
            ),
            Self::BadTokenized { offset, reason } => {
                write!(f, "programa tokenizado no v\u{e1}lido en el byte {offset}: {reason}")
            }
            Self::BadCe158Header(reason) => write!(f, "cabecera CE-158 no v\u{e1}lida: {reason}"),
        }
    }
}

impl Error for BasicError {}

/// Algo raro pero no fatal encontrado al interpretar un programa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// Número de línea BASIC, si el aviso es de una línea concreta.
    pub line: Option<u16>,
    pub kind: WarningKind,
}

/// Ver [`Warning`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WarningKind {
    /// Líneas de texto que no empiezan por un número (comentarios `'`
    /// de cabecera del corpus...) y se han saltado. Las vacías no cuentan.
    SkippedNonProgramLines(usize),
    /// El número de línea ya estaba: gana la última, como en el editor.
    DuplicateLine,
    /// La línea tenía apóstrofos fuera de `REM`: la ROM los borra
    /// siempre, también dentro de cadenas ($F962 `CPI A,$27`).
    ApostropheDeleted,
    /// Cuerpo (con el `$0D`) de más de [`MAX_EDITABLE_BODY`] bytes: se
    /// carga y se ejecuta, pero no se puede editar en la máquina.
    BodyLongerThan79 { len: usize },
    /// Las líneas de un archivo tokenizado no van en orden creciente (se
    /// cargan tal cual; la ROM no encontrará bien algunas).
    UnsortedLines,
    /// La cabecera CE-158 anuncia más bytes de los que trae el archivo.
    Ce158Truncated { declared: usize, present: usize },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "l\u{ed}nea {line}: ")?;
        }
        match &self.kind {
            WarningKind::SkippedNonProgramLines(n) => {
                write!(f, "{n} l\u{ed}neas de texto sin n\u{fa}mero ignoradas")
            }
            WarningKind::DuplicateLine => f.write_str("n\u{fa}mero repetido, gana la \u{fa}ltima"),
            WarningKind::ApostropheDeleted => {
                f.write_str("la PC-1500 borra los ap\u{f3}strofos (') fuera de REM")
            }
            WarningKind::BodyLongerThan79 { len } => write!(
                f,
                "{len} bytes, m\u{e1}s de {MAX_EDITABLE_BODY}: no se podr\u{e1} editar en la m\u{e1}quina"
            ),
            WarningKind::UnsortedLines => f.write_str("l\u{ed}neas fuera de orden"),
            WarningKind::Ce158Truncated { declared, present } => write!(
                f,
                "la cabecera CE-158 anuncia {declared} bytes y el archivo trae {present}"
            ),
        }
    }
}

// --- Tablas de palabras clave -------------------------------------------

/// Una entrada de una tabla de palabras clave de la ROM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyword {
    /// Dirección de su byte de control (0 en las tablas sintéticas).
    pub addr: u16,
    /// El texto de la palabra clave (`b"PRINT"`).
    pub text: Vec<u8>,
    /// El token de 2 bytes ($F097 para `PRINT`).
    pub token: u16,
    /// Dirección de la rutina que la ejecuta (0 en las sintéticas).
    pub vector: u16,
    /// Bit 4 de sus atributos: es la última palabra de su letra, y si no
    /// encaja el tokenizador pasa a la tabla siguiente (`TOK_INBUF_25`
    /// $FA36-$FA3E: `ANI A,$10` / `BZS TOK_INBUF_16`).
    pub last_of_letter: bool,
}

/// Una tabla de palabras clave, tal como la recorre la ROM.
///
/// # Formato (comprobado en el `.lst` de la A04)
/// - `base + $00` = $55 (firma).
/// - `base + $20 + 2·(letra-'A')`: puntero big-endian al SEGUNDO carácter
///   de la primera palabra de esa letra, o $0000 si no hay ninguna (en la
///   ROM principal H, J, K, Q, X, Y y Z). La ROM solo mira el byte alto.
/// - Desde `base + $54`, entradas `[ctl][texto][token hi][token lo]
///   [vector hi][vector lo]`. El nibble bajo de `ctl` es la longitud del
///   texto; el alto son los atributos de la entrada ANTERIOR (macro
///   `CNIB` de `PC-1500_Macros.lib`), así que los de la entrada k están
///   en el `ctl` de la k+1.
/// - Termina en un `ctl` con nibble bajo 0 ($C34E = $D0 en la A04).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeywordTable {
    base: u16,
    pv: bool,
    entries: Vec<Keyword>,
    first_by_letter: [Option<usize>; 26],
    end: u16,
}

/// Tokens de los periféricos CE-150 (impresora/casete) y CE-158 (RS-232/
/// paralelo), para LISTAR programas de máquinas reales que los usen. Son
/// los de las tablas de esas ROMs (CE-150 $B800, CE-158 $8000/$8800 con
/// `PV`=1), que el emulador no tiene.
const PERIPHERAL_KEYWORDS: [(&str, u16); 38] = [
    ("LPRINT", 0xF0B9),
    ("LLIST", 0xF0B8),
    ("LF", 0xF0B6),
    ("COLOR", 0xF0B5),
    ("CSIZE", 0xE680),
    ("GRAPH", 0xE681),
    ("GLCURSOR", 0xE682),
    ("LCURSOR", 0xE683),
    ("SORGN", 0xE684),
    ("ROTATE", 0xE685),
    ("TEXT", 0xE686),
    ("LINE", 0xF0B7),
    ("RLINE", 0xF0BA),
    ("FEED", 0xF0B0),
    ("RMT", 0xE7A9),
    ("CHAIN", 0xF0B2),
    ("TEST", 0xF0BC),
    ("CLOAD", 0xF089),
    ("CSAVE", 0xF095),
    ("MERGE", 0xF08F),
    ("BREAK", 0xF0B3),
    ("TAB", 0xF0BB),
    ("ZONE", 0xF0B4),
    ("ERL", 0xF053),
    ("ERN", 0xF052),
    ("SPACE$", 0xF061),
    ("CONSOLE", 0xF0B1),
    ("SETCOM", 0xE882),
    ("SETDEV", 0xE886),
    ("PROTOCOL", 0xE881),
    ("DTE", 0xE884),
    ("DEV$", 0xE857),
    ("COM$", 0xE858),
    ("INSTAT", 0xE859),
    ("OUTSTAT", 0xE880),
    ("TRANSMIT", 0xE885),
    ("TERMINAL", 0xE883),
    ("RINKEY$", 0xE85A),
];

/// Dirección de la tabla de la ROM principal.
pub const MAIN_TABLE_BASE: u16 = 0xC000;
/// Primera entrada de una tabla (`base + $54`).
const FIRST_ENTRY_OFFSET: u16 = 0x54;
/// Punteros por letra (`base + $20`).
const LETTER_POINTERS_OFFSET: u16 = 0x20;
/// Tope de entradas al recorrer una tabla: una tabla rota (o memoria sin
/// tabla) no puede hacer que el recorrido no termine.
const MAX_TABLE_ENTRIES: usize = 512;

impl KeywordTable {
    /// Interpreta la tabla en `table_base` de una imagen de ROM `rom` que
    /// empieza en `rom_base`. Fuera de la imagen se lee $FF.
    pub fn parse(rom: &[u8], rom_base: u16, table_base: u16) -> Result<Self, BasicError> {
        Self::parse_with(table_base, false, |addr| {
            rom.get(usize::from(addr.wrapping_sub(rom_base)))
                .copied()
                .unwrap_or(0xFF)
        })
    }

    /// Interpreta la tabla en `base` leyendo cada byte con `read` (p. ej.
    /// de la memoria emulada con el `PV` que toque, ver
    /// [`Pc1500::basic_keywords`]). `pv` solo se guarda, para saber de
    /// dónde salió.
    pub fn parse_with<F: Fn(u16) -> u8>(base: u16, pv: bool, read: F) -> Result<Self, BasicError> {
        let bad = |reason| BasicError::BadKeywordTable { base, reason };
        if read(base) != TABLE_SIGNATURE {
            return Err(bad("falta la firma $55"));
        }
        let mut entries: Vec<Keyword> = Vec::new();
        let mut addr = base.wrapping_add(FIRST_ENTRY_OFFSET);
        // Atributos de cada entrada = nibble alto del `ctl` de la
        // siguiente (o del terminador).
        let mut next_ctls = Vec::new();
        loop {
            let ctl = read(addr);
            if !entries.is_empty() {
                next_ctls.push(ctl);
            }
            let len = ctl & 0x0F;
            if len == 0 {
                break;
            }
            if entries.len() >= MAX_TABLE_ENTRIES {
                return Err(bad("no termina"));
            }
            let at = |offset: u8| read(addr.wrapping_add(u16::from(offset)));
            let text: Vec<u8> = (1..=len).map(at).collect();
            if text.iter().any(|&b| b >= 0xE0) {
                return Err(bad("texto de palabra clave con bytes >= $E0"));
            }
            let token = u16::from(at(len + 1)) << 8 | u16::from(at(len + 2));
            let vector = u16::from(at(len + 3)) << 8 | u16::from(at(len + 4));
            if token < 0xE100 {
                // El tokenizador reconoce el final del texto por un byte
                // > $E0 ($F9F9 `CPI A,$E0`, `CIN` del '.' en $F9EF).
                return Err(bad("token menor que $E100"));
            }
            entries.push(Keyword { addr, text, token, vector, last_of_letter: false });
            let Some(next) = addr.checked_add(5 + u16::from(len)) else {
                return Err(bad("se sale de la memoria"));
            };
            addr = next;
        }
        for (entry, ctl) in entries.iter_mut().zip(next_ctls) {
            entry.last_of_letter = ctl & 0x10 != 0;
        }

        let mut first_by_letter = [None; 26];
        for (letter, slot) in (0_u16..).zip(first_by_letter.iter_mut()) {
            let ptr_addr = base.wrapping_add(LETTER_POINTERS_OFFSET + 2 * letter);
            let hi = read(ptr_addr);
            if hi == 0 {
                continue;
            }
            let ptr = u16::from(hi) << 8 | u16::from(read(ptr_addr.wrapping_add(1)));
            let entry_addr = ptr.wrapping_sub(2);
            let Some(index) = entries.iter().position(|e| e.addr == entry_addr) else {
                return Err(bad("un puntero de letra no apunta a ninguna entrada"));
            };
            *slot = Some(index);
        }

        Ok(Self { base, pv, entries, first_by_letter, end: addr })
    }

    /// La tabla de la ROM principal A04 ($C000), interpretada una sola vez
    /// de la imagen de ROM del emulador.
    pub fn main_rom() -> &'static Self {
        static TABLE: OnceLock<KeywordTable> = OnceLock::new();
        TABLE.get_or_init(|| {
            Self::parse(memory::PC1500_ROM_BYTES, MAIN_TABLE_BASE, MAIN_TABLE_BASE)
                .unwrap_or_else(|_| Self::empty(MAIN_TABLE_BASE))
        })
    }

    /// Los tokens de los periféricos CE-150/CE-158 (ver
    /// `PERIPHERAL_KEYWORDS`) como una tabla sintética: por defecto solo
    /// sirven para detokenizar (ver [`Keywords`]). Dentro de cada letra
    /// van en el orden de la lista; `addr` y `vector` son 0.
    pub fn peripherals() -> &'static Self {
        static TABLE: OnceLock<KeywordTable> = OnceLock::new();
        TABLE.get_or_init(|| Self::from_list(&PERIPHERAL_KEYWORDS))
    }

    /// Tabla sintética a partir de pares (texto, token), agrupados por su
    /// primera letra en el orden dado.
    fn from_list(list: &[(&str, u16)]) -> Self {
        let mut entries: Vec<Keyword> = Vec::new();
        let mut first_by_letter = [None; 26];
        for (letter, slot) in (b'A'..=b'Z').zip(first_by_letter.iter_mut()) {
            let group: Vec<&(&str, u16)> = list
                .iter()
                .filter(|(text, _)| text.as_bytes().first() == Some(&letter))
                .collect();
            if group.is_empty() {
                continue;
            }
            *slot = Some(entries.len());
            let last = group.len() - 1;
            for (i, (text, token)) in group.into_iter().enumerate() {
                entries.push(Keyword {
                    addr: 0,
                    text: text.as_bytes().to_vec(),
                    token: *token,
                    vector: 0,
                    last_of_letter: i == last,
                });
            }
        }
        Self { base: 0, pv: false, entries, first_by_letter, end: 0 }
    }

    const fn empty(base: u16) -> Self {
        Self { base, pv: false, entries: Vec::new(), first_by_letter: [None; 26], end: base }
    }

    /// Las entradas, en el orden de la tabla.
    #[must_use]
    pub fn entries(&self) -> &[Keyword] {
        &self.entries
    }

    /// Índice de la primera entrada de `letter` (`b'A'..=b'Z'`).
    #[must_use]
    pub fn first_for_letter(&self, letter: u8) -> Option<usize> {
        let index = usize::from(letter.checked_sub(b'A')?);
        *self.first_by_letter.get(index)?
    }

    /// Dirección de la tabla (0 en las sintéticas).
    #[must_use]
    pub const fn base(&self) -> u16 {
        self.base
    }

    /// `PV` con el que se leyó la tabla.
    #[must_use]
    pub const fn pv(&self) -> bool {
        self.pv
    }

    /// Dirección del byte terminador (nibble bajo 0).
    #[must_use]
    pub const fn end(&self) -> u16 {
        self.end
    }

    /// El texto de `token`: la primera entrada que lo tenga, como el
    /// recorrido de `LIST` (`TOK_PROCESS_5/6` $FAB7-$FAEF).
    #[must_use]
    pub fn name(&self, token: u16) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|e| e.token == token)
            .map(|e| e.text.as_slice())
    }
}

/// El conjunto de tablas con que se tokeniza (en orden de búsqueda) y se
/// detokeniza.
///
/// Para tokenizar solo se usan las tablas de búsqueda; para detokenizar,
/// estas y luego los nombres de los periféricos
/// ([`KeywordTable::peripherals`]), que la máquina emulada no tiene pero
/// los archivos de máquinas reales sí pueden traer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keywords {
    search: Vec<KeywordTable>,
    peripherals_searched: bool,
}

impl Keywords {
    /// Solo la tabla de la ROM principal: lo mismo que hace la ROM de un
    /// emulador sin nada mapeado en $8000-$BFFF. Para la máquina emulada
    /// tal cual está, mejor [`Pc1500::basic_keywords`].
    #[must_use]
    pub fn main_rom() -> Self {
        Self::from_tables(vec![KeywordTable::main_rom().clone()])
    }

    /// Las tablas dadas, en el orden en que las buscará el tokenizador.
    #[must_use]
    pub const fn from_tables(search: Vec<KeywordTable>) -> Self {
        Self { search, peripherals_searched: false }
    }

    /// Añade los tokens de los periféricos CE-150/CE-158 también al
    /// TOKENIZAR (después de las demás tablas), para preparar archivos
    /// para una máquina real que los tenga. No es lo que haría la ROM
    /// emulada, que los dejaría como letras sueltas.
    #[must_use]
    pub fn with_peripherals(mut self) -> Self {
        if !self.peripherals_searched {
            self.search.push(KeywordTable::peripherals().clone());
            self.peripherals_searched = true;
        }
        self
    }

    /// Las tablas de búsqueda, en orden.
    #[must_use]
    pub fn tables(&self) -> &[KeywordTable] {
        &self.search
    }

    /// El texto de `token` en la primera tabla que lo tenga (las de
    /// búsqueda y luego las de los periféricos).
    #[must_use]
    pub fn name(&self, token: u16) -> Option<&[u8]> {
        self.search
            .iter()
            .find_map(|t| t.name(token))
            .or_else(|| KeywordTable::peripherals().name(token))
    }

    /// [`tokenize_body`] con estas tablas.
    pub fn tokenize_body(&self, src: &[u8]) -> Result<Vec<u8>, BasicError> {
        tokenize_body(src, &self.search)
    }
}

impl Default for Keywords {
    fn default() -> Self {
        Self::main_rom()
    }
}

// --- Tokenizador (TOK_INBUF, $F957) --------------------------------------

/// Resultado interno del tokenizador: el cuerpo y lo que conviene avisar.
struct Tokenized {
    body: Vec<u8>,
    apostrophes_deleted: bool,
}

/// Tokeniza el cuerpo de una línea exactamente como `TOK_INBUF` ($F957).
///
/// `src` es lo que va tras el número de línea, sin `$0D`; las palabras
/// clave se buscan en `tables` por orden. Leer más allá del final da
/// `$0D`, como el búfer de entrada de la ROM.
///
/// Las reglas, en el orden de la ROM:
/// 1. `'` ($27) se salta siempre ($F962), también en cadenas.
/// 2. `$0D` termina la línea.
/// 3. `"` cambia el estado "dentro de cadena" y se copia ($F96E).
/// 4. Dentro de una cadena, un byte >= $E0 es el `ERROR 28` ($F9A5) y lo
///    demás se copia tal cual.
/// 5. Fuera, el espacio se descarta ($F9AB).
/// 6. Lo que esté por debajo de `A` o en $5B..$DF (minúsculas incluidas)
///    se copia ($F9AF/$F9B3/$F9B7).
/// 7. Un byte >= $E0 es un token ya presente: se copian 2 bytes.
/// 8. Una letra `A`-`Z` busca una palabra clave: tabla por tabla, desde
///    la primera entrada de su letra, comparando el resto del texto con
///    lo que sigue en la entrada y saltando ahí espacios y apóstrofos
///    ($F9E1/$F9E5). Un `.` acepta la entrada actual ($F9E9). Si no
///    encaja, la entrada siguiente, salvo si era la última de su letra,
///    que pasa a la tabla siguiente. Si no hay ninguna, la letra se copia
///    y se sigue por el carácter siguiente: por eso hay palabras clave
///    dentro de los identificadores.
/// 9. El token se escribe big-endian; si es `REM` ($F1AB), el resto de la
///    línea se copia crudo ($FA0D-$FA1B).
pub fn tokenize_body(src: &[u8], tables: &[KeywordTable]) -> Result<Vec<u8>, BasicError> {
    tokenize(src, tables).map(|t| t.body)
}

fn tokenize(src: &[u8], tables: &[KeywordTable]) -> Result<Tokenized, BasicError> {
    let at = |i: usize| src.get(i).copied().unwrap_or(CR);
    let mut out = Vec::with_capacity(src.len());
    let mut y = 0;
    let mut in_string = false;
    // Hasta dónde llega lo tokenizado (el resto, tras `REM`, va crudo):
    // para el aviso de apóstrofos.
    let mut raw_from = None;
    loop {
        let a = at(y);
        y += 1;
        if a == b'\'' {
            continue;
        }
        if a == CR {
            break;
        }
        if a == b'"' {
            in_string = !in_string;
            out.push(a);
            continue;
        }
        if in_string {
            if a >= 0xE0 {
                return Err(BasicError::TokenInString { line: None });
            }
            out.push(a);
            continue;
        }
        if a == b' ' {
            continue;
        }
        let token = if a >= 0xE0 {
            // Token ya presente: los 2 bytes tal cual (`LIN Y` del byte
            // bajo aunque sea el `$0D`, como la ROM).
            let low = at(y);
            y += 1;
            u16::from(a) << 8 | u16::from(low)
        } else if a.is_ascii_uppercase() {
            let Some((token, next)) = find_keyword(src, y, a, tables) else {
                out.push(a);
                continue;
            };
            y = next;
            token
        } else {
            out.push(a);
            continue;
        };
        out.push((token >> 8) as u8);
        out.push((token & 0xFF) as u8);
        if token == TOKEN_REM {
            let rest = src.get(y..).unwrap_or_default();
            let rest = rest.split(|&b| b == CR).next().unwrap_or_default();
            out.extend_from_slice(rest);
            raw_from = Some(y);
            break;
        }
    }
    let tokenized_part = &src[..raw_from.unwrap_or(src.len()).min(src.len())];
    let tokenized_part = tokenized_part.split(|&b| b == CR).next().unwrap_or_default();
    Ok(Tokenized { body: out, apostrophes_deleted: tokenized_part.contains(&b'\'') })
}

/// La búsqueda de la regla 8 de [`tokenize_body`]: el token y por dónde
/// seguir en `src`, o `None`. `y` apunta al carácter que sigue a la letra.
fn find_keyword(src: &[u8], y: usize, letter: u8, tables: &[KeywordTable]) -> Option<(u16, usize)> {
    let at = |i: usize| src.get(i).copied().unwrap_or(CR);
    for table in tables {
        let Some(mut index) = table.first_for_letter(letter) else {
            continue;
        };
        while let Some(entry) = table.entries.get(index) {
            // La comparación empieza en el SEGUNDO carácter del texto (a
            // donde apunta el puntero de la letra).
            let mut yy = y;
            let mut xi = 1;
            let matched = loop {
                let b = at(yy);
                yy += 1;
                if b == b' ' || b == b'\'' {
                    continue;
                }
                if b == b'.' {
                    break true;
                }
                if entry.text.get(xi) != Some(&b) {
                    break false;
                }
                xi += 1;
                if xi >= entry.text.len() {
                    break true;
                }
            };
            if matched {
                return Some((entry.token, yy));
            }
            if entry.last_of_letter {
                break;
            }
            index += 1;
        }
    }
    None
}

// --- El programa ----------------------------------------------------------

/// Una línea de programa: número y cuerpo tokenizado SIN el `$0D`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramLine {
    pub number: u16,
    pub body: Vec<u8>,
}

/// Fin de línea al escribir texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eol {
    /// `\r\n`, la convención de los `.bas` del corpus (y de `.txt`).
    CrLf,
    /// `\r`, la de los `.ASC` de la CE-158(X).
    Cr,
    /// `\n`.
    Lf,
}

impl Eol {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
            Self::Lf => "\n",
        }
    }
}

/// Opciones del texto ASCII.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AsciiOptions {
    /// Formato de `LIST` (lo predeterminado), en lectura y escritura:
    /// - Al leer se quita un `:` justo tras el número (el LCD muestra
    ///   `10:PRINT`) y, si tras `REM` hay dos espacios, uno (el LCD y
    ///   este escritor ponen un espacio tras cada palabra clave).
    /// - Al escribir, `REM` va seguido de un espacio si el resto de la
    ///   línea empieza por espacio (`REM  * JACKPOT*`, como el LCD).
    ///
    /// Con `false` el texto se toma tal cual se teclearía en la máquina
    /// (fiel a la ROM byte a byte).
    pub list_format: bool,
}

impl Default for AsciiOptions {
    fn default() -> Self {
        Self { list_format: true }
    }
}

/// La cabecera de 27 bytes de un `.BAS` tokenizado de la CE-158.
///
/// `$01 "@COM"`, nombre (16 bytes, relleno con NUL), dirección de inicio,
/// longitud − 1 y dirección final (la del `$FF`), todo big-endian. Es lo
/// que escribe `CSAVE` en la ROM de la CE-158: `SUB_9CEC` envía los 27
/// bytes de `OUT_BUF` ($7B60-$7B7A), con el inicio en +$15 (de
/// `PRGST2XREG`, `STATUS_1M2_UREG` $DFFA de la A04: X = `BASPRG_ST`, U =
/// `BASPRG_END` − 1 − X), la longitud − 1 en +$17 y `BASPRG_END` en +$19
/// ($9980). Tras ella van los bytes del programa SIN el `$FF`: en los
/// archivos de Jeff Birt (`BATHY.BAS`, 933 = 27 + 906 bytes, longitud − 1
/// = $0389; `DUN`, $1CBD) y en el volcado de RAM `BASPRG_ST..BASPRG_END`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ce158Header {
    /// Nombre, sin el relleno de NUL.
    pub name: Vec<u8>,
    pub start: u16,
    pub size_minus_one: u16,
    pub end: u16,
}

/// Un programa BASIC tokenizado: líneas con número y cuerpo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    lines: Vec<ProgramLine>,
}

impl Program {
    /// Programa con estas líneas, en este orden (sin comprobar nada).
    #[must_use]
    pub const fn from_lines(lines: Vec<ProgramLine>) -> Self {
        Self { lines }
    }

    /// Las líneas.
    #[must_use]
    pub fn lines(&self) -> &[ProgramLine] {
        &self.lines
    }

    /// Bytes que ocupa en RAM, sin el `$FF` final.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.lines.iter().map(|l| l.body.len() + 4).sum()
    }

    /// Interpreta un programa tokenizado: líneas hasta el final de los
    /// bytes o hasta un byte alto $FF (lo que haya detrás se ignora).
    /// Cada cuerpo debe terminar en `$0D` y cada número estar en
    /// 1..=65279; el orden solo se avisa.
    pub fn from_tokenized(bytes: &[u8]) -> Result<(Self, Vec<Warning>), BasicError> {
        let mut lines = Vec::new();
        let mut warnings = Vec::new();
        let mut offset = 0;
        let bad = |at, reason| BasicError::BadTokenized { offset: at, reason };
        while let Some(&hi) = bytes.get(offset) {
            if hi == END_MARK {
                break;
            }
            let (Some(&lo), Some(&len)) = (bytes.get(offset + 1), bytes.get(offset + 2)) else {
                return Err(bad(offset, "l\u{ed}nea cortada"));
            };
            let number = u16::from(hi) << 8 | u16::from(lo);
            if number == 0 {
                return Err(bad(offset, "n\u{fa}mero de l\u{ed}nea 0"));
            }
            if len == 0 {
                return Err(bad(offset + 2, "longitud 0"));
            }
            let body_start = offset + 3;
            let Some(body) = bytes.get(body_start..body_start + usize::from(len)) else {
                return Err(bad(offset, "l\u{ed}nea cortada"));
            };
            let Some((&CR, body)) = body.split_last() else {
                return Err(bad(body_start + usize::from(len) - 1, "el cuerpo no termina en $0D"));
            };
            if lines.last().is_some_and(|l: &ProgramLine| l.number >= number)
                && !warnings.iter().any(|w: &Warning| w.kind == WarningKind::UnsortedLines)
            {
                warnings.push(Warning { line: Some(number), kind: WarningKind::UnsortedLines });
            }
            lines.push(ProgramLine { number, body: body.to_vec() });
            offset = body_start + usize::from(len);
        }
        Ok((Self { lines }, warnings))
    }

    /// Los bytes del programa tal como van en RAM, SIN el `$FF` final
    /// (como los `.bin` del corpus y lo que hay entre `BASPRG_ST` y
    /// `BASPRG_END`).
    #[must_use]
    pub fn to_tokenized(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.byte_len());
        for line in &self.lines {
            out.push((line.number >> 8) as u8);
            out.push((line.number & 0xFF) as u8);
            // `from_ascii` no deja pasar cuerpos de más de 254 bytes; con
            // `from_lines` es cosa de quien lo construya.
            out.push(u8::try_from(line.body.len() + 1).unwrap_or(u8::MAX));
            out.extend_from_slice(&line.body);
            out.push(CR);
        }
        out
    }

    /// Lee un listado de texto.
    ///
    /// - Las líneas acaban en CRLF, CR (los `.ASC` de la CE-158) o LF.
    /// - Se quitan espacios y tabuladores iniciales. Una línea que no
    ///   empiece por un dígito se salta (y se cuenta en un aviso): las
    ///   cabeceras `'` del corpus.
    /// - El número es la tira de dígitos inicial (1..=65279). Con
    ///   [`AsciiOptions::list_format`], un `:` justo detrás se quita.
    /// - π es el token `PI` y √ el de `SQR`; el tabulador es un espacio;
    ///   `\xNN` (dos dígitos hexadecimales) es el byte $NN tal cual; el
    ///   resto de ASCII imprimible pasa igual. Cualquier otro carácter es
    ///   un error con su línea y columna.
    /// - Un número repetido sustituye al anterior (como el editor) y las
    ///   líneas se ordenan.
    pub fn from_ascii(
        text: &str,
        keywords: &Keywords,
        options: &AsciiOptions,
    ) -> Result<(Self, Vec<Warning>), BasicError> {
        let mut lines: Vec<ProgramLine> = Vec::new();
        let mut warnings = Vec::new();
        let mut skipped = 0;
        for (text_index, raw) in split_text_lines(text).enumerate() {
            let line_text = raw.trim_start_matches([' ', '\t']);
            let digits = line_text.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 {
                if !line_text.trim().is_empty() {
                    skipped += 1;
                }
                continue;
            }
            let (number_text, mut rest) = line_text.split_at(digits);
            let number = number_text
                .bytes()
                .try_fold(0_u64, |n, d| n.checked_mul(10)?.checked_add(u64::from(d - b'0')))
                .unwrap_or(u64::MAX);
            let number = u16::try_from(number)
                .ok()
                .filter(|n| (1..=MAX_LINE_NUMBER).contains(n))
                .ok_or(BasicError::BadLineNumber { text_line: text_index + 1, number })?;
            if options.list_format {
                rest = rest.strip_prefix(':').unwrap_or(rest);
            }
            let column_base = raw.chars().count() - rest.chars().count();
            let src = encode_text(rest, number, column_base)?;
            let tokenized = tokenize(&src, keywords.tables()).map_err(|e| match e {
                BasicError::TokenInString { .. } => BasicError::TokenInString { line: Some(number) },
                other => other,
            })?;
            let mut body = tokenized.body;
            if options.list_format {
                collapse_rem_double_space(&mut body);
            }
            if body.len() + 1 > usize::from(u8::MAX) {
                return Err(BasicError::LineTooLong { line: number, len: body.len() + 1 });
            }
            if tokenized.apostrophes_deleted {
                warnings.push(Warning { line: Some(number), kind: WarningKind::ApostropheDeleted });
            }
            if body.len() + 1 > MAX_EDITABLE_BODY {
                warnings.push(Warning {
                    line: Some(number),
                    kind: WarningKind::BodyLongerThan79 { len: body.len() + 1 },
                });
            }
            if let Some(old) = lines.iter_mut().find(|l| l.number == number) {
                old.body = body;
                warnings.push(Warning { line: Some(number), kind: WarningKind::DuplicateLine });
            } else {
                lines.push(ProgramLine { number, body });
            }
        }
        lines.sort_by_key(|l| l.number);
        if skipped > 0 {
            warnings.insert(0, Warning { line: None, kind: WarningKind::SkippedNonProgramLines(skipped) });
        }
        Ok((Self { lines }, warnings))
    }

    /// El listado como texto: `{número} {cuerpo}` y `eol` por línea, con
    /// cada palabra clave seguida de un espacio, como `LIST` (`70
    /// A=RND (5)+CHR$ 65`, `IF A=&FFGOTO 5120`). Los bytes fuera de
    /// $20..=$7E, la barra invertida y los tokens desconocidos se
    /// escriben como `\xNN`. [`Program::from_ascii`] con las mismas
    /// opciones lo vuelve a convertir en exactamente los mismos bytes.
    #[must_use]
    pub fn to_ascii(&self, eol: Eol, keywords: &Keywords, options: &AsciiOptions) -> String {
        let mut out = String::new();
        for line in &self.lines {
            write!(out, "{} ", line.number).ok();
            detokenize_body(&mut out, &line.body, keywords, *options);
            out.push_str(eol.as_str());
        }
        out
    }

    /// El listado en el formato de los `.ASC` de la CE-158(X) (`CSAVEa`,
    /// los `BATHY.ASC`/`HEY.ASC` de Jeff Birt): número alineado a la
    /// derecha en 5 caracteres, un espacio, el cuerpo, CR, y un CR más al
    /// final. Se lee con [`Program::from_ascii`] como cualquier otro.
    #[must_use]
    pub fn to_ce158_ascii(&self, keywords: &Keywords) -> String {
        let options = AsciiOptions::default();
        let mut out = String::new();
        for line in &self.lines {
            write!(out, "{:>5} ", line.number).ok();
            detokenize_body(&mut out, &line.body, keywords, options);
            out.push('\r');
        }
        out.push('\r');
        out
    }

    /// Lee un `.BAS` con cabecera CE-158 (ver [`Ce158Header`]), con
    /// manga ancha: se exige la firma `$01 "@COM"`, se toman como mucho
    /// `longitud − 1 + 1` bytes tras la cabecera y el `$FF` final es
    /// opcional (los archivos de Jeff Birt no lo traen; otros, con
    /// longitud − 1 contando el `$FF`, sí). Las direcciones no se usan.
    pub fn from_ce158(bytes: &[u8]) -> Result<(Self, Ce158Header, Vec<Warning>), BasicError> {
        if bytes.len() < CE158_HEADER_LEN {
            return Err(BasicError::BadCe158Header("menos de 27 bytes"));
        }
        if !bytes.starts_with(&CE158_MAGIC) {
            return Err(BasicError::BadCe158Header("no empieza por $01 \"@COM\""));
        }
        let word = |i: usize| u16::from(bytes[i]) << 8 | u16::from(bytes[i + 1]);
        let name_field = &bytes[5..5 + CE158_NAME_LEN];
        let name_len = name_field.iter().position(|&b| b == 0).unwrap_or(CE158_NAME_LEN);
        let header = Ce158Header {
            name: name_field[..name_len].to_vec(),
            start: word(21),
            size_minus_one: word(23),
            end: word(25),
        };
        let payload = &bytes[CE158_HEADER_LEN..];
        let declared = usize::from(header.size_minus_one) + 1;
        let mut warnings = Vec::new();
        if payload.len() < declared {
            warnings.push(Warning {
                line: None,
                kind: WarningKind::Ce158Truncated { declared, present: payload.len() },
            });
        }
        let (program, more) = Self::from_tokenized(&payload[..declared.min(payload.len())])
            .map_err(|e| match e {
                BasicError::BadTokenized { offset, reason } => {
                    BasicError::BadTokenized { offset: offset + CE158_HEADER_LEN, reason }
                }
                other => other,
            })?;
        warnings.extend(more);
        Ok((program, header, warnings))
    }

    /// El programa con la cabecera CE-158 que escribiría `CSAVE` en la
    /// máquina real cargado en `start` (ver [`Ce158Header`]): `name`
    /// truncado a 16 bytes (lo que no sea ASCII imprimible pasa a `_`),
    /// longitud − 1, `start + longitud` como dirección final y los bytes
    /// del programa sin el `$FF`.
    #[must_use]
    pub fn to_ce158(&self, name: &str, start: u16) -> Vec<u8> {
        let program = self.to_tokenized();
        let len = u16::try_from(program.len()).unwrap_or(u16::MAX);
        let mut out = Vec::with_capacity(CE158_HEADER_LEN + program.len());
        out.extend_from_slice(&CE158_MAGIC);
        let mut name_field = [0_u8; CE158_NAME_LEN];
        for (slot, ch) in name_field.iter_mut().zip(name.chars()) {
            *slot = if ch.is_ascii_graphic() || ch == ' ' { ch as u8 } else { b'_' };
        }
        out.extend_from_slice(&name_field);
        for word in [start, len.saturating_sub(1), start.wrapping_add(len)] {
            out.push((word >> 8) as u8);
            out.push((word & 0xFF) as u8);
        }
        out.extend_from_slice(&program);
        out
    }
}

/// Los 2 bytes de un token, en el orden en que van en el programa.
const fn token_bytes(token: u16) -> [u8; 2] {
    [(token >> 8) as u8, (token & 0xFF) as u8]
}

/// Divide en líneas por CRLF, CR o LF.
pub(crate) fn split_text_lines(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = Some(text);
    iter::from_fn(move || {
        let current = rest?;
        let Some(i) = current.find(['\r', '\n']) else {
            rest = None;
            return (!current.is_empty()).then_some(current);
        };
        let (line, eol) = current.split_at(i);
        rest = Some(eol.strip_prefix("\r\n").unwrap_or_else(|| eol.split_at(1).1));
        Some(line)
    })
}

/// Convierte el texto de una línea (tras el número) en los bytes que se
/// teclearían (ver [`Program::from_ascii`]). `column_base` es cuántos
/// caracteres de la línea de texto van antes, para los errores.
fn encode_text(text: &str, line: u16, column_base: usize) -> Result<Vec<u8>, BasicError> {
    let mut out = Vec::with_capacity(text.len());
    let mut chars = text.chars().enumerate();
    while let Some((column, ch)) = chars.next() {
        match ch {
            '\u{3c0}' => out.extend_from_slice(&token_bytes(TOKEN_PI)),
            '\u{221a}' => out.extend_from_slice(&token_bytes(TOKEN_SQR)),
            '\t' => out.push(b' '),
            '\\' => {
                let rest: String = text.chars().skip(column + 1).take(3).collect();
                let hex = rest.strip_prefix('x').filter(|h| h.len() == 2);
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        for _ in 0..3 {
                            chars.next();
                        }
                    }
                    None => out.push(b'\\'),
                }
            }
            ' '..='~' => out.push(ch as u8),
            _ => {
                return Err(BasicError::BadCharacter { line, column: column_base + column + 1, ch });
            }
        }
    }
    Ok(out)
}

/// Regla de [`AsciiOptions::list_format`] para `REM`: si tras el token
/// vienen dos espacios, se quita uno (el que pone `LIST` tras la palabra
/// clave).
fn collapse_rem_double_space(body: &mut Vec<u8>) {
    if let Some(rest) = rem_rest_offset(body)
        && body.get(rest..rest + 2) == Some(b"  ")
    {
        body.remove(rest);
    }
}

/// Posición del primer byte tras el token `REM` de un cuerpo tokenizado,
/// si lo hay (fuera de cadenas, como lo vería el intérprete).
fn rem_rest_offset(body: &[u8]) -> Option<usize> {
    let mut i = 0;
    let mut in_string = false;
    while let Some(&b) = body.get(i) {
        if !in_string && b >= 0xE0 {
            if body.get(i + 1) == Some(&((TOKEN_REM & 0xFF) as u8)) && u16::from(b) == TOKEN_REM >> 8 {
                return Some(i + 2);
            }
            i += 2;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
        }
        i += 1;
    }
    None
}

/// Añade a `out` el cuerpo `body` como lo lista la ROM (ver
/// [`Program::to_ascii`]).
fn detokenize_body(out: &mut String, body: &[u8], keywords: &Keywords, options: AsciiOptions) {
    let mut i = 0;
    let mut in_string = false;
    while let Some(&b) = body.get(i) {
        if b == b'"' {
            in_string = !in_string;
            out.push('"');
            i += 1;
            continue;
        }
        if !in_string
            && b >= 0xE0
            && let Some(&low) = body.get(i + 1)
        {
            let token = u16::from(b) << 8 | u16::from(low);
            i += 2;
            match keywords.name(token) {
                Some(name) => push_escaped(out, name),
                None => push_escaped(out, &[b, low]),
            }
            if token == TOKEN_REM {
                let rest = &body[i..];
                if options.list_format && rest.first() == Some(&b' ') {
                    out.push(' ');
                }
                push_escaped(out, rest);
                return;
            }
            out.push(' ');
            continue;
        }
        push_escaped(out, &[b]);
        i += 1;
    }
}

/// ASCII imprimible tal cual; lo demás (y `\`) como `\xNN`.
fn push_escaped(out: &mut String, bytes: &[u8]) {
    for &b in bytes {
        if (0x20..=0x7E).contains(&b) && b != b'\\' {
            out.push(char::from(b));
        } else {
            write!(out, "\\x{b:02X}").ok();
        }
    }
}

/// Texto de un archivo: UTF-8 si lo es (sin BOM); si no, Latin-1 (los
/// comentarios del corpus traen bytes sueltos que no son UTF-8).
#[must_use]
pub fn decode_text(bytes: &[u8]) -> Cow<'_, str> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    str::from_utf8(bytes).map_or_else(
        |_| Cow::Owned(bytes.iter().map(|&b| char::from(b)).collect()),
        Cow::Borrowed,
    )
}

// --- Formatos de archivo ----------------------------------------------------

/// Formato de un archivo de programa al leerlo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasicFormat {
    /// Listado de texto.
    Ascii,
    /// Bytes tokenizados tal como van en RAM (con o sin `$FF` final).
    Tokenized,
    /// Tokenizado con la cabecera de 27 bytes de la CE-158.
    Ce158,
}

/// Formato de un archivo de programa al escribirlo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFormat {
    /// Texto con CRLF (`.txt`, la convención del corpus).
    AsciiCrLf,
    /// Texto con CR al estilo de los `.ASC` de la CE-158(X) (`.asc`).
    AsciiCr,
    /// Bytes tokenizados sin cabecera ni `$FF` (`.bin`).
    Tokenized,
    /// Tokenizado con cabecera CE-158 (`.bas` y sin extensión: lo que
    /// hace `CSAVE "X"` en la CE-158).
    Ce158,
}

impl SaveFormat {
    /// El formato que toca por la extensión (sin el punto, da igual
    /// mayúsculas o minúsculas): `.asc` CR, `.txt`/`.sbs` CRLF,
    /// `.bin`/`.tok`/`.img` tokenizado y el resto (`.bas`, sin extensión)
    /// CE-158.
    #[must_use]
    pub fn for_extension(ext: Option<&str>) -> Self {
        match ext.map(str::to_ascii_lowercase).as_deref() {
            Some("asc") => Self::AsciiCr,
            Some("txt" | "sbs") => Self::AsciiCrLf,
            Some("bin" | "tok" | "img") => Self::Tokenized,
            _ => Self::Ce158,
        }
    }

    /// [`SaveFormat::for_extension`] con la extensión de `path`.
    #[must_use]
    pub fn for_path(path: &Path) -> Self {
        Self::for_extension(path.extension().and_then(|e| e.to_str()))
    }
}

/// Averigua el formato de un archivo de programa por su contenido (y la
/// extensión, sin punto):
/// - `$01 "@COM"` al principio es CE-158, tenga la extensión que tenga.
/// - `.bin`, `.tok` y `.img` son tokenizados.
/// - Si no, un archivo con bytes de control que no salen en un texto
///   (cualquiera por debajo de $20 salvo tabulador, CR, LF, FF y el
///   Ctrl-Z de fin de archivo) que se lee entero como programa tokenizado
///   es tokenizado: un programa tokenizado casi siempre los tiene (el
///   byte alto de los números de línea menores de 8192, las longitudes
///   cortas).
/// - Lo demás, texto.
#[must_use]
pub fn detect_format(ext: Option<&str>, bytes: &[u8]) -> BasicFormat {
    if bytes.len() >= CE158_HEADER_LEN && bytes.starts_with(&CE158_MAGIC) {
        return BasicFormat::Ce158;
    }
    if matches!(
        ext.map(str::to_ascii_lowercase).as_deref(),
        Some("bin" | "tok" | "img")
    ) {
        return BasicFormat::Tokenized;
    }
    let binary = bytes
        .iter()
        .any(|&b| b < 0x20 && !matches!(b, b'\t' | b'\r' | b'\n' | 0x0C | 0x1A));
    if binary && Program::from_tokenized(bytes).is_ok_and(|(p, _)| !p.lines().is_empty()) {
        return BasicFormat::Tokenized;
    }
    BasicFormat::Ascii
}

/// Un programa leído de un archivo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedProgram {
    pub program: Program,
    pub format: BasicFormat,
    pub warnings: Vec<Warning>,
}

/// Interpreta los bytes de un archivo de programa en `format` (ver
/// [`detect_format`]).
pub fn decode_program(
    bytes: &[u8],
    format: BasicFormat,
    keywords: &Keywords,
    options: &AsciiOptions,
) -> Result<DecodedProgram, BasicError> {
    let (program, warnings) = match format {
        BasicFormat::Ascii => Program::from_ascii(&decode_text(bytes), keywords, options)?,
        BasicFormat::Tokenized => Program::from_tokenized(bytes)?,
        BasicFormat::Ce158 => {
            let (program, _, warnings) = Program::from_ce158(bytes)?;
            (program, warnings)
        }
    };
    Ok(DecodedProgram { program, format, warnings })
}

/// Los bytes del archivo de `program` en `format`. `name` (el nombre de
/// la cabecera CE-158) y `start` (su dirección de inicio) solo cuentan en
/// [`SaveFormat::Ce158`].
#[must_use]
pub fn encode_program(
    program: &Program,
    format: SaveFormat,
    name: &str,
    start: u16,
    keywords: &Keywords,
) -> Vec<u8> {
    match format {
        SaveFormat::AsciiCrLf => program
            .to_ascii(Eol::CrLf, keywords, &AsciiOptions::default())
            .into_bytes(),
        SaveFormat::AsciiCr => program.to_ce158_ascii(keywords).into_bytes(),
        SaveFormat::Tokenized => program.to_tokenized(),
        SaveFormat::Ce158 => program.to_ce158(name, start),
    }
}

// --- La máquina emulada ---------------------------------------------------

/// Por qué no se pudo cargar un programa en la máquina.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BasicLoadError {
    /// La ROM no está esperando teclado en el prompt de BASIC (está
    /// ejecutando, en un `INPUT`, en la pausa de un `PRINT`, apagada...).
    /// No se ha tocado la memoria.
    NotAtPrompt,
    /// `RAM_ST_H`/`RAM_END_H` no describen una RAM (la ROM no ha
    /// arrancado todavía).
    NoRam,
    /// El programa (con su `$FF`) necesita `need` bytes y desde su inicio
    /// hasta el final de la RAM hay `avail`. No se ha tocado la memoria.
    TooBig { need: usize, avail: usize },
    /// El archivo no es un programa válido.
    Format(BasicError),
    /// No se pudo leer el archivo.
    Io(String),
}

impl fmt::Display for BasicLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAtPrompt => f.write_str(
                "la PC-1500 no est\u{e1} en el prompt BASIC (pulsa CL o BREAK)",
            ),
            Self::NoRam => f.write_str("la PC-1500 todav\u{ed}a no ha arrancado"),
            Self::TooBig { need, avail } => write!(
                f,
                "el programa necesita {need} bytes y solo hay {avail} libres"
            ),
            Self::Format(e) => e.fmt(f),
            Self::Io(e) => write!(f, "error de lectura: {e}"),
        }
    }
}

impl Error for BasicLoadError {}

impl From<BasicError> for BasicLoadError {
    fn from(e: BasicError) -> Self {
        Self::Format(e)
    }
}

/// Por qué no se pudo guardar el programa de la máquina.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BasicSaveError {
    /// El programa en RAM está corrupto.
    Program(BasicError),
    /// No se pudo escribir el archivo.
    Io(String),
}

impl fmt::Display for BasicSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program(e) => write!(f, "programa en memoria no v\u{e1}lido: {e}"),
            Self::Io(e) => write!(f, "error de escritura: {e}"),
        }
    }
}

impl Error for BasicSaveError {}

/// Lo que hizo [`Pc1500::load_basic_program`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadReport {
    /// `BASPRG_ST`: dónde empieza el programa.
    pub start: u16,
    /// `BASPRG_END`: la dirección de su `$FF`.
    pub end: u16,
    /// Bytes del programa, sin el `$FF`.
    pub bytes: usize,
    /// Líneas.
    pub lines: usize,
    /// Bytes libres entre el `$FF` y el final de la RAM.
    pub free: usize,
    /// Avisos al interpretar el archivo (vacío con
    /// [`Pc1500::load_basic_program`]).
    pub warnings: Vec<Warning>,
}

/// Lo que hizo [`Pc1500::save_basic_file`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveReport {
    pub format: SaveFormat,
    /// Bytes escritos en el archivo.
    pub bytes: usize,
    /// Líneas del programa.
    pub lines: usize,
}

/// Páginas de las tablas de expansión que recorre la ROM tras la
/// principal, con `PV`=0 y luego con `PV`=1 (`DEC_OPN` $FA58: pasos de 2
/// KB, $B8, $B0, ..., $80).
const EXPANSION_TABLE_PAGES: [u8; 8] = [0xB8, 0xB0, 0xA8, 0xA0, 0x98, 0x90, 0x88, 0x80];

impl Pc1500 {
    /// Las tablas de palabras clave de la máquina emulada, en el orden en
    /// que las busca el tokenizador de la ROM (`TOK_INBUF` $F9C3: la
    /// principal en $C000 y luego, con `DEC_OPN`, cada tabla con firma $55
    /// en $B800, $B000, ..., $8000 con `PV`=0 y otra vez con `PV`=1). Se
    /// leen de la memoria emulada, así que incluyen cualquier tabla de
    /// expansión que esté mapeada. Una tabla que no se pueda interpretar
    /// se salta.
    #[must_use]
    pub fn basic_keywords(&self) -> Keywords {
        let mut tables = Vec::new();
        let main = KeywordTable::parse_with(MAIN_TABLE_BASE, self.lh5801.pv(), |a| {
            self.read_byte(u32::from(a))
        })
        .unwrap_or_else(|_| KeywordTable::main_rom().clone());
        tables.push(main);
        for pv in [false, true] {
            for page in EXPANSION_TABLE_PAGES {
                let base = u16::from(page) << 8;
                if self.read_table_byte(base, pv) != TABLE_SIGNATURE {
                    continue;
                }
                if let Ok(table) = KeywordTable::parse_with(base, pv, |a| self.read_table_byte(a, pv)) {
                    tables.push(table);
                }
            }
        }
        Keywords::from_tables(tables)
    }

    /// Un byte de ME0 en $8000-$BFFF como lo vería la CPU con `pv`, sea
    /// cual sea el `PV` que tenga ahora (ver `Pc1500::read_banked`, en
    /// `memory.rs`: hoy solo está la tabla virtual de `CLOAD`/`CSAVE` en
    /// $9800 con `PV`=0). Fuera de esa zona `PV` no cambia nada.
    fn read_table_byte(&self, addr: u16, pv: bool) -> u8 {
        if (0x8000..0xC000).contains(&addr) {
            return self.read_banked(addr, pv);
        }
        self.read_byte(u32::from(addr))
    }

    pub(crate) fn read_word(&self, addr: u16) -> u16 {
        u16::from(self.read_byte(u32::from(addr))) << 8 | u16::from(self.read_byte(u32::from(addr) + 1))
    }

    pub(crate) fn write_word(&mut self, addr: u16, value: u16) {
        self.write_byte(u32::from(addr), (value >> 8) as u8);
        self.write_byte(u32::from(addr) + 1, (value & 0xFF) as u8);
    }

    /// Corrige el programa de arranque que `MemoryBus::new()` pone en
    /// $0100 (ver [`Pc1500::new`]) para que sea un programa BASIC válido,
    /// igual que si se hubiera cargado con [`Pc1500::load_basic_program`]:
    ///
    /// - Escribe el `$FF` de fin en `BASPRG_END` (el fixture no lo trae y
    ///   la memoria de usuario empieza a 0). Sin él, la ROM sigue
    ///   buscando líneas en los ceros que hay detrás: teclear `5645 A=2`
    ///   tras arrancar con `mole` metía la línea en un sitio absurdo y
    ///   `LIST` mostraba basura (`65535:~~ "MOLE"`).
    /// - Pone `VAR_START` = `RAM_END_H`:00 = $4800, lo que haría
    ///   `DEL_DIM_VAR` ($D091). El arranque en frío no lo hace (solo
    ///   `RUN`, `NEW` y `CLEAR`), así que quedaba en $FFFF y un `DIM` en
    ///   modo directo, o un `GOTO` sin `RUN`, usaba una base absurda.
    ///
    /// La pantalla de arranque (`NEW0? :CHECK`), `LIST` y `RUN` no
    /// cambian; en memoria cambian esos 3 bytes.
    pub(crate) fn fix_boot_fixture(&mut self) {
        let start = self.read_word(BASPRG_ST);
        let end = self.read_word(BASPRG_END);
        let user_end = memory::USER_MEMORY_END_EXCLUSIVE;
        if start <= end && end < user_end {
            self.write_byte(u32::from(end), END_MARK);
        }
        self.write_word(VAR_START, user_end & 0xFF00);
    }

    /// `true` si la ROM está en el prompt de BASIC esperando teclado, el
    /// único momento en que se puede cambiar el programa desde fuera: la
    /// CPU dormida en `WAIT_4_KB` ([`Pc1500::is_waiting_for_key`]) y
    /// [`BREAKPARAM`] sin los bits de "dentro de un programa" ($50 en un
    /// `INPUT`, $20 en la pausa de un `PRINT`). Tras un `STOP` o un BREAK
    /// ($80) sí cuenta como prompt.
    #[must_use]
    pub fn is_at_basic_prompt(&self) -> bool {
        self.is_waiting_for_key() && self.read_byte(u32::from(BREAKPARAM)) & BREAKPARAM_BUSY == 0
    }

    /// Carga `program` como programa BASIC de la máquina, sustituyendo al
    /// que hubiera, como lo dejaría teclearlo tras un `NEW` (validado con
    /// `LIST`, las flechas arriba/abajo, editar líneas, `RUN`, `CONT` y
    /// `MODE`):
    ///
    /// 1. Solo en el prompt ([`Pc1500::is_at_basic_prompt`]); si no,
    ///    [`BasicLoadError::NotAtPrompt`] sin tocar nada.
    /// 2. Empieza en el `BASPRG_ST` actual, o en `RAM_ST_H`:$C5 (lo que
    ///    pone `NEW0`) si ese no está dentro de la RAM.
    /// 3. Tiene que caber con su `$FF` antes de `RAM_END_H`:00
    ///    (`PRGLINE_TDI_6` $CF7F `CPA (RAM_END_H)`); si no,
    ///    [`BasicLoadError::TooBig`] sin tocar nada.
    /// 4. Escribe los bytes y el `$FF`, `BASPRG_ST` = `BASPRG_EDT` =
    ///    inicio y `BASPRG_END` = la dirección del `$FF` (como `NEW`,
    ///    $C853-$C85C).
    /// 5. `VAR_START` = `RAM_END_H`:00 (`DEL_DIM_VAR`, $D091).
    /// 6. La parte de `INIT_SYS_ADDR` ($CFCC-$D00C) que tiene sentido con
    ///    la ROM esperando teclado: `DATA_PTR` = (inicio + 3) | $8000;
    ///    $78A2/A3, $78B4/B5, `ERL` ($789B) y $7895-$7898 a 0;
    ///    `ON_ERR_ADD_H` ($78B8) |= $80; `CURS_CTRL` ($7B0E) &= $1F;
    ///    `BREAKPARAM` = 0 (así `CONT` da `ERROR 1` en vez de continuar
    ///    dentro del programa viejo); `GOSB_STK_PTR_L` ($7891) = $FF y
    ///    `FORNXT_STK_PTR` ($7890) = $38.
    ///
    /// NO se tocan $7871-$787E (el cursor y su parpadeo, en uso dentro de
    /// `WAIT_4_KB`), ni las direcciones de búsqueda de línea, ni
    /// $789C-$78B7 (llenarlas de $FF, como el resto de
    /// `INIT_SYS_ADDR`, rompía las teclas `MODE` y arriba), ni las
    /// variables fijas, `TRON` o `WAIT`. `ROM_ST` sigue en $FFFF.
    pub fn load_basic_program(&mut self, program: &Program) -> Result<LoadReport, BasicLoadError> {
        if !self.is_at_basic_prompt() {
            return Err(BasicLoadError::NotAtPrompt);
        }
        let (start, ram_end) = self.program_load_area()?;
        let bytes = program.to_tokenized();
        let need = bytes.len() + 1;
        let avail = usize::from(ram_end - start);
        if need > avail {
            return Err(BasicLoadError::TooBig { need, avail });
        }
        let end = self.write_program(start, &bytes);
        self.write_word(VAR_START, ram_end);
        self.init_sys_addr_subset(start);
        Ok(LoadReport {
            start,
            end,
            bytes: bytes.len(),
            lines: program.lines().len(),
            free: usize::from(ram_end - end) - 1,
            warnings: Vec::new(),
        })
    }

    /// Dónde empieza un programa que sustituya al actual y dónde acaba la
    /// RAM (`RAM_END_H`:00): los pasos 2 y 3 de
    /// [`Pc1500::load_basic_program`]. Lo usa también `CLOAD` (ver
    /// `host_io.rs`).
    pub(crate) fn program_load_area(&self) -> Result<(u16, u16), BasicLoadError> {
        let ram_st_h = self.read_byte(u32::from(RAM_ST_H));
        let ram_end_h = self.read_byte(u32::from(RAM_END_H));
        if ram_st_h == 0 || ram_end_h <= ram_st_h || ram_end_h == 0xFF {
            return Err(BasicLoadError::NoRam);
        }
        let ram_end = u16::from(ram_end_h) << 8;
        let current = self.read_word(BASPRG_ST);
        let start = if (ram_st_h..ram_end_h).contains(&((current >> 8) as u8)) {
            current
        } else {
            u16::from(ram_st_h) << 8 | u16::from(NEW0_START_LOW)
        };
        Ok((start, ram_end))
    }

    /// Escribe `bytes` (un programa sin `$FF`) en `start`, el `$FF` detrás
    /// y los punteros `BASPRG_ST`/`BASPRG_EDT`/`BASPRG_END`, como `NEW`
    /// ($C853-$C85C). Devuelve la dirección del `$FF`.
    pub(crate) fn write_program(&mut self, start: u16, bytes: &[u8]) -> u16 {
        for (addr, &byte) in (u32::from(start)..).zip(bytes) {
            self.write_byte(addr, byte);
        }
        let end = start.wrapping_add(u16::try_from(bytes.len()).unwrap_or(u16::MAX));
        self.write_byte(u32::from(end), END_MARK);
        self.write_word(BASPRG_ST, start);
        self.write_word(BASPRG_EDT, start);
        self.write_word(BASPRG_END, end);
        end
    }

    /// Ver el paso 6 de [`Pc1500::load_basic_program`].
    fn init_sys_addr_subset(&mut self, start: u16) {
        self.write_word(DATA_PTR, start.wrapping_add(3) | 0x8000);
        for addr in [0x78A2_u32, 0x78A3, 0x78B4, 0x78B5, 0x789B, 0x7895, 0x7896, 0x7897, 0x7898] {
            self.write_byte(addr, 0);
        }
        let on_err = self.read_byte(0x78B8);
        self.write_byte(0x78B8, on_err | 0x80);
        let curs_ctrl = self.read_byte(0x7B0E);
        self.write_byte(0x7B0E, curs_ctrl & 0x1F);
        self.write_byte(u32::from(BREAKPARAM), 0);
        self.write_byte(0x7891, 0xFF);
        self.write_byte(0x7890, 0x38);
    }

    /// Interpreta `bytes` (ver [`detect_format`], `ext` sin punto) con las
    /// palabras clave de la máquina ([`Pc1500::basic_keywords`]) y lo
    /// carga con [`Pc1500::load_basic_program`]. Los avisos van en el
    /// [`LoadReport`].
    pub fn load_basic_bytes(
        &mut self,
        bytes: &[u8],
        ext: Option<&str>,
        options: &AsciiOptions,
    ) -> Result<LoadReport, BasicLoadError> {
        let format = detect_format(ext, bytes);
        let decoded = decode_program(bytes, format, &self.basic_keywords(), options)?;
        let mut report = self.load_basic_program(&decoded.program)?;
        report.warnings = decoded.warnings;
        Ok(report)
    }

    /// [`Pc1500::load_basic_bytes`] de un archivo, con las opciones
    /// predeterminadas.
    pub fn load_basic_file(&mut self, path: &Path) -> Result<LoadReport, BasicLoadError> {
        let bytes = fs::read(path).map_err(|e| BasicLoadError::Io(e.to_string()))?;
        let ext = path.extension().and_then(|e| e.to_str());
        self.load_basic_bytes(&bytes, ext, &AsciiOptions::default())
    }

    /// Los bytes del programa BASIC de la máquina sin el `$FF`: las
    /// líneas desde `BASPRG_ST` hasta la que empiece por $FF, sin pasar
    /// de `BASPRG_END` (ni del final de la RAM si este no tiene sentido).
    #[must_use]
    pub fn basic_program_bytes(&self) -> Vec<u8> {
        let start = self.read_word(BASPRG_ST);
        let end = self.read_word(BASPRG_END);
        let ram_end = (u16::from(self.read_byte(u32::from(RAM_END_H))) << 8)
            .min(memory::USER_MEMORY_END_EXCLUSIVE);
        let limit = if start <= end && end <= ram_end { end } else { ram_end };
        let mut addr = start;
        while addr < limit && self.read_byte(u32::from(addr)) != END_MARK {
            let len = self.read_byte(u32::from(addr) + 2);
            let Some(next) = addr.checked_add(3 + u16::from(len)) else {
                break;
            };
            if next > limit {
                break;
            }
            addr = next;
        }
        self.read_bytes(u32::from(start), usize::from(addr.saturating_sub(start)))
    }

    /// El programa BASIC de la máquina (ver
    /// [`Pc1500::basic_program_bytes`]).
    pub fn basic_program(&self) -> Result<Program, BasicError> {
        Program::from_tokenized(&self.basic_program_bytes()).map(|(program, _)| program)
    }

    /// Guarda el programa BASIC de la máquina en `path`, en `format` o,
    /// si es `None`, en el que toque por la extensión
    /// ([`SaveFormat::for_path`]: `.bas` CE-158, `.asc` texto CR, `.txt`
    /// texto CRLF, `.bin` tokenizado). La cabecera CE-158 lleva el nombre
    /// del archivo y la dirección de inicio actual.
    pub fn save_basic_file(
        &self,
        path: &Path,
        format: Option<SaveFormat>,
    ) -> Result<SaveReport, BasicSaveError> {
        let program = self.basic_program().map_err(BasicSaveError::Program)?;
        let format = format.unwrap_or_else(|| SaveFormat::for_path(path));
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = encode_program(
            &program,
            format,
            &name,
            self.read_word(BASPRG_ST),
            &self.basic_keywords(),
        );
        fs::write(path, &bytes).map_err(|e| BasicSaveError::Io(e.to_string()))?;
        Ok(SaveReport { format, bytes: bytes.len(), lines: program.lines().len() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_lines_accepts_every_eol() {
        let lines: Vec<&str> = split_text_lines("a\r\nb\rc\nd").collect();
        assert_eq!(lines, ["a", "b", "c", "d"], "CRLF, CR y LF");
        let with_blank: Vec<&str> = split_text_lines("a\r\r\n").collect();
        assert_eq!(with_blank, ["a", ""], "l\u{ed}nea vac\u{ed}a intermedia");
    }

    #[test]
    fn escapes_round_trip() {
        let src = encode_text("A\\x0D\\xF0\\y", 10, 0).unwrap_or_default();
        assert_eq!(src, b"A\x0D\xF0\\y", "\\xNN y barra suelta");
        let mut out = String::new();
        push_escaped(&mut out, b"A\\\x01~");
        assert_eq!(out, "A\\x5C\\x01~", "escape de bytes no imprimibles");
    }

    #[test]
    fn save_format_by_extension() {
        assert_eq!(SaveFormat::for_extension(Some("BAS")), SaveFormat::Ce158, ".bas");
        assert_eq!(SaveFormat::for_extension(None), SaveFormat::Ce158, "sin extensi\u{f3}n");
        assert_eq!(SaveFormat::for_extension(Some("asc")), SaveFormat::AsciiCr, ".asc");
        assert_eq!(SaveFormat::for_extension(Some("txt")), SaveFormat::AsciiCrLf, ".txt");
        assert_eq!(SaveFormat::for_extension(Some("bin")), SaveFormat::Tokenized, ".bin");
    }
}
