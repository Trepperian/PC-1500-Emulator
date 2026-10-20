//! `CLOAD`, `CSAVE`, `MERGE` and `CHAIN` (and `CLOAD?`, and the CE-158's `a` suffix) from the
//! emulated PC-1500 itself, with host files.
//!
//! # Why a keyword table is needed
//! The A04 main ROM has none of these commands: on the real machine they come with the CE-150
//! (printer/cassette, table 01 `CMT` at $B800) or the CE-158 (RS-232, table 08 `COM` at $8000 with
//! `PV`=1). Without them `CLOAD` stays as single letters (`ERROR 1`) and an already tokenized
//! `F0 89` gives `ERROR 27` when run (`TOK_PROCESS` $FA89, `LDI UH,$1B` at $FAE4). The tokens are
//! the same in both: `CHAIN` $F0B2, `CLOAD` $F089, `CSAVE` $F095, `MERGE` $F08F. Being $F0xx
//! tokens, the ROM looks for them in the table of `OPN` ($79D1, by default $60 = $C000) and then in
//! each table with the signature $55 from $B800 to $8000 in steps of 2 KB, first with `PV`=0 and
//! then with `PV`=1 (`DEC_OPN` $FA58, `TOK_PROCESS_7..9` $FAC5-$FAE0; `TOK_TABL_SRCH` $E4A8 does
//! the same at boot: `CPI XH,$88` + `ADI A,$F7`, which adds the carry, gives steps of 8 pages: $B8,
//! $B0, ..., $80). So ONE table with that shape at any of those addresses is enough for the real
//! ROM's tokenizer, `LIST` and interpreter to know the four commands, without needing any
//! peripheral ROM.
//!
//! # Why at $9800 with `PV`=0
//! It is an area that no real expansion uses (the CE-150 is at `PV`=0 $A000-$BFFF and the CE-158 at
//! `PV`=1 $8000-$9FFF). At $B800 the ROM would believe there is a CE-150: cassette
//! `PRINT#`/`INPUT#` jump to $B8A3 blindly ($E51E-$E524) and the system vectors $FFA4-$FFB8 point
//! at $B888-$B8A0. At $9800 the table also falls in the walk of `TOK_TABL_SRCH` at boot, so its
//! initialization vector (an `RTN`) is called at each reset (`RESET_19` $E10B-$E115).
//!
//! # The table ($9800-$9FFF, the rest $FF)
//! ```text
//! +00 55                      signature ($FA84 CPI XL,$55; $E4B4 CPA (X))
//! +01 0E                      table number (CE-150 01/02, CE-158 08/09)
//! +02 "HOST" 0D 0D 0D 0D      name (compared by OPN "...", $E47D-$E489)
//! +0A 9A 9A 9A                initialization vector: RTN
//! +0D 68 1B E0                vector of INPUT#: ERROR 27 (as CE-150 $B80D)
//! +10 68 1B E0                vector of PRINT#: the same
//! +13..+1C 9A                 padding
//! +1D C4 AF FF                vector of TRACE = TRCROUTINE $C4AF
//! +20..+53                    pointers by letter (C -> $9856, M -> $9874)
//! +54 95 "CHAIN" F0B2 9880    entries [ctl][text][token][vector]
//! +5E A5 "CLOAD" F089 9883
//! +68 85 "CSAVE" F095 9887
//! +72 D5 "MERGE" F08F 988A
//! +7C 90                      end (low nibble 0)
//! +80 CHAIN trap    +83 60 mode mask of CLOAD   +84 CLOAD trap
//! +87 CSAVE trap    +8A 60 mode mask of MERGE   +8B MERGE trap
//! +90 CD 36           VMJ ($36) = STRING_2_CSI $DF0F (name -> CSI)
//! +92 "name evaluated" trap
//! +98 BE D0 B4  BE D0 0D  E2   PRG_SEARCH, INIT_SYS_ADDR_6, BASIC_INT
//! +A0 A5 7B 0E FD C8 BE D0 0D FD 8A AE 7B 0E BA C8 C1
//!                             resume after CHAIN, copied byte by byte
//!                             from the CE-150 ($BA1D-$BA2C)
//! ```
//! The attributes of each entry go in the high nibble of the `ctl` of the NEXT one (see
//! [`crate::basic::KeywordTable`]): bit 3 always, bit 4 "last of its letter", bits 6/5 "allowed in
//! direct mode/in a program" (`PV_BANK_6` $FB7A-$FB9C). If the last two are 0, the command is
//! direct-only and the first byte of its routine is a mode mask ANDed with $764F (bit 6 RUN, 5 PRO,
//! 4 RESERVE; `ERROR 26` if it gives 0); it runs from the next byte. Chosen as in the CE-150 and
//! the CE-158: `CHAIN` $A (only in programs), `CLOAD` $8 (direct only, RUN or PRO: `ERROR 1` inside
//! a program, as on the CE-158), `CSAVE` $D (everywhere, last of the C) and `MERGE` $9 (direct
//! only, last of the M).
//!
//! # Traps
//! Each "trap" address has in the table the bytes `68 1B E0` (`LDI UH,$1B` / `VEJ (E0)`:
//! `ERROR 27`, "peripheral not connected"). When the CPU is about to run there with `PV`=0,
//! `Pc1500::step_cpu` first calls `Pc1500::host_trap`, which does the work in Rust and leaves the
//! PC at the next ROM routine. If host I/O is disabled (folder `None`, the default in the core),
//! the trap does nothing and those bytes run: `ERROR 27`, the same as on a PC-1500 with no
//! peripheral.
//!
//! 1. On entering `CLOAD`/`CSAVE`/`MERGE`/`CHAIN` (from `PV_BANK_6`, with `Y` at the first byte
//!    after the token) the modifiers are read: `a` (ASCII, as on the CE-158) and `?` (only `CLOAD`:
//!    verify), and it jumps to `VMJ ($36)`, the ROM routine that evaluates a string literal or
//!    variable and leaves its descriptor ("CSI") at $7A04-$7A07.
//! 2. On coming back (`+92`), with the carry set if there was a name, the name is read, in `CHAIN`
//!    the optional `,line`, a `:` or the end of the line is required, and the operation is done. It
//!    leaves through `BASIC_INT` ($C400, with `Y` at the terminator: the rest of the line goes on
//!    running), through `+98`/`+A0` after loading, or through `ERRN` ($CD8B) with the code in `UH`.
//!    The stack that `PV_BANK_6` left is not needed: `BASIC_INT` and `ERRN` reload `S`=$784F.
//!
//! # Files
//! Everything that is format (tokenizing, detecting, the CE-158 header, writing the program in RAM)
//! belongs to [`crate::basic`]; here only which file and which format are decided, as the CE-158
//! does:
//!
//! - `CSAVE "X"` writes `X.BAS`, tokenized with the CE-158's 27-byte header, and `CSAVEa "X"`,
//!   `X.ASC` in text (CR, as the CE-158X's `.ASC` files). With an explicit extension it is used as
//!   it is; the extension decides the format only if it is `.bin` (tokenized without header) or
//!   `.txt` (CRLF text); with any other, the CE-158's rule rules (with `a` text: CR if `.asc`, CRLF
//!   otherwise; without `a` tokenized with header). See [`csave_format`].
//! - `CLOAD` works out the format by itself (text, tokenized or CE-158 header), with `a` or without
//!   it, and looks for the name ignoring case: as it is if it has an extension; otherwise, `.BAS`,
//!   `.ASC`, `.TXT`, `.BIN` (with `a`, `.ASC` first) and the bare name.
//! - Without a name (`CSAVE`, `CLOAD`) the "tape" file chosen in the GUI is used
//!   ([`Pc1500::set_tape`]); with no tape, `ERROR 40`.
//! - Only safe names inside the folder: printable ASCII without `/ \ : * ? " < > |`, not starting
//!   with `.`, without `..` and without `.` or a space at the end; otherwise, `ERROR 40`.
//!
//! # Errors (those of the CE-150/CE-158)
//! `1` syntax, `11` `CHAIN` line that does not exist, `26` `CSAVE` in RESERVE mode, `27` host I/O
//! disabled, `40` invalid name or no tape, `42` does not fit, `43` `CLOAD?` does not match, `44`
//! read or write error or corrupt tokenized file, `61` the file does not exist, `67` text line
//! without a number or that cannot be tokenized.

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

