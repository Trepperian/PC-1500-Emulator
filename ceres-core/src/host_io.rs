//! `CLOAD`, `CSAVE`, `MERGE` y `CHAIN` (y `CLOAD?`, y el sufijo `a` de
//! la CE-158) desde la propia PC-1500 emulada, con archivos del host.
//!
//! # Por qué hace falta una tabla de palabras clave
//! La ROM principal A04 no tiene ninguno de estos comandos: en la máquina
//! real los ponen la CE-150 (impresora/casete, tabla 01 `CMT` en $B800) o
//! la CE-158 (RS-232, tabla 08 `COM` en $8000 con `PV`=1). Sin ellas
//! `CLOAD` se queda en letras sueltas (`ERROR 1`) y un `F0 89` ya
//! tokenizado da `ERROR 27` al ejecutarlo (`TOK_PROCESS` $FA89, `LDI
//! UH,$1B` en $FAE4). Los tokens son los mismos en las dos: `CHAIN`
//! $F0B2, `CLOAD` $F089, `CSAVE` $F095, `MERGE` $F08F. Como son tokens
//! $F0xx, la ROM los busca en la tabla de `OPN` ($79D1, por defecto $60 =
//! $C000) y luego en cada tabla con firma $55 de $B800 a $8000 en pasos
//! de 2 KB, primero con `PV`=0 y luego con `PV`=1 (`DEC_OPN` $FA58,
//! `TOK_PROCESS_7..9` $FAC5-$FAE0; `TOK_TABL_SRCH` $E4A8 hace lo mismo
//! al arrancar: `CPI XH,$88` + `ADI A,$F7`, que suma el acarreo, da pasos
//! de 8 páginas: $B8, $B0, ..., $80). Así que basta con UNA tabla con esa
//! forma en cualquiera de esas direcciones para que el tokenizador, `LIST`
//! y el intérprete de la ROM real conozcan los cuatro comandos, sin
//! necesitar ninguna ROM de periférico.
//!
//! # Por qué en $9800 con `PV`=0
//! Es una zona que no usa ninguna expansión real (la CE-150 está en
//! `PV`=0 $A000-$BFFF y la CE-158 en `PV`=1 $8000-$9FFF). En $B800 la ROM
//! creería que hay una CE-150: `PRINT#`/`INPUT#` de casete saltan a
//! $B8A3 a ciegas ($E51E-$E524) y los vectores de sistema $FFA4-$FFB8
//! apuntan a $B888-$B8A0. En $9800 la tabla entra además en el recorrido
//! de `TOK_TABL_SRCH` del arranque, así que su vector de inicialización
//! (un `RTN`) se llama en cada reset (`RESET_19` $E10B-$E115).
//!
//! # La tabla ($9800-$9FFF, el resto a $FF)
//! ```text
//! +00 55                      firma ($FA84 CPI XL,$55; $E4B4 CPA (X))
//! +01 0E                      número de tabla (CE-150 01/02, CE-158 08/09)
//! +02 "HOST" 0D 0D 0D 0D      nombre (lo compara OPN "...", $E47D-$E489)
//! +0A 9A 9A 9A                vector de inicialización: RTN
//! +0D 68 1B E0                vector de INPUT#: ERROR 27 (como CE-150 $B80D)
//! +10 68 1B E0                vector de PRINT#: igual
//! +13..+1C 9A                 relleno
//! +1D C4 AF FF                vector de TRACE = TRCROUTINE $C4AF
//! +20..+53                    punteros por letra (C -> $9856, M -> $9874)
//! +54 95 "CHAIN" F0B2 9880    entradas [ctl][texto][token][vector]
//! +5E A5 "CLOAD" F089 9883
//! +68 85 "CSAVE" F095 9887
//! +72 D5 "MERGE" F08F 988A
//! +7C 90                      fin (nibble bajo 0)
//! +80 trampa CHAIN    +83 60 máscara de modo de CLOAD   +84 trampa CLOAD
//! +87 trampa CSAVE    +8A 60 máscara de modo de MERGE   +8B trampa MERGE
//! +90 CD 36           VMJ ($36) = STRING_2_CSI $DF0F (nombre -> CSI)
//! +92 trampa "nombre evaluado"
//! +98 BE D0 B4  BE D0 0D  E2   PRG_SEARCH, INIT_SYS_ADDR_6, BASIC_INT
//! +A0 A5 7B 0E FD C8 BE D0 0D FD 8A AE 7B 0E BA C8 C1
//!                             reanudar tras CHAIN, copiado byte a byte
//!                             de la CE-150 ($BA1D-$BA2C)
//! ```
//! Los atributos de cada entrada van en el nibble alto del `ctl` de la
//! SIGUIENTE (ver [`crate::basic::KeywordTable`]): bit 3 siempre, bit 4
//! "última de su letra", bits 6/5 "vale en modo directo/en programa"
//! (`PV_BANK_6` $FB7A-$FB9C). Si los dos últimos son 0, el comando es
//! solo directo y el primer byte de su rutina es una máscara de modo que
//! se hace AND con $764F (bit 6 RUN, 5 PRO, 4 RESERVE; `ERROR 26` si da
//! 0); se ejecuta desde el byte siguiente. Elegidos como en la CE-150 y
//! la CE-158: `CHAIN` $A (solo en programas), `CLOAD` $8 (solo directo,
//! RUN o PRO: `ERROR 1` dentro de un programa, como en la CE-158),
//! `CSAVE` $D (en todas partes, última de la C) y `MERGE` $9 (solo
//! directo, última de la M).
//!
//! # Trampas
//! Cada dirección "trampa" tiene en la tabla los bytes `68 1B E0` (`LDI
//! UH,$1B` / `VEJ (E0)`: `ERROR 27`, "periférico no conectado"). Cuando
//! la CPU va a ejecutar ahí con `PV`=0, `Pc1500::step_cpu` llama antes a
//! `Pc1500::host_trap`, que hace el trabajo en Rust y deja el PC en la
//! siguiente rutina de la ROM. Si la E/S del host está desactivada
//! (carpeta `None`, lo predeterminado en el núcleo), la trampa no actúa
//! y se ejecutan esos bytes: `ERROR 27`, lo mismo que en una PC-1500 sin
//! periférico.
//!
//! 1. Al entrar en `CLOAD`/`CSAVE`/`MERGE`/`CHAIN` (desde `PV_BANK_6`, con
//!    `Y` en el primer byte tras el token) se leen los modificadores: `a`
//!    (ASCII, como en la CE-158) y `?` (solo `CLOAD`: verificar), y se
//!    salta a `VMJ ($36)`, la rutina de la ROM que evalúa un literal o
//!    una variable de cadena y deja su descriptor ("CSI") en $7A04-$7A07.
//! 2. Al volver (`+92`), con el acarreo a 1 si había nombre, se lee el
//!    nombre, en `CHAIN` el `,línea` opcional, se exige `:` o fin de
//!    línea, y se hace la operación. Se sale por `BASIC_INT` ($C400, con
//!    `Y` en el terminador: el resto de la línea sigue ejecutándose), por
//!    `+98`/`+A0` tras cargar, o por `ERRN` ($CD8B) con el código en `UH`.
//!    La pila que dejó `PV_BANK_6` no hace falta: `BASIC_INT` y `ERRN`
//!    recargan `S`=$784F.
//!
//! # Archivos
//! Todo lo que es formato (tokenizar, detectar, la cabecera CE-158,
//! escribir el programa en RAM) es de [`crate::basic`]; aquí solo se
//! decide qué archivo y qué formato, como la CE-158:
//!
//! - `CSAVE "X"` escribe `X.BAS`, tokenizado con la cabecera de 27 bytes
//!   de la CE-158, y `CSAVEa "X"`, `X.ASC` en texto (CR, como los `.ASC`
//!   de la CE-158X). Con extensión explícita se usa tal cual; la
//!   extensión decide el formato solo si es `.bin` (tokenizado sin
//!   cabecera) o `.txt` (texto CRLF); con cualquier otra manda la regla
//!   de la CE-158 (con `a` texto: CR si es `.asc`, CRLF si no; sin `a`
//!   tokenizado con cabecera). Ver [`csave_format`].
//! - `CLOAD` reconoce solo el formato (texto, tokenizado o cabecera
//!   CE-158), con `a` o sin ella, y busca el nombre sin distinguir
//!   mayúsculas: tal cual si trae extensión; si no, `.BAS`, `.ASC`,
//!   `.TXT`, `.BIN` (con `a`, `.ASC` primero) y el nombre a secas.
//! - Sin nombre (`CSAVE`, `CLOAD`) se usa el archivo "cinta" elegido en la
//!   GUI ([`Pc1500::set_tape`]); sin cinta, `ERROR 40`.
//! - Solo nombres seguros dentro de la carpeta: ASCII imprimible sin `/ \
//!   : * ? " < > |`, que no empiecen por `.`, sin `..` y sin `.` o
//!   espacio al final; si no, `ERROR 40`.
//!
//! # Errores (los de la CE-150/CE-158)
//! `1` sintaxis, `11` línea de `CHAIN` que no existe, `26` `CSAVE` en modo
//! RESERVE, `27` E/S del host desactivada, `40` nombre no válido o sin
//! cinta, `42` no cabe, `43` `CLOAD?` no coincide, `44` error de lectura o
//! escritura o archivo tokenizado corrupto, `61` no existe el archivo,
//! `67` línea de texto sin número o que no se puede tokenizar.