// --- Map of the table -------------------------------------------------------

/// First address (ME0) of the virtual table, for the `match` of `Pc1500::read_byte`.
pub(crate) const TABLE_BEGIN: u32 = 0x9800;
/// Last address (ME0) of the virtual table.
pub(crate) const TABLE_END: u32 = 0x9FFF;
/// The same, to compare with the PC in `Pc1500::step_cpu`: `PC & MASK == PAGE` is "inside
/// $9800-$9FFF".
pub(crate) const TRAP_PAGE: u16 = 0x9800;
pub(crate) const TRAP_PAGE_MASK: u16 = 0xF800;
/// CPU states charged for each trap that acts. The operation is instant (a 1 KB file through the
/// real CE-158 at 1200 baud would take some 10 s): this only keeps a trap from not moving the CPU's
/// clock on, which the timer and the peripherals depend on.
pub(crate) const TRAP_STATES: u8 = 200;

/// Address of the table.
pub const HOST_TABLE_BASE: u16 = 0x9800;
/// Size of the table's window: 2 KB, as each expansion table.
const TABLE_SIZE: usize = 0x800;

/// Token of `CHAIN` (CE-150 $B854, CE-158).
pub const TOKEN_CHAIN: u16 = 0xF0B2;
/// Token of `CLOAD`.
pub const TOKEN_CLOAD: u16 = 0xF089;
/// Token of `CSAVE`.
pub const TOKEN_CSAVE: u16 = 0xF095;
/// Token of `MERGE`.
pub const TOKEN_MERGE: u16 = 0xF08F;

/// Trap of `CHAIN` (its vector points here).
const H_CHAIN: u16 = HOST_TABLE_BASE + 0x80;
/// Mode mask of `CLOAD` (its vector points here; the routine starts at the next byte, see the
/// module's documentation).
const CLOAD_MASK: u16 = HOST_TABLE_BASE + 0x83;
/// Trap of `CLOAD`.
const H_CLOAD: u16 = HOST_TABLE_BASE + 0x84;
/// Trap of `CSAVE`.
const H_CSAVE: u16 = HOST_TABLE_BASE + 0x87;
/// Mode mask of `MERGE`.
const MERGE_MASK: u16 = HOST_TABLE_BASE + 0x8A;
/// Trap of `MERGE`.
const H_MERGE: u16 = HOST_TABLE_BASE + 0x8B;
/// `VMJ ($36)`: evaluate the name.
const EVAL: u16 = HOST_TABLE_BASE + 0x90;
/// Trap on returning from `VMJ ($36)`.
const EVAL_DONE: u16 = HOST_TABLE_BASE + 0x92;
/// After `CLOAD`/`MERGE`: `PRG_SEARCH`, `INIT_SYS_ADDR_6` (what `NEW` does, $C85C) and `BASIC_INT`.
const CONT_LOADED: u16 = HOST_TABLE_BASE + 0x98;
/// After `CHAIN`: resume running in the new program.
const CHAIN_GO: u16 = HOST_TABLE_BASE + 0xA0;

/// `BASIC_INT` ($C400, vector `VEJ (E2)`): go on interpreting from `Y` (which must point at `:` or
/// at the end of the line).
const BASIC_INT: u16 = 0xC400;
/// `ERRN` ($CD8B, vector `VEJ (E0)`): `ERROR` with the code of `UH`.
const ERRN: u16 = 0xCD8B;

/// Fallback bytes of each trap: `LDI UH,$1B` / `VEJ (E0)` = `ERROR 27`.
const ERROR_27: [u8; 3] = [0x68, 0x1B, 0xE0];
/// `RTN`.
const RTN: u8 = 0x9A;
/// Mode mask of the direct-only commands: RUN | PRO.
const MODE_MASK_RUN_PRO: u8 = 0x60;

/// An entry of the table: text, token, vector and attributes (nibble).
struct TableEntry {
    text: &'static [u8],
    token: u16,
    vector: u16,
    attributes: u8,
}

/// The entries, in the order of the table (alphabetical within each letter, as the ROM walks them
/// to tokenize and for `LIST`).
const ENTRIES: [TableEntry; 4] = [
    TableEntry { text: b"CHAIN", token: TOKEN_CHAIN, vector: H_CHAIN, attributes: 0xA },
    TableEntry { text: b"CLOAD", token: TOKEN_CLOAD, vector: CLOAD_MASK, attributes: 0x8 },
    TableEntry { text: b"CSAVE", token: TOKEN_CSAVE, vector: H_CSAVE, attributes: 0xD },
    TableEntry { text: b"MERGE", token: TOKEN_MERGE, vector: MERGE_MASK, attributes: 0x9 },
];

/// High nibble of the first `ctl`, which are nobody's attributes: $9, as in the CE-150's table
/// ($B854 = $95).
const FIRST_CTL_HIGH: u8 = 0x9;

/// Builds the image of the table (see the module's documentation).
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
            // To the SECOND character of the first word of the letter.
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

/// The image of the table, built only once.
fn host_rom() -> &'static [u8; TABLE_SIZE] {
    static ROM: OnceLock<[u8; TABLE_SIZE]> = OnceLock::new();
    ROM.get_or_init(build_rom)
}

/// A byte of the table ($9800-$9FFF; outside, $FF).
pub(crate) fn rom_byte(addr: u32) -> u8 {
    addr.checked_sub(TABLE_BEGIN)
        .and_then(|i| host_rom().get(usize::try_from(i).ok()?).copied())
        .unwrap_or(0xFF)
}

/// The 2 bytes of a word, big-endian.
const fn word_bytes(word: u16) -> [u8; 2] {
    [(word >> 8) as u8, (word & 0xFF) as u8]
}

// --- Addresses of the system RAM ------------------------------------