use core::{fmt, iter, mem};
use std::{
    ffi::{OsStr, OsString},
    fs, io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::{
    Pc1500,
    basic::{
        self, AsciiOptions, BASPRG_END, BASPRG_ST, BasicError, BasicFormat, MAX_LINE_NUMBER,
        SaveFormat, VAR_START, Warning,
    },
};

// --- Mapa de la tabla -------------------------------------------------------

/// Primera dirección (ME0) de la tabla virtual, para el `match` de
/// `Pc1500::read_byte`.
pub(crate) const TABLE_BEGIN: u32 = 0x9800;
/// Última dirección (ME0) de la tabla virtual.
pub(crate) const TABLE_END: u32 = 0x9FFF;
/// Lo mismo, para comparar con el PC en `Pc1500::step_cpu`: `PC & MASK ==
/// PAGE` es "dentro de $9800-$9FFF".
pub(crate) const TRAP_PAGE: u16 = 0x9800;
pub(crate) const TRAP_PAGE_MASK: u16 = 0xF800;
/// Estados de CPU que se cobran por cada trampa que actúa. La operación
/// es instantánea (un archivo de 1 KB por la CE-158 real a 1200 baudios
/// tardaría unos 10 s): esto solo evita que una trampa no haga avanzar el
/// reloj de la CPU, del que dependen el temporizador y los periféricos.
pub(crate) const TRAP_STATES: u8 = 200;

/// Dirección de la tabla.
pub const HOST_TABLE_BASE: u16 = 0x9800;
/// Tamaño de la ventana de la tabla: 2 KB, como cada tabla de expansión.
const TABLE_SIZE: usize = 0x800;

/// Token de `CHAIN` (CE-150 $B854, CE-158).
pub const TOKEN_CHAIN: u16 = 0xF0B2;
/// Token de `CLOAD`.
pub const TOKEN_CLOAD: u16 = 0xF089;
/// Token de `CSAVE`.
pub const TOKEN_CSAVE: u16 = 0xF095;
/// Token de `MERGE`.
pub const TOKEN_MERGE: u16 = 0xF08F;

/// Trampa de `CHAIN` (su vector apunta aquí).
const H_CHAIN: u16 = HOST_TABLE_BASE + 0x80;
/// Máscara de modo de `CLOAD` (su vector apunta aquí; la rutina empieza
/// en el byte siguiente, ver la documentación del módulo).
const CLOAD_MASK: u16 = HOST_TABLE_BASE + 0x83;
/// Trampa de `CLOAD`.
const H_CLOAD: u16 = HOST_TABLE_BASE + 0x84;
/// Trampa de `CSAVE`.
const H_CSAVE: u16 = HOST_TABLE_BASE + 0x87;
/// Máscara de modo de `MERGE`.
const MERGE_MASK: u16 = HOST_TABLE_BASE + 0x8A;
/// Trampa de `MERGE`.
const H_MERGE: u16 = HOST_TABLE_BASE + 0x8B;
/// `VMJ ($36)`: evaluar el nombre.
const EVAL: u16 = HOST_TABLE_BASE + 0x90;
/// Trampa a la vuelta de `VMJ ($36)`.
const EVAL_DONE: u16 = HOST_TABLE_BASE + 0x92;
/// Tras `CLOAD`/`MERGE`: `PRG_SEARCH`, `INIT_SYS_ADDR_6` (lo que hace
/// `NEW`, $C85C) y `BASIC_INT`.
const CONT_LOADED: u16 = HOST_TABLE_BASE + 0x98;
/// Tras `CHAIN`: reanudar la ejecución en el programa nuevo.
const CHAIN_GO: u16 = HOST_TABLE_BASE + 0xA0;

/// `BASIC_INT` ($C400, vector `VEJ (E2)`): seguir interpretando desde `Y`
/// (que debe apuntar a `:` o al fin de línea).
const BASIC_INT: u16 = 0xC400;
/// `ERRN` ($CD8B, vector `VEJ (E0)`): `ERROR` con el código de `UH`.
const ERRN: u16 = 0xCD8B;

/// Bytes de reserva de cada trampa: `LDI UH,$1B` / `VEJ (E0)` = `ERROR 27`.
const ERROR_27: [u8; 3] = [0x68, 0x1B, 0xE0];
/// `RTN`.
const RTN: u8 = 0x9A;
/// Máscara de modo de los comandos solo directos: RUN | PRO.
const MODE_MASK_RUN_PRO: u8 = 0x60;

/// Una entrada de la tabla: texto, token, vector y atributos (nibble).
struct TableEntry {
    text: &'static [u8],
    token: u16,
    vector: u16,
    attributes: u8,
}

/// Las entradas, en el orden de la tabla (alfabético dentro de cada
/// letra, como la ROM las recorre para tokenizar y para `LIST`).
const ENTRIES: [TableEntry; 4] = [
    TableEntry { text: b"CHAIN", token: TOKEN_CHAIN, vector: H_CHAIN, attributes: 0xA },
    TableEntry { text: b"CLOAD", token: TOKEN_CLOAD, vector: CLOAD_MASK, attributes: 0x8 },
    TableEntry { text: b"CSAVE", token: TOKEN_CSAVE, vector: H_CSAVE, attributes: 0xD },
    TableEntry { text: b"MERGE", token: TOKEN_MERGE, vector: MERGE_MASK, attributes: 0x9 },
];

/// Nibble alto del primer `ctl`, que no son los atributos de nadie: $9,
/// como el de la tabla de la CE-150 ($B854 = $95).
const FIRST_CTL_HIGH: u8 = 0x9;

/// Construye la imagen de la tabla (ver la documentación del módulo).
fn build_rom() -> [u8; TABLE_SIZE] {
    let mut rom = [0xFF; TABLE_SIZE];
    let offset = |addr: u16| usize::from(addr - HOST_TABLE_BASE);
    rom[0x00] = 0x55;
    rom[0x01] = 0x0E;
    rom[0x02..0x0A].copy_from_slice(b"HOST\r\r\r\r");
    rom[0x0A..0x0D].fill(RTN);
    rom[0x0D..0x10].copy_from_slice(&ERROR_27);
    rom[0x10..0x13].copy_from_slice(&ERROR_27);
    rom[0x13..0x1D].fill(RTN);
    rom[0x1D..0x20].copy_from_slice(&[0xC4, 0xAF, 0xFF]);
    rom[0x20..0x54].fill(0);

    let mut at = 0x54_usize;
    let mut previous = FIRST_CTL_HIGH;
    for entry in &ENTRIES {
        let letter = usize::from(entry.text[0] - b'A');
        let pointer = 0x20 + 2 * letter;
        if rom[pointer] == 0 {
            // Al SEGUNDO carácter de la primera palabra de la letra.
            let second = HOST_TABLE_BASE + u16::try_from(at + 2).unwrap_or_default();
            rom[pointer..pointer + 2].copy_from_slice(&word_bytes(second));
        }
        let len = u8::try_from(entry.text.len()).unwrap_or_default();
        rom[at] = previous << 4 | len;
        at += 1;
        rom[at..at + entry.text.len()].copy_from_slice(entry.text);
        at += entry.text.len();
        rom[at..at + 2].copy_from_slice(&word_bytes(entry.token));
        rom[at + 2..at + 4].copy_from_slice(&word_bytes(entry.vector));
        at += 4;
        previous = entry.attributes;
    }
    rom[at] = previous << 4;

    for trap in [H_CHAIN, H_CLOAD, H_CSAVE, H_MERGE, EVAL_DONE] {
        rom[offset(trap)..offset(trap) + 3].copy_from_slice(&ERROR_27);
    }
    rom[offset(CLOAD_MASK)] = MODE_MASK_RUN_PRO;
    rom[offset(MERGE_MASK)] = MODE_MASK_RUN_PRO;
    rom[offset(EVAL)..offset(EVAL) + 2].copy_from_slice(&[0xCD, 0x36]);
    rom[offset(CONT_LOADED)..offset(CONT_LOADED) + 7]
        .copy_from_slice(&[0xBE, 0xD0, 0xB4, 0xBE, 0xD0, 0x0D, 0xE2]);
    rom[offset(CHAIN_GO)..offset(CHAIN_GO) + 16].copy_from_slice(&[
        0xA5, 0x7B, 0x0E, 0xFD, 0xC8, 0xBE, 0xD0, 0x0D, 0xFD, 0x8A, 0xAE, 0x7B, 0x0E, 0xBA, 0xC8,
        0xC1,
    ]);
    rom
}

/// La imagen de la tabla, construida una sola vez.
fn host_rom() -> &'static [u8; TABLE_SIZE] {
    static ROM: OnceLock<[u8; TABLE_SIZE]> = OnceLock::new();
    ROM.get_or_init(build_rom)
}

/// Un byte de la tabla ($9800-$9FFF; fuera, $FF).
pub(crate) fn rom_byte(addr: u32) -> u8 {
    addr.checked_sub(TABLE_BEGIN)
        .and_then(|i| host_rom().get(usize::try_from(i).ok()?).copied())
        .unwrap_or(0xFF)
}

/// Los 2 bytes de una palabra, big-endian.
const fn word_bytes(word: u16) -> [u8; 2] {
    [(word >> 8) as u8, (word & 0xFF) as u8]
}

// --- Direcciones de la RAM del sistema ------------------------------------

/// Descriptor ("CSI") del último resultado de cadena: $7A04 = $D0 si es
/// una cadena, $7A05/06 su dirección y $7A07 su longitud
/// (`CREATE_CSI_4` $DEAF).
const CSI_TYPE: u32 = 0x7A04;
const CSI_ADDR: u32 = 0x7A05;
const CSI_LEN: u32 = 0x7A07;
const CSI_STRING: u8 = 0xD0;
/// Indicadores de modo del LCD: bit 4 = RESERVE.
const MODE_SYMBOLS: u32 = 0x764F;
const MODE_RESERVE: u8 = 0x10;
/// `SRCH_ADD` ($78A6/A7), `SRCH_LINE` ($78A8/A9) y `SRCH_TOP` ($78AA/AB),
/// lo que deja `PRG_SEARCH` ($D0B4-$D0CD) y lee `RUN`.
const SRCH_ADD: u16 = 0x78A6;
const SRCH_LINE: u16 = 0x78A8;
const SRCH_TOP: u16 = 0x78AA;

const CR: u8 = 0x0D;
const COLON: u8 = b':';
const END_MARK: u8 = 0xFF;

// --- Errores ----------------------------------------------------------------

const ERR_SYNTAX: u8 = 1;
const ERR_LINE_NOT_FOUND: u8 = 11;
const ERR_RESERVE_MODE: u8 = 26;
const ERR_NO_DEVICE: u8 = 27;
const ERR_BAD_NAME: u8 = 40;
const ERR_TOO_LARGE: u8 = 42;
const ERR_VERIFY: u8 = 43;
const ERR_IO: u8 = 44;
const ERR_NOT_FOUND: u8 = 61;
const ERR_ASCII_LINE: u8 = 67;

/// Un `ERROR n` de la PC-1500 y su explicación para el host.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HostError {
    code: u8,
    message: String,
}

impl HostError {
    const fn new(code: u8, message: String) -> Self {
        Self { code, message }
    }
}

// --- Estado y eventos ---------------------------------------------------------

/// Qué comando de cinta se ejecutó.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostCommand {
    Cload,
    /// `CLOAD?`: comparar el archivo con el programa de la máquina.
    CloadVerify,
    Csave,
    Merge,
    Chain,
}

impl fmt::Display for HostCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cload => "CLOAD",
            Self::CloadVerify => "CLOAD?",
            Self::Csave => "CSAVE",
            Self::Merge => "MERGE",
            Self::Chain => "CHAIN",
        })
    }
}