/// Descriptor ("CSI") of the last string result: $7A04 = $D0 if it is a string, $7A05/06 its
/// address and $7A07 its length (`CREATE_CSI_4` $DEAF).
const CSI_TYPE: u32 = 0x7A04;
const CSI_ADDR: u32 = 0x7A05;
const CSI_LEN: u32 = 0x7A07;
const CSI_STRING: u8 = 0xD0;
/// Mode indicators of the LCD: bit 4 = RESERVE.
const MODE_SYMBOLS: u32 = 0x764F;
const MODE_RESERVE: u8 = 0x10;
/// `SRCH_ADD` ($78A6/A7), `SRCH_LINE` ($78A8/A9) and `SRCH_TOP` ($78AA/AB), what `PRG_SEARCH`
/// ($D0B4-$D0CD) leaves and `RUN` reads.
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

/// A PC-1500 `ERROR n` and its explanation for the host.
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

// --- State and events ---------------------------------------------------------

/// Which tape command ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostCommand {
    Cload,
    /// `CLOAD?`: compare the file with the machine's program.
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

/// What a tape command has done, for the GUI ([`Pc1500::take_host_events`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostEvent {
    /// `CSAVE`: file written.
    Saved { path: PathBuf, format: SaveFormat, bytes: usize, lines: usize },
    /// `CLOAD`, `MERGE` or `CHAIN`: program loaded.
    Loaded {
        command: HostCommand,
        path: PathBuf,
        format: BasicFormat,
        /// Bytes of the program loaded (without the `$FF`).
        bytes: usize,
        lines: usize,
        warnings: Vec<Warning>,
    },
    /// `CLOAD?`: the file matches the memory.
    Verified { path: PathBuf, bytes: usize },
    /// The command ended in `ERROR code` on the PC-1500.
    Error { command: HostCommand, code: u8, message: String },
}

impl HostEvent {
    /// The PC-1500's `ERROR` code, if the command failed.
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
        SaveFormat::Ce158 => "tokenized with CE-158 header",
        SaveFormat::Tokenized => "tokenized",
        SaveFormat::AsciiCr => "CR text",
        SaveFormat::AsciiCrLf => "CRLF text",
    }
}

const fn basic_format_label(format: BasicFormat) -> &'static str {
    match format {
        BasicFormat::Ce158 => "tokenized with CE-158 header",
        BasicFormat::Tokenized => "tokenized",
        BasicFormat::Ascii => "text",
    }
}

impl fmt::Display for HostEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Saved { path, format, bytes, lines } => write!(
                f,
                "CSAVE \u{2192} {} ({}, {lines} lines, {bytes} bytes)",
                path.display(),
                save_format_label(*format)
            ),
            Self::Loaded { command, path, format, bytes, lines, warnings } => {
                write!(
                    f,
                    "{command} \u{2190} {} ({}, {lines} lines, {bytes} bytes)",
                    path.display(),
                    basic_format_label(*format)
                )?;
                if let Some(first) = warnings.first() {
                    write!(f, "; {} warnings: {first}", warnings.len())?;
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

/// A command in progress between the first trap and the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    command: HostCommand,
    /// Sufijo `a`.
    ascii: bool,
}

/// Configuration and state of the host's file I/O.
#[derive(Debug, Default)]
pub(crate) struct HostIo {
    /// Folder of the files; `None` = disabled (`ERROR 27`).
    dir: Option<PathBuf>,
    /// "Tape" file for `CLOAD`/`CSAVE` without a name.
    tape: Option<PathBuf>,
    pending: Option<Pending>,
    events: Vec<HostEvent>,
}

impl HostIo {
    /// For `Pc1500::reset`: the configuration (folder and tape) and the events the GUI has not
    /// collected yet survive; a command in progress does not (the ROM starts from scratch).
    pub(crate) fn keep_config_from(&mut self, old: Self) {
        self.dir = old.dir;
        self.tape = old.tape;
        self.events = old.events;
        self.pending = None;
    }
}

// --- Names and formats (pure functions) -----------------------------------

/// Characters a name cannot have (separators and those not valid on Windows).
const FORBIDDEN_NAME_CHARS: &[u8] = b"/\\:*?\"<>|";

/// The name as the program gave it, checked (see the module's documentation): `ERROR 40` if it is
/// not safe.
fn validate_name(raw: &[u8]) -> Result<String, HostError> {
    let bad = |why: &str| {
        HostError::new(
            ERR_BAD_NAME,
            format!("invalid name \"{}\": {why}", String::from_utf8_lossy(raw)),
        )
    };
    if raw.iter().any(|b| !(0x20..=0x7E).contains(b)) {
        return Err(bad("printable ASCII only"));
    }
    if raw.iter().any(|b| FORBIDDEN_NAME_CHARS.contains(b)) {
        return Err(bad("it cannot have / \\ : * ? \" < > |"));
    }
    if raw.first() == Some(&b'.') {
        return Err(bad("it cannot start with '.'"));
    }
    if raw.windows(2).any(|w| w == b"..") {
        return Err(bad("it cannot have '..'"));
    }
    if matches!(raw.last(), Some(b'.' | b' ')) {
        return Err(bad("it cannot end in '.' or in a space"));
    }
    if raw.iter().all(|&b| b == b' ') {
        return Err(bad("empty"));
    }
    Ok(raw.iter().map(|&b| char::from(b)).collect())
}

/// The extension of a file name, if it has one.
fn extension(file_name: &str) -> Option<&str> {
    Path::new(file_name).extension().and_then(OsStr::to_str)
}

/// The file name of `CSAVE "name"`: with an extension, as it is; without it, `.ASC` with the `a`
/// suffix and `.BAS` without it (as the CE-158).
fn csave_file_name(name: &str, ascii: bool) -> String {
    if extension(name).is_some() {
        name.to_owned()
    } else {
        format!("{name}.{}", if ascii { "ASC" } else { "BAS" })
    }
}

/// The format of `CSAVE` to `file_name`.
///
/// See the module's documentation: `.bin` tokenized without header and `.txt` CRLF text whatever
/// happens; otherwise, with `a` text (CR in `.asc`, CRLF in the rest) and without `a` tokenized
/// with the CE-158 header, as the real CE-158.
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

/// The names `CLOAD "name"` tries, in order: as it is if it has an extension; otherwise, with
/// `.BAS`, `.ASC`, `.TXT` and `.BIN` (`.ASC` first with the `a` suffix) and lastly bare.
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

/// The file `file_name` of `dir` ignoring case: the exact name if it exists, otherwise the first
/// one (in alphabetical order) that matches.
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

/// `ERROR 67` if the text has a line that is not a program line: empty ones, only spaces or control
/// characters (the end-of-file Ctrl-Z) and `'` comments (the corpus's headers) are skipped; the
/// rest must start with a number, as those typed on the machine.
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
            format!("line {} of the text without a number: \"{shown}\"", index + 1),
        ));
    }
    Ok(())
}

/// The `ERROR` due for a file that could not be read: `67` in text (bad line), `44` in tokenized
/// (corrupt data).
fn decode_error(format: BasicFormat, error: &BasicError) -> HostError {
    let code = if format == BasicFormat::Ascii { ERR_ASCII_LINE } else { ERR_IO };
    HostError::new(code, error.to_string())
}

fn io_error(what: &str, path: &Path, error: &io::Error) -> HostError {
    HostError::new(ERR_IO, format!("{what} {}: {error}", path.display()))
}

/// Writes `bytes` to `path` without ever leaving a half-written file: first to a hidden temporary
/// file in the same folder and then `rename`, which replaces the previous one in one go.
fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let Some(file_name) = path.file_name() else {
        return Err(io::Error::other("path without a file name"));
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

// --- The machine ---------------------------------------------------------------

/// Where a `CSAVE` goes.
struct SaveTarget {
    path: PathBuf,
    format: SaveFormat,
    /// The name of the CE-158 header.
    header_name: String,
}

/// What the load does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadKind {
    /// `CLOAD`: replace the program.
    Replace,
    /// `CLOAD?`: only compare.
    Verify,
    /// `MERGE`: add after the current program.
    Merge,
    /// `CHAIN`: replace and go on running (at the given line or at the first).
    Chain(Option<u16>),
}

impl Pc1500 {
    /// Enables the host's file I/O with `dir` as the folder of `CLOAD`/`CSAVE`/`MERGE`/`CHAIN`, or
    /// disables it with `None` (the default in [`Pc1500::new`], so that the tests and any use
    /// without the GUI do not touch the disk): then those commands give `ERROR 27`, as a PC-1500
    /// with no peripheral. The GUI uses the current folder. It survives [`Pc1500::reset`].
    pub fn set_host_dir(&mut self, dir: Option<PathBuf>) {
        self.host_io.dir = dir;
    }

    /// The folder of [`Pc1500::set_host_dir`].
    #[must_use]
    pub fn host_dir(&self) -> Option<&Path> {
        self.host_io.dir.as_deref()
    }

    /// The "tape" file that `CLOAD` and `CSAVE` without a name use (full path, it need not be in
    /// the folder), or `None` (then without a name they give `ERROR 40`). It survives
    /// [`Pc1500::reset`].
    pub fn set_tape(&mut self, tape: Option<PathBuf>) {
        self.host_io.tape = tape;
    }

    /// The file of [`Pc1500::set_tape`].
    #[must_use]
    pub fn tape(&self) -> Option<&Path> {
        self.host_io.tape.as_deref()
    }