/// Lo que ha hecho un comando de cinta, para la GUI
/// ([`Pc1500::take_host_events`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostEvent {
    /// `CSAVE`: archivo escrito.
    Saved { path: PathBuf, format: SaveFormat, bytes: usize, lines: usize },
    /// `CLOAD`, `MERGE` o `CHAIN`: programa cargado.
    Loaded {
        command: HostCommand,
        path: PathBuf,
        format: BasicFormat,
        /// Bytes del programa cargado (sin el `$FF`).
        bytes: usize,
        lines: usize,
        warnings: Vec<Warning>,
    },
    /// `CLOAD?`: el archivo coincide con la memoria.
    Verified { path: PathBuf, bytes: usize },
    /// El comando terminó en `ERROR code` en la PC-1500.
    Error { command: HostCommand, code: u8, message: String },
}

impl HostEvent {
    /// El código de `ERROR` de la PC-1500, si el comando falló.
    #[must_use]
    pub const fn error_code(&self) -> Option<u8> {
        match self {
            Self::Error { code, .. } => Some(*code),
            _ => None,
        }
    }
}

const fn save_format_label(format: SaveFormat) -> &'static str {
    match format {
        SaveFormat::Ce158 => "tokenizado con cabecera CE-158",
        SaveFormat::Tokenized => "tokenizado",
        SaveFormat::AsciiCr => "texto CR",
        SaveFormat::AsciiCrLf => "texto CRLF",
    }
}

const fn basic_format_label(format: BasicFormat) -> &'static str {
    match format {
        BasicFormat::Ce158 => "tokenizado con cabecera CE-158",
        BasicFormat::Tokenized => "tokenizado",
        BasicFormat::Ascii => "texto",
    }
}

impl fmt::Display for HostEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Saved { path, format, bytes, lines } => write!(
                f,
                "CSAVE \u{2192} {} ({}, {lines} l\u{ed}neas, {bytes} bytes)",
                path.display(),
                save_format_label(*format)
            ),
            Self::Loaded { command, path, format, bytes, lines, warnings } => {
                write!(
                    f,
                    "{command} \u{2190} {} ({}, {lines} l\u{ed}neas, {bytes} bytes)",
                    path.display(),
                    basic_format_label(*format)
                )?;
                if let Some(first) = warnings.first() {
                    write!(f, "; {} avisos: {first}", warnings.len())?;
                }
                Ok(())
            }
            Self::Verified { path, bytes } => {
                write!(f, "CLOAD? {}: coincide ({bytes} bytes)", path.display())
            }
            Self::Error { command, code, message } => {
                write!(f, "{command}: ERROR {code} \u{2014} {message}")
            }
        }
    }
}

/// Un comando a medias entre la primera trampa y la segunda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    command: HostCommand,
    /// Sufijo `a`.
    ascii: bool,
}

/// Configuración y estado de la E/S de archivos del host.
#[derive(Debug, Default)]
pub(crate) struct HostIo {
    /// Carpeta de los archivos; `None` = desactivado (`ERROR 27`).
    dir: Option<PathBuf>,
    /// Archivo "cinta" para `CLOAD`/`CSAVE` sin nombre.
    tape: Option<PathBuf>,
    pending: Option<Pending>,
    events: Vec<HostEvent>,
}

impl HostIo {
    /// Para `Pc1500::reset`: la configuración (carpeta y cinta) y los
    /// eventos que la GUI todavía no ha recogido sobreviven; un comando a
    /// medias no (la ROM arranca de cero).
    pub(crate) fn keep_config_from(&mut self, old: Self) {
        self.dir = old.dir;
        self.tape = old.tape;
        self.events = old.events;
        self.pending = None;
    }
}

// --- Nombres y formatos (funciones puras) -----------------------------------

/// Caracteres que no puede tener un nombre (separadores y los que no
/// valen en Windows).
const FORBIDDEN_NAME_CHARS: &[u8] = b"/\\:*?\"<>|";

/// El nombre tal como lo dio el programa, comprobado (ver la
/// documentación del módulo): `ERROR 40` si no es seguro.
fn validate_name(raw: &[u8]) -> Result<String, HostError> {
    let bad = |why: &str| {
        HostError::new(
            ERR_BAD_NAME,
            format!("nombre no v\u{e1}lido \"{}\": {why}", String::from_utf8_lossy(raw)),
        )
    };
    if raw.iter().any(|b| !(0x20..=0x7E).contains(b)) {
        return Err(bad("solo ASCII imprimible"));
    }
    if raw.iter().any(|b| FORBIDDEN_NAME_CHARS.contains(b)) {
        return Err(bad("no puede llevar / \\ : * ? \" < > |"));
    }
    if raw.first() == Some(&b'.') {
        return Err(bad("no puede empezar por '.'"));
    }
    if raw.windows(2).any(|w| w == b"..") {
        return Err(bad("no puede llevar '..'"));
    }
    if matches!(raw.last(), Some(b'.' | b' ')) {
        return Err(bad("no puede acabar en '.' ni en espacio"));
    }
    if raw.iter().all(|&b| b == b' ') {
        return Err(bad("vac\u{ed}o"));
    }
    Ok(raw.iter().map(|&b| char::from(b)).collect())
}

/// La extensión de un nombre de archivo, si tiene.
fn extension(file_name: &str) -> Option<&str> {
    Path::new(file_name).extension().and_then(OsStr::to_str)
}

/// El nombre de archivo de `CSAVE "name"`: con extensión, tal cual; sin
/// ella, `.ASC` con el sufijo `a` y `.BAS` sin él (como la CE-158).
fn csave_file_name(name: &str, ascii: bool) -> String {
    if extension(name).is_some() {
        name.to_owned()
    } else {
        format!("{name}.{}", if ascii { "ASC" } else { "BAS" })
    }
}

/// El formato de `CSAVE` a `file_name`.
///
/// Ver la documentación del módulo: `.bin` tokenizado sin cabecera y `.txt` texto CRLF pase lo que pase;
/// si no, con `a` texto (CR en `.asc`, CRLF en lo demás) y sin `a`
/// tokenizado con la cabecera CE-158, como la CE-158 real.
#[must_use]
pub fn csave_format(file_name: &str, ascii: bool) -> SaveFormat {
    match extension(file_name).map(str::to_ascii_lowercase).as_deref() {
        Some("bin") => SaveFormat::Tokenized,
        Some("txt") => SaveFormat::AsciiCrLf,
        Some("asc") if ascii => SaveFormat::AsciiCr,
        _ if ascii => SaveFormat::AsciiCrLf,
        _ => SaveFormat::Ce158,
    }
}

/// Los nombres que prueba `CLOAD "name"`, en orden: tal cual si trae
/// extensión; si no, con `.BAS`, `.ASC`, `.TXT` y `.BIN` (`.ASC` primero
/// con el sufijo `a`) y por último a secas.
fn cload_candidates(name: &str, ascii: bool) -> Vec<String> {
    if extension(name).is_some() {
        return vec![name.to_owned()];
    }
    let extensions: [&str; 4] =
        if ascii { ["ASC", "BAS", "TXT", "BIN"] } else { ["BAS", "ASC", "TXT", "BIN"] };
    extensions
        .iter()
        .map(|ext| format!("{name}.{ext}"))
        .chain(iter::once(name.to_owned()))
        .collect()
}

/// El archivo `file_name` de `dir` sin distinguir mayúsculas: el nombre
/// exacto si existe, si no el primero (en orden alfabético) que coincida.
fn find_case_insensitive(dir: &Path, file_name: &str) -> io::Result<Option<PathBuf>> {
    let exact = dir.join(file_name);
    if exact.is_file() {
        return Ok(Some(exact));
    }
    let mut matches: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(file_name))
        })
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    matches.sort();
    Ok(matches.into_iter().next())
}