    /// What the tape commands have done since the last call, in order (for the GUI's status line).
    pub fn take_host_events(&mut self) -> Vec<HostEvent> {
        mem::take(&mut self.host_io.events)
    }

    /// Trap of `Pc1500::step_cpu`: the PC is at $9800-$9FFF with `PV`=0. It returns `true` if it
    /// has done the work (and moved the PC), or `false` so that the instruction there runs.
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

    /// Jumps to `ERRN` with `code` in `UH`: the ROM shows `ERROR code` (or `ERROR code IN line`)
    /// and notes `ERL` as with any error.
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

    /// First trap: modifiers (`a`, `?`) and off to evaluate the name.
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
                // `CSAVE M "X";start,end` and `CLOAD M`: machine code on the CE-150, not supported.
                b'-' => {
                    self.raise(command, HostError::new(ERR_SYNTAX, "form not supported".to_owned()));
                    return true;
                }
                b'M' if self.byte_at(y.wrapping_add(1)) == b'"' => {
                    self.raise(
                        command,
                        HostError::new(ERR_SYNTAX, "machine code (M) not supported".to_owned()),
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

    /// What `VMJ ($36)` left: the name if there was a string (carry set and a string CSI);
    /// otherwise, `Y` goes back to where it was (`X`, as `HDR_PREP_2` $BDBE of the CE-150) and
    /// there is no name.
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

    /// Second trap: name evaluated; do the operation.
    fn trap_eval_done(&mut self) -> bool {
        let Some(pending) = self.host_io.pending.take() else {
            if self.host_io.dir.is_none() {
                return false;
            }
            // Someone got here without going through the first trap (a `CALL` by hand).
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
                    self.raise(command, HostError::new(ERR_SYNTAX, "line number not valid".to_owned()));
                    return true;
                }
            }
        }
        if !matches!(self.byte_at(y), COLON | CR) {
            self.raise(command, HostError::new(ERR_SYNTAX, "syntax".to_owned()));
            return true;
        }
        self.set_y(y);
        // `CSAVE ""` is like no name.
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
            .ok_or_else(|| HostError::new(ERR_NO_DEVICE, "file I/O disabled".to_owned()))
    }

    fn tape_or_error(&self) -> Result<PathBuf, HostError> {
        self.host_io.tape.clone().ok_or_else(|| {
            HostError::new(ERR_BAD_NAME, "no name and no tape file chosen".to_owned())
        })
    }

    /// The file and the format of a `CSAVE` (see [`csave_format`]). A file that already exists with
    /// a different case is overwritten, instead of creating another that only differs in that.
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
            .map_err(|e| io_error("the folder cannot be read", &dir, &e))?;
        Ok(SaveTarget {
            path: existing.unwrap_or_else(|| dir.join(&file_name)),
            format: csave_format(&file_name, ascii),
            header_name: name.to_owned(),
        })
    }

    /// The file that `CLOAD`/`MERGE`/`CHAIN` read (see [`cload_candidates`]); `ERROR 61` if there
    /// is none.
    fn load_source(&self, name: Option<&str>, ascii: bool) -> Result<PathBuf, HostError> {
        let dir = self.host_dir_or_error()?;
        let Some(name) = name else {
            let tape = self.tape_or_error()?;
            return if tape.is_file() {
                Ok(tape)
            } else {
                Err(HostError::new(ERR_NOT_FOUND, format!("{} does not exist", tape.display())))
            };
        };
        for candidate in cload_candidates(name, ascii) {
            if let Some(path) = find_case_insensitive(&dir, &candidate)
                .map_err(|e| io_error("the folder cannot be read", &dir, &e))?
            {
                return Ok(path);
            }
        }
        Err(HostError::new(
            ERR_NOT_FOUND,
            format!("there is no \"{name}\" in {}", dir.display()),
        ))
    }

    /// `CSAVE`. The program is read from RAM and written with [`basic::encode_program`].
    fn host_csave(&self, name: Option<&str>, ascii: bool) -> Result<(u16, HostEvent), HostError> {
        if self.read_byte(MODE_SYMBOLS) & MODE_RESERVE != 0 {
            return Err(HostError::new(ERR_RESERVE_MODE, "not allowed in RESERVE mode".to_owned()));
        }
        let target = self.save_target(name, ascii)?;
        let program = self.basic_program().map_err(|e| {
            HostError::new(ERR_IO, format!("program in memory not valid: {e}"))
        })?;
        let bytes = basic::encode_program(
            &program,
            target.format,
            &target.header_name,
            self.read_word(BASPRG_ST),
            &self.basic_keywords(),
        );
        write_atomically(&target.path, &bytes)
            .map_err(|e| io_error("it cannot be written", &target.path, &e))?;
        let event = HostEvent::Saved {
            path: target.path,
            format: target.format,
            bytes: bytes.len(),
            lines: program.lines().len(),
        };
        Ok((BASIC_INT, event))
    }

    /// Reads and interprets the file of a load with [`basic::detect_format`] and
    /// [`basic::decode_program`], with the machine's keywords (those of the ROM and those of this
    /// table, as the ROM would tokenize them).
    fn read_program_file(
        &self,
        name: Option<&str>,
        ascii: bool,
    ) -> Result<(PathBuf, basic::DecodedProgram), HostError> {
        let path = self.load_source(name, ascii)?;
        let bytes = fs::read(&path).map_err(|e| io_error("it cannot be read", &path, &e))?;
        let format = basic::detect_format(path.extension().and_then(OsStr::to_str), &bytes);
        if format == BasicFormat::Ascii {
            check_ascii_lines(&basic::decode_text(&bytes))?;
        }
        let decoded =
            basic::decode_program(&bytes, format, &self.basic_keywords(), &AsciiOptions::default())
                .map_err(|e| decode_error(format, &e))?;
        Ok((path, decoded))
    }

    /// `CLOAD`, `CLOAD?`, `MERGE` and `CHAIN`.
    ///
    /// - `CLOAD`: at the program's current start (or where `NEW0` would put it), with its `$FF` and
    ///   the pointers as `NEW` (`Pc1500::write_program`). `ERROR 42` if it does not fit (with the
    ///   `$FF`) before `RAM_END_H`:00, without touching anything.
    /// - `MERGE`: the same but over the `$FF` of the current program, keeping `BASPRG_ST` and with
    ///   `BASPRG_EDT` at the start of what is added, as the CE-150's `MERGE_150` ($B9F7-$BA02).
    /// - Both and `CHAIN` then do `DEL_DIM_VAR_1` ($D09C): if the program reaches the dimensioned
    ///   variables, they are deleted.
    /// - `CHAIN`: as `CLOAD` and then `SRCH_TOP`/`SRCH_LINE`/`SRCH_ADD` of the line asked for (or
    ///   the first; `ERROR 11` if it does not exist) so that the ROM goes on running there. The
    ///   variables are kept.
    /// - `CLOAD?`: compares the bytes with the machine's program (`ERROR 43` if they do not match)
    ///   without touching anything.
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
                    format!("{} does not match the program in memory", path.display()),
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
                    "{} bytes and there are only {} free",
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

    /// What `PRG_SEARCH` ($D0B4-$D0CD) would do with the line of `CHAIN`: `SRCH_TOP` = start,
    /// `SRCH_LINE` = its number and `SRCH_ADD` = its address + 3 (the body). Without a number, the
    /// first line.
    fn prepare_chain(&mut self, start: u16, line: Option<u16>) -> Result<(), HostError> {
        let mut addr = start;
        let found = loop {
            if self.byte_at(addr) == END_MARK {
                if let Some(wanted) = line {
                    return Err(HostError::new(
                        ERR_LINE_NOT_FOUND,
                        format!("the program has no line {wanted}"),
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
        assert_eq!(rom[0], 0x55, "signature");
        assert_eq!(&rom[2..10], b"HOST\r\r\r\r", "name");
        assert_eq!(rom[0x0A..0x0D], [RTN; 3], "initialization vector");
        assert_eq!(rom[0x0D..0x10], ERROR_27, "INPUT#");
        assert_eq!(rom[0x10..0x13], ERROR_27, "PRINT#");
        assert_eq!(rom[0x1D..0x20], [0xC4, 0xAF, 0xFF], "TRACE");
        assert_eq!(rom[0x24..0x26], [0x98, 0x56], "pointer of the C");
        assert_eq!(rom[0x38..0x3A], [0x98, 0x74], "pointer of the M");
        let others = (0x20..0x54).filter(|&i| !(0x24..0x26).contains(&i) && !(0x38..0x3A).contains(&i));
        assert!(others.into_iter().all(|i| rom[i] == 0), "the other letters without words");
        assert_eq!(
            rom[0x54..0x7D],
            [
                0x95, b'C', b'H', b'A', b'I', b'N', 0xF0, 0xB2, 0x98, 0x80, //
                0xA5, b'C', b'L', b'O', b'A', b'D', 0xF0, 0x89, 0x98, 0x83, //
                0x85, b'C', b'S', b'A', b'V', b'E', 0xF0, 0x95, 0x98, 0x87, //
                0xD5, b'M', b'E', b'R', b'G', b'E', 0xF0, 0x8F, 0x98, 0x8A, //
                0x90,
            ],
            "entries and terminator"
        );
        for trap in [0x80, 0x84, 0x87, 0x8B, 0x92] {
            assert_eq!(rom[trap..trap + 3], ERROR_27, "fallback at +{trap:02X}");
        }
        assert_eq!(rom[0x83], 0x60, "mask of CLOAD");
        assert_eq!(rom[0x8A], 0x60, "mask of MERGE");
        assert_eq!(rom[0x90..0x92], [0xCD, 0x36], "VMJ ($36)");
        assert_eq!(rom[0x98..0x9F], [0xBE, 0xD0, 0xB4, 0xBE, 0xD0, 0x0D, 0xE2], "after loading");
        assert_eq!(rom[0xA0], 0xA5, "CHAIN_GO");
        assert!(rom[0xB0..].iter().all(|&b| b == 0xFF), "rest empty");
        assert_eq!(rom_byte(0x9800), 0x55, "rom_byte");
        assert_eq!(rom_byte(0xA000), 0xFF, "outside the table");
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
            "entries"
        );
        assert_eq!(table.first_for_letter(b'C'), Some(0), "C");
        assert_eq!(table.first_for_letter(b'M'), Some(3), "M");
        assert_eq!(table.first_for_letter(b'A'), None, "A");
    }

    #[test]
    fn csave_names_and_formats() {
        assert_eq!(csave_file_name("X", false), "X.BAS", "CSAVE");
        assert_eq!(csave_file_name("X", true), "X.ASC", "CSAVEa");
        assert_eq!(csave_file_name("x.bin", true), "x.bin", "explicit extension");
        assert_eq!(csave_format("X.BAS", false), SaveFormat::Ce158, "CE-158");
        assert_eq!(csave_format("X.ASC", true), SaveFormat::AsciiCr, "CSAVEa");
        assert_eq!(csave_format("X.ASC", false), SaveFormat::Ce158, ".asc without a: CE-158 rule");
        assert_eq!(csave_format("X.bas", true), SaveFormat::AsciiCrLf, ".bas with a: CRLF");
        assert_eq!(csave_format("X.Bin", true), SaveFormat::Tokenized, ".bin rules");
        assert_eq!(csave_format("X.TXT", false), SaveFormat::AsciiCrLf, ".txt rules");
        assert_eq!(csave_format("X.DAT", false), SaveFormat::Ce158, "another extension");
        assert_eq!(csave_format("tape", true), SaveFormat::AsciiCrLf, "tape without extension, with a");
    }

    #[test]
    fn cload_candidate_order() {
        assert_eq!(cload_candidates("X", false), ["X.BAS", "X.ASC", "X.TXT", "X.BIN", "X"], "without a");
        assert_eq!(cload_candidates("X", true), ["X.ASC", "X.BAS", "X.TXT", "X.BIN", "X"], "with a");
        assert_eq!(cload_candidates("X.bin", false), ["X.bin"], "with extension");
    }

    #[test]
    fn names_are_sandboxed() {
        assert_eq!(validate_name(b"MOLE"), Ok("MOLE".to_owned()), "normal");
        assert_eq!(validate_name(b"t1.bas"), Ok("t1.bas".to_owned()), "with extension");
        assert_eq!(validate_name(b"MY GAME"), Ok("MY GAME".to_owned()), "with a space");
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
        assert_eq!(check_ascii_lines("' comment\r\n\r\n10 PRINT 1\r\n\x1A"), Ok(()), "valid");
        assert_eq!(check_ascii_lines("   20 A=1\r"), Ok(()), "leading spaces");
        let error = check_ascii_lines("10 PRINT 1\nPRINT 2\n").map_err(|e| e.code);
        assert_eq!(error, Err(ERR_ASCII_LINE), "line without a number");
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
        assert_eq!(contents, Some(b"x".to_vec()), "ignoring case: {found:?}");
        let dir_only = find_case_insensitive(path, "sub.bas").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(dir_only, None, "folders do not count");
        write_atomically(&path.join("Mole.Bas"), b"new").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(fs::read(path.join("Mole.Bas")).ok(), Some(b"new".to_vec()), "replaced");
        let leftovers = fs::read_dir(path)
            .map(|d| d.filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().ends_with(".ceres-tmp")).count())
            .unwrap_or_default();
        assert_eq!(leftovers, 0, "no temporary files left");
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
            "CSAVE \u{2192} /d/T1.BAS (tokenized with CE-158 header, 20 lines, 1007 bytes)",
            "saved"
        );
        assert_eq!(saved.error_code(), None, "no error");
        let error = HostEvent::Error { command: HostCommand::Cload, code: 61, message: "it does not exist".to_owned() };
        assert_eq!(error.to_string(), "CLOAD: ERROR 61 \u{2014} it does not exist", "error");
        assert_eq!(error.error_code(), Some(61), "code");
    }
}