/// `ERROR 67` si el texto tiene una línea que no es de programa: vacías,
/// solo espacios o caracteres de control (el Ctrl-Z de fin de archivo) y
/// comentarios `'` (las cabeceras del corpus) se saltan; las demás deben
/// empezar por un número, como las que se teclean en la máquina.
fn check_ascii_lines(text: &str) -> Result<(), HostError> {
    for (index, line) in basic::split_text_lines(text).enumerate() {
        let trimmed = line.trim_start_matches([' ', '\t']);
        if trimmed.chars().all(|c| c.is_whitespace() || c.is_control())
            || trimmed.starts_with('\'')
            || trimmed.starts_with(|c: char| c.is_ascii_digit())
        {
            continue;
        }
        let shown: String = trimmed.chars().take(24).collect();
        return Err(HostError::new(
            ERR_ASCII_LINE,
            format!("l\u{ed}nea {} del texto sin n\u{fa}mero: \"{shown}\"", index + 1),
        ));
    }
    Ok(())
}

/// El `ERROR` que toca a un archivo que no se pudo interpretar: `67` en
/// texto (línea mala), `44` en tokenizado (datos corruptos).
fn decode_error(format: BasicFormat, error: &BasicError) -> HostError {
    let code = if format == BasicFormat::Ascii { ERR_ASCII_LINE } else { ERR_IO };
    HostError::new(code, error.to_string())
}

fn io_error(what: &str, path: &Path, error: &io::Error) -> HostError {
    HostError::new(ERR_IO, format!("{what} {}: {error}", path.display()))
}

/// Escribe `bytes` en `path` sin dejar nunca un archivo a medias: primero
/// en un temporal oculto de la misma carpeta y luego `rename`, que
/// sustituye al anterior de una vez.
fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let Some(file_name) = path.file_name() else {
        return Err(io::Error::other("ruta sin nombre de archivo"));
    };
    let mut tmp_name = OsString::from(".");
    tmp_name.push(file_name);
    tmp_name.push(".ceres-tmp");
    let tmp = parent.join(tmp_name);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        fs::remove_file(&tmp).ok();
    })
}

// --- La máquina ---------------------------------------------------------------

/// Adónde va un `CSAVE`.
struct SaveTarget {
    path: PathBuf,
    format: SaveFormat,
    /// El nombre de la cabecera CE-158.
    header_name: String,
}

/// Qué hace la carga.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadKind {
    /// `CLOAD`: sustituir el programa.
    Replace,
    /// `CLOAD?`: solo comparar.
    Verify,
    /// `MERGE`: añadir tras el programa actual.
    Merge,
    /// `CHAIN`: sustituir y seguir ejecutando (en la línea dada o en la
    /// primera).
    Chain(Option<u16>),
}

impl Pc1500 {
    /// Activa la E/S de archivos del host con `dir` como carpeta de
    /// `CLOAD`/`CSAVE`/`MERGE`/`CHAIN`, o la desactiva con `None` (lo
    /// predeterminado en [`Pc1500::new`], para que los tests y cualquier
    /// uso sin GUI no toquen el disco): entonces esos comandos dan
    /// `ERROR 27`, como una PC-1500 sin periférico. La GUI usa la carpeta
    /// actual. Sobrevive a [`Pc1500::reset`].
    pub fn set_host_dir(&mut self, dir: Option<PathBuf>) {
        self.host_io.dir = dir;
    }

    /// La carpeta de [`Pc1500::set_host_dir`].
    #[must_use]
    pub fn host_dir(&self) -> Option<&Path> {
        self.host_io.dir.as_deref()
    }

    /// El archivo "cinta" que usan `CLOAD` y `CSAVE` sin nombre (ruta
    /// completa, no tiene por qué estar en la carpeta), o `None` (entonces
    /// sin nombre dan `ERROR 40`). Sobrevive a [`Pc1500::reset`].
    pub fn set_tape(&mut self, tape: Option<PathBuf>) {
        self.host_io.tape = tape;
    }

    /// El archivo de [`Pc1500::set_tape`].
    #[must_use]
    pub fn tape(&self) -> Option<&Path> {
        self.host_io.tape.as_deref()
    }

    /// Lo que han hecho los comandos de cinta desde la última llamada, en
    /// orden (para la línea de estado de la GUI).
    pub fn take_host_events(&mut self) -> Vec<HostEvent> {
        mem::take(&mut self.host_io.events)
    }

    /// Trampa de `Pc1500::step_cpu`: el PC está en $9800-$9FFF con
    /// `PV`=0. Devuelve `true` si ha hecho el trabajo (y movido el PC), o
    /// `false` para que se ejecute la instrucción que hay ahí.
    pub(crate) fn host_trap(&mut self) -> bool {
        match self.lh5801.p() {
            H_CHAIN => self.trap_command(HostCommand::Chain),
            H_CLOAD => self.trap_command(HostCommand::Cload),
            H_CSAVE => self.trap_command(HostCommand::Csave),
            H_MERGE => self.trap_command(HostCommand::Merge),
            EVAL_DONE => self.trap_eval_done(),
            _ => false,
        }
    }

    fn set_x(&mut self, value: u16) {
        let [hi, lo] = word_bytes(value);
        self.lh5801.set_xh(hi);
        self.lh5801.set_xl(lo);
    }

    fn set_y(&mut self, value: u16) {
        let [hi, lo] = word_bytes(value);
        self.lh5801.set_yh(hi);
        self.lh5801.set_yl(lo);
    }

    fn byte_at(&self, addr: u16) -> u8 {
        self.read_byte(u32::from(addr))
    }

    /// Salta a `ERRN` con `code` en `UH`: la ROM muestra `ERROR code` (o
    /// `ERROR code IN línea`) y apunta `ERL` como con cualquier error.
    fn jump_to_error(&mut self, code: u8) {
        self.lh5801.set_uh(code);
        self.lh5801.set_pc(ERRN);
    }

    fn raise(&mut self, command: HostCommand, error: HostError) {
        self.jump_to_error(error.code);
        self.host_io.events.push(HostEvent::Error {
            command,
            code: error.code,
            message: error.message,
        });
    }

    /// Primera trampa: modificadores (`a`, `?`) y a evaluar el nombre.
    fn trap_command(&mut self, mut command: HostCommand) -> bool {
        if self.host_io.dir.is_none() {
            return false;
        }
        let mut y = self.lh5801.y();
        let mut ascii = false;
        loop {
            match self.byte_at(y) {
                b'a' if !ascii => ascii = true,
                b'?' if command == HostCommand::Cload => command = HostCommand::CloadVerify,
                // `CSAVE M "X";inicio,fin` y `CLOAD M`: código máquina en
                // la CE-150, no soportado.
                b'-' => {
                    self.raise(command, HostError::new(ERR_SYNTAX, "forma no soportada".to_owned()));
                    return true;
                }
                b'M' if self.byte_at(y.wrapping_add(1)) == b'"' => {
                    self.raise(
                        command,
                        HostError::new(ERR_SYNTAX, "c\u{f3}digo m\u{e1}quina (M) no soportado".to_owned()),
                    );
                    return true;
                }
                _ => break,
            }
            y = y.wrapping_add(1);
        }
        self.set_y(y);
        self.host_io.pending = Some(Pending { command, ascii });
        self.lh5801.set_pc(EVAL);
        true
    }

    /// Lo que dejó `VMJ ($36)`: el nombre si había una cadena (acarreo a
    /// 1 y CSI de cadena); si no, `Y` vuelve a donde estaba (`X`, como
    /// `HDR_PREP_2` $BDBE de la CE-150) y no hay nombre.
    fn evaluated_name(&mut self) -> Option<Vec<u8>> {
        if self.lh5801.t() & 0x01 != 0 && self.read_byte(CSI_TYPE) == CSI_STRING {
            let addr = self.read_word_be(CSI_ADDR);
            let len = usize::from(self.read_byte(CSI_LEN));
            return Some(self.read_bytes(u32::from(addr), len));
        }
        let x = self.lh5801.x();
        self.set_y(x);
        None
    }

    /// Segunda trampa: nombre evaluado; hacer la operación.
    fn trap_eval_done(&mut self) -> bool {
        let Some(pending) = self.host_io.pending.take() else {
            if self.host_io.dir.is_none() {
                return false;
            }
            // Alguien ha llegado aquí sin pasar por la primera trampa (un
            // `CALL` a mano).
            self.jump_to_error(ERR_SYNTAX);
            return true;
        };
        let command = pending.command;
        let name = self.evaluated_name();
        let mut y = self.lh5801.y();
        let mut line = None;
        if command == HostCommand::Chain && self.byte_at(y) == b',' {
            y = y.wrapping_add(1);
            let mut number: u32 = 0;
            let mut digits = 0;
            while let digit @ b'0'..=b'9' = self.byte_at(y) {
                number = number * 10 + u32::from(digit - b'0');
                digits += 1;
                y = y.wrapping_add(1);
                if number > u32::from(MAX_LINE_NUMBER) {
                    break;
                }
            }
            match u16::try_from(number) {
                Ok(n) if digits > 0 && (1..=MAX_LINE_NUMBER).contains(&n) => line = Some(n),
                _ => {
                    self.raise(command, HostError::new(ERR_SYNTAX, "n\u{fa}mero de l\u{ed}nea no v\u{e1}lido".to_owned()));
                    return true;
                }
            }
        }
        if !matches!(self.byte_at(y), COLON | CR) {
            self.raise(command, HostError::new(ERR_SYNTAX, "sintaxis".to_owned()));
            return true;
        }
        self.set_y(y);
        // `CSAVE ""` es como sin nombre.
        let name = match name.filter(|n| !n.is_empty()).map(|n| validate_name(&n)).transpose() {
            Ok(name) => name,
            Err(error) => {
                self.raise(command, error);
                return true;
            }
        };
        let name = name.as_deref();
        let result = match command {
            HostCommand::Csave => self.host_csave(name, pending.ascii),
            HostCommand::Cload => self.host_load(command, name, pending.ascii, LoadKind::Replace),
            HostCommand::CloadVerify => self.host_load(command, name, pending.ascii, LoadKind::Verify),
            HostCommand::Merge => self.host_load(command, name, pending.ascii, LoadKind::Merge),
            HostCommand::Chain => self.host_load(command, name, pending.ascii, LoadKind::Chain(line)),
        };
        match result {
            Ok((next, event)) => {
                self.host_io.events.push(event);
                self.lh5801.set_pc(next);
            }
            Err(error) => self.raise(command, error),
        }
        true
    }

    fn host_dir_or_error(&self) -> Result<PathBuf, HostError> {
        self.host_io
            .dir
            .clone()
            .ok_or_else(|| HostError::new(ERR_NO_DEVICE, "E/S de archivos desactivada".to_owned()))
    }

    fn tape_or_error(&self) -> Result<PathBuf, HostError> {
        self.host_io.tape.clone().ok_or_else(|| {
            HostError::new(ERR_BAD_NAME, "sin nombre y sin archivo de cinta elegido".to_owned())
        })
    }

    /// El archivo y el formato de un `CSAVE` (ver [`csave_format`]). Un
    /// archivo que ya exista con otras mayúsculas se sobrescribe, en vez
    /// de crear otro que solo se distinga en eso.
    fn save_target(&self, name: Option<&str>, ascii: bool) -> Result<SaveTarget, HostError> {
        let dir = self.host_dir_or_error()?;
        let Some(name) = name else {
            let tape = self.tape_or_error()?;
            let file_name = tape
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            return Ok(SaveTarget { format: csave_format(&file_name, ascii), path: tape, header_name: file_name });
        };
        let file_name = csave_file_name(name, ascii);
        let existing = find_case_insensitive(&dir, &file_name)
            .map_err(|e| io_error("no se puede leer la carpeta", &dir, &e))?;
        Ok(SaveTarget {
            path: existing.unwrap_or_else(|| dir.join(&file_name)),
            format: csave_format(&file_name, ascii),
            header_name: name.to_owned(),
        })
    }

    /// El archivo que lee `CLOAD`/`MERGE`/`CHAIN` (ver
    /// [`cload_candidates`]); `ERROR 61` si no hay ninguno.
    fn load_source(&self, name: Option<&str>, ascii: bool) -> Result<PathBuf, HostError> {
        let dir = self.host_dir_or_error()?;
        let Some(name) = name else {
            let tape = self.tape_or_error()?;
            return if tape.is_file() {
                Ok(tape)
            } else {
                Err(HostError::new(ERR_NOT_FOUND, format!("no existe {}", tape.display())))
            };
        };
        for candidate in cload_candidates(name, ascii) {
            if let Some(path) = find_case_insensitive(&dir, &candidate)
                .map_err(|e| io_error("no se puede leer la carpeta", &dir, &e))?
            {
                return Ok(path);
            }
        }
        Err(HostError::new(
            ERR_NOT_FOUND,
            format!("no hay ning\u{fa}n \"{name}\" en {}", dir.display()),
        ))
    }

    /// `CSAVE`. El programa se lee de la RAM y se escribe con
    /// [`basic::encode_program`].
    fn host_csave(&self, name: Option<&str>, ascii: bool) -> Result<(u16, HostEvent), HostError> {
        if self.read_byte(MODE_SYMBOLS) & MODE_RESERVE != 0 {
            return Err(HostError::new(ERR_RESERVE_MODE, "no se puede en modo RESERVE".to_owned()));
        }
        let target = self.save_target(name, ascii)?;
        let program = self.basic_program().map_err(|e| {
            HostError::new(ERR_IO, format!("programa en memoria no v\u{e1}lido: {e}"))
        })?;
        let bytes = basic::encode_program(
            &program,
            target.format,
            &target.header_name,
            self.read_word(BASPRG_ST),
            &self.basic_keywords(),
        );
        write_atomically(&target.path, &bytes)
            .map_err(|e| io_error("no se puede escribir", &target.path, &e))?;
        let event = HostEvent::Saved {
            path: target.path,
            format: target.format,
            bytes: bytes.len(),
            lines: program.lines().len(),
        };
        Ok((BASIC_INT, event))
    }

    /// Lee e interpreta el archivo de una carga con [`basic::detect_format`]
    /// y [`basic::decode_program`], con las palabras clave de la máquina
    /// (las de la ROM y las de esta tabla, como las tokenizaría la ROM).
    fn read_program_file(
        &self,
        name: Option<&str>,
        ascii: bool,
    ) -> Result<(PathBuf, basic::DecodedProgram), HostError> {
        let path = self.load_source(name, ascii)?;
        let bytes = fs::read(&path).map_err(|e| io_error("no se puede leer", &path, &e))?;
        let format = basic::detect_format(path.extension().and_then(OsStr::to_str), &bytes);
        if format == BasicFormat::Ascii {
            check_ascii_lines(&basic::decode_text(&bytes))?;
        }
        let decoded =
            basic::decode_program(&bytes, format, &self.basic_keywords(), &AsciiOptions::default())
                .map_err(|e| decode_error(format, &e))?;
        Ok((path, decoded))
    }

    /// `CLOAD`, `CLOAD?`, `MERGE` y `CHAIN`.
    ///
    /// - `CLOAD`: en el inicio actual del programa (o donde lo pondría
    ///   `NEW0`), con su `$FF` y los punteros como `NEW`
    ///   (`Pc1500::write_program`). `ERROR 42` si no cabe (con el `$FF`)
    ///   antes de `RAM_END_H`:00, sin tocar nada.
    /// - `MERGE`: igual pero sobre el `$FF` del programa actual, dejando
    ///   `BASPRG_ST` y con `BASPRG_EDT` en el principio de lo añadido,
    ///   como `MERGE_150` de la CE-150 ($B9F7-$BA02).
    /// - Las dos y `CHAIN` hacen luego `DEL_DIM_VAR_1` ($D09C): si el
    ///   programa llega a las variables dimensionadas, se borran.
    /// - `CHAIN`: como `CLOAD` y luego `SRCH_TOP`/`SRCH_LINE`/`SRCH_ADD`
    ///   de la línea pedida (o la primera; `ERROR 11` si no existe) para
    ///   que la ROM siga ejecutando ahí. Las variables se conservan.
    /// - `CLOAD?`: compara los bytes con el programa de la máquina
    ///   (`ERROR 43` si no coinciden) sin tocar nada.
    fn host_load(
        &mut self,
        command: HostCommand,
        name: Option<&str>,
        ascii: bool,
        kind: LoadKind,
    ) -> Result<(u16, HostEvent), HostError> {
        let (path, decoded) = self.read_program_file(name, ascii)?;
        let bytes = decoded.program.to_tokenized();
        if kind == LoadKind::Verify {
            if bytes != self.basic_program_bytes() {
                return Err(HostError::new(
                    ERR_VERIFY,
                    format!("{} no coincide con el programa de la memoria", path.display()),
                ));
            }
            return Ok((BASIC_INT, HostEvent::Verified { path, bytes: bytes.len() }));
        }

        let (start, ram_end) = self
            .program_load_area()
            .map_err(|e| HostError::new(ERR_IO, e.to_string()))?;
        let old_start = self.read_word(BASPRG_ST);
        let old_end = self.read_word(BASPRG_END);
        let merge_at = (kind == LoadKind::Merge
            && old_start == start
            && (start..ram_end).contains(&old_end))
        .then_some(old_end);
        let at = merge_at.unwrap_or(start);
        let need = bytes.len() + 1;
        if usize::from(at) + need > usize::from(ram_end) {
            return Err(HostError::new(
                ERR_TOO_LARGE,
                format!(
                    "{} bytes y solo hay {} libres",
                    bytes.len(),
                    usize::from(ram_end).saturating_sub(usize::from(at) + 1)
                ),
            ));
        }
        let end = self.write_program(at, &bytes);
        if merge_at.is_some() {
            self.write_word(BASPRG_ST, old_start);
        }
        if end >= self.read_word(VAR_START) {
            self.write_word(VAR_START, ram_end);
        }

        let next = if let LoadKind::Chain(line) = kind {
            self.prepare_chain(at, line)?;
            CHAIN_GO
        } else {
            let first = self.read_word(BASPRG_ST);
            self.set_x(first);
            CONT_LOADED
        };
        let event = HostEvent::Loaded {
            command,
            path,
            format: decoded.format,
            bytes: bytes.len(),
            lines: decoded.program.lines().len(),
            warnings: decoded.warnings,
        };
        Ok((next, event))
    }

    /// Lo que haría `PRG_SEARCH` ($D0B4-$D0CD) con la línea de `CHAIN`:
    /// `SRCH_TOP` = inicio, `SRCH_LINE` = su número y `SRCH_ADD` = su
    /// dirección + 3 (el cuerpo). Sin número, la primera línea.
    fn prepare_chain(&mut self, start: u16, line: Option<u16>) -> Result<(), HostError> {
        let mut addr = start;
        let found = loop {
            if self.byte_at(addr) == END_MARK {
                if let Some(wanted) = line {
                    return Err(HostError::new(
                        ERR_LINE_NOT_FOUND,
                        format!("el programa no tiene la l\u{ed}nea {wanted}"),
                    ));
                }
                break start;
            }
            let number = self.read_word(addr);
            if line.is_none_or(|wanted| wanted == number) {
                break addr;
            }
            addr = addr.wrapping_add(3 + u16::from(self.byte_at(addr.wrapping_add(2))));
        };
        self.write_word(SRCH_TOP, start);
        let number = self.read_word(found);
        self.write_word(SRCH_LINE, number);
        self.write_word(SRCH_ADD, found.wrapping_add(3));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic::KeywordTable;

    #[test]
    fn table_image_layout() {
        let rom = host_rom();
        assert_eq!(rom[0], 0x55, "firma");
        assert_eq!(&rom[2..10], b"HOST\r\r\r\r", "nombre");
        assert_eq!(rom[0x0A..0x0D], [RTN; 3], "vector de inicializaci\u{f3}n");
        assert_eq!(rom[0x0D..0x10], ERROR_27, "INPUT#");
        assert_eq!(rom[0x10..0x13], ERROR_27, "PRINT#");
        assert_eq!(rom[0x1D..0x20], [0xC4, 0xAF, 0xFF], "TRACE");
        assert_eq!(rom[0x24..0x26], [0x98, 0x56], "puntero de la C");
        assert_eq!(rom[0x38..0x3A], [0x98, 0x74], "puntero de la M");
        let others = (0x20..0x54).filter(|&i| !(0x24..0x26).contains(&i) && !(0x38..0x3A).contains(&i));
        assert!(others.into_iter().all(|i| rom[i] == 0), "las dem\u{e1}s letras sin palabras");
        assert_eq!(
            rom[0x54..0x7D],
            [
                0x95, b'C', b'H', b'A', b'I', b'N', 0xF0, 0xB2, 0x98, 0x80, //
                0xA5, b'C', b'L', b'O', b'A', b'D', 0xF0, 0x89, 0x98, 0x83, //
                0x85, b'C', b'S', b'A', b'V', b'E', 0xF0, 0x95, 0x98, 0x87, //
                0xD5, b'M', b'E', b'R', b'G', b'E', 0xF0, 0x8F, 0x98, 0x8A, //
                0x90,
            ],
            "entradas y terminador"
        );
        for trap in [0x80, 0x84, 0x87, 0x8B, 0x92] {
            assert_eq!(rom[trap..trap + 3], ERROR_27, "reserva en +{trap:02X}");
        }
        assert_eq!(rom[0x83], 0x60, "m\u{e1}scara de CLOAD");
        assert_eq!(rom[0x8A], 0x60, "m\u{e1}scara de MERGE");
        assert_eq!(rom[0x90..0x92], [0xCD, 0x36], "VMJ ($36)");
        assert_eq!(rom[0x98..0x9F], [0xBE, 0xD0, 0xB4, 0xBE, 0xD0, 0x0D, 0xE2], "tras cargar");
        assert_eq!(rom[0xA0], 0xA5, "CHAIN_GO");
        assert!(rom[0xB0..].iter().all(|&b| b == 0xFF), "resto vac\u{ed}o");
        assert_eq!(rom_byte(0x9800), 0x55, "rom_byte");
        assert_eq!(rom_byte(0xA000), 0xFF, "fuera de la tabla");
    }

    #[test]
    fn table_parses_like_the_rom() {
        let table = KeywordTable::parse(host_rom(), HOST_TABLE_BASE, HOST_TABLE_BASE)
            .unwrap_or_else(|e| panic!("{e}"));
        let entries: Vec<(&[u8], u16, u16, bool)> = table
            .entries()
            .iter()
            .map(|e| (e.text.as_slice(), e.token, e.vector, e.last_of_letter))
            .collect();
        assert_eq!(
            entries,
            [
                (&b"CHAIN"[..], TOKEN_CHAIN, 0x9880, false),
                (&b"CLOAD"[..], TOKEN_CLOAD, 0x9883, false),
                (&b"CSAVE"[..], TOKEN_CSAVE, 0x9887, true),
                (&b"MERGE"[..], TOKEN_MERGE, 0x988A, true),
            ],
            "entradas"
        );
        assert_eq!(table.first_for_letter(b'C'), Some(0), "C");
        assert_eq!(table.first_for_letter(b'M'), Some(3), "M");
        assert_eq!(table.first_for_letter(b'A'), None, "A");
    }

    #[test]
    fn csave_names_and_formats() {
        assert_eq!(csave_file_name("X", false), "X.BAS", "CSAVE");
        assert_eq!(csave_file_name("X", true), "X.ASC", "CSAVEa");
        assert_eq!(csave_file_name("x.bin", true), "x.bin", "extensi\u{f3}n expl\u{ed}cita");
        assert_eq!(csave_format("X.BAS", false), SaveFormat::Ce158, "CE-158");
        assert_eq!(csave_format("X.ASC", true), SaveFormat::AsciiCr, "CSAVEa");
        assert_eq!(csave_format("X.ASC", false), SaveFormat::Ce158, ".asc sin a: regla CE-158");
        assert_eq!(csave_format("X.bas", true), SaveFormat::AsciiCrLf, ".bas con a: CRLF");
        assert_eq!(csave_format("X.Bin", true), SaveFormat::Tokenized, ".bin manda");
        assert_eq!(csave_format("X.TXT", false), SaveFormat::AsciiCrLf, ".txt manda");
        assert_eq!(csave_format("X.DAT", false), SaveFormat::Ce158, "otra extensi\u{f3}n");
        assert_eq!(csave_format("tape", true), SaveFormat::AsciiCrLf, "cinta sin extensi\u{f3}n, con a");
    }

    #[test]
    fn cload_candidate_order() {
        assert_eq!(cload_candidates("X", false), ["X.BAS", "X.ASC", "X.TXT", "X.BIN", "X"], "sin a");
        assert_eq!(cload_candidates("X", true), ["X.ASC", "X.BAS", "X.TXT", "X.BIN", "X"], "con a");
        assert_eq!(cload_candidates("X.bin", false), ["X.bin"], "con extensi\u{f3}n");
    }

    #[test]
    fn names_are_sandboxed() {
        assert_eq!(validate_name(b"MOLE"), Ok("MOLE".to_owned()), "normal");
        assert_eq!(validate_name(b"t1.bas"), Ok("t1.bas".to_owned()), "con extensi\u{f3}n");
        assert_eq!(validate_name(b"MY GAME"), Ok("MY GAME".to_owned()), "con espacio");
        for bad in [
            &b"../X"[..],
            b"A/B",
            b"A\\B",
            b"C:X",
            b".hidden",
            b"..",
            b"A..B",
            b"X.",
            b"X ",
            b"   ",
            b"A\x01",
            b"A\xF0",
            b"A*",
            b"A?",
        ] {
            let result = validate_name(bad);
            assert_eq!(
                result.map_err(|e| e.code),
                Err(ERR_BAD_NAME),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
    }

    #[test]
    fn ascii_line_check() {
        assert_eq!(check_ascii_lines("' comentario\r\n\r\n10 PRINT 1\r\n\x1A"), Ok(()), "v\u{e1}lido");
        assert_eq!(check_ascii_lines("   20 A=1\r"), Ok(()), "espacios delante");
        let error = check_ascii_lines("10 PRINT 1\nPRINT 2\n").map_err(|e| e.code);
        assert_eq!(error, Err(ERR_ASCII_LINE), "l\u{ed}nea sin n\u{fa}mero");
    }

    #[test]
    fn case_insensitive_lookup() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path();
        fs::write(path.join("Mole.Bas"), b"x").unwrap_or_else(|e| panic!("{e}"));
        fs::create_dir(path.join("SUB.BAS")).unwrap_or_else(|e| panic!("{e}"));
        // On a case-insensitive file system (macOS, Windows) the exact
        // name already opens `Mole.Bas`: what matters is finding that file.
        let found = find_case_insensitive(path, "MOLE.BAS").unwrap_or_else(|e| panic!("{e}"));
        let contents = found.as_ref().and_then(|found| fs::read(found).ok());
        assert_eq!(contents, Some(b"x".to_vec()), "sin distinguir may\u{fa}sculas: {found:?}");
        let dir_only = find_case_insensitive(path, "sub.bas").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(dir_only, None, "las carpetas no cuentan");
        write_atomically(&path.join("Mole.Bas"), b"new").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(fs::read(path.join("Mole.Bas")).ok(), Some(b"new".to_vec()), "sustituido");
        let leftovers = fs::read_dir(path)
            .map(|d| d.filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().ends_with(".ceres-tmp")).count())
            .unwrap_or_default();
        assert_eq!(leftovers, 0, "sin temporales");
    }

    #[test]
    fn event_texts() {
        let saved = HostEvent::Saved {
            path: PathBuf::from("/d/T1.BAS"),
            format: SaveFormat::Ce158,
            bytes: 1007,
            lines: 20,
        };
        assert_eq!(
            saved.to_string(),
            "CSAVE \u{2192} /d/T1.BAS (tokenizado con cabecera CE-158, 20 l\u{ed}neas, 1007 bytes)",
            "guardado"
        );
        assert_eq!(saved.error_code(), None, "sin error");
        let error = HostEvent::Error { command: HostCommand::Cload, code: 61, message: "no existe".to_owned() };
        assert_eq!(error.to_string(), "CLOAD: ERROR 61 \u{2014} no existe", "error");
        assert_eq!(error.error_code(), Some(61), "c\u{f3}digo");
    }
}
