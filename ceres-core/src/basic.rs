//! BASIC programs from and to the host.
//!
//! The PC-1500's program format in RAM, a tokenizer and a detokenizer that read the keywords from
//! the ROM's own tables, the host's file formats (ASCII text, "raw" tokenized dump and tokenized
//! `.BAS` with the CE-158's 27-byte header) and loading/extracting the program of the emulated
//! machine.
//!
//! # The program in RAM
//! Each line is `[no. hi][no. lo][len][body ... $0D]`, where `len` counts the body INCLUDING the
//! final `$0D` (line 5000 of `mole`: `13 88 07 22 4D 4F 4C 45 22 0D`). Valid numbers go from 1 to
//! 65279 ($FEFF): a high byte $FF is the end mark. The lines go in increasing order and the program
//! ends with ONE byte $FF at the address that `BASPRG_END` ($7867/68) keeps. The ROM finds the end
//! by that $FF, not by the pointer (`LINE_SEARCH_6` $D2FF: `LDA (SRCH_LINE_H)` / `CPI A,$FF`), and
//! `NEW` writes it ($C857 `ORI (X),$FF` and $C859 `BASPRG_END` = the address of that $FF). The
//! format has no absolute addresses (the targets of `GOTO` are ASCII digits or labels), so a
//! program can be loaded at any address.
//!
//! # The keywords come from the ROM
//! There is no keyword list written by hand to tokenize: [`KeywordTable`] reads the main ROM's
//! table ($C000) byte by byte, and [`Pc1500::basic_keywords`] also walks, in the SAME order as the
//! ROM's `TOK_INBUF` routine, all the expansion tables there are in the emulated machine ($B800,
//! $B000, ..., $8000 with `PV`=0 and then with `PV`=1). So a table added to the memory map (e.g.
//! the virtual table of `CLOAD`/`CSAVE` at $9800) is used by itself, without touching this module.
//! To list files of real machines with a CE-150/CE-158, the detokenizer also knows the names of
//! those peripherals' tokens ([`KeywordTable::peripherals`]).
//!
//! # The tokenizer is the ROM's
//! [`tokenize_body`] is a literal translation of `TOK_INBUF` ($F957), with its quirks: the
//! apostrophe is deleted everywhere (even inside strings), spaces outside strings disappear, spaces
//! INSIDE a keyword are skipped when comparing it (`G O T O` is `GOTO`), lowercase letters are
//! never tokenized, `.` abbreviates to the first word of the table that fits (`P.` = `PRINT`),
//! keywords are also found inside identifiers (`ATOB` = `A` `TO` `B`) and after `REM` the rest of
//! the line is copied as it is. It has been checked by typing on the emulated ROM all the lines of
//! the corpus of programs (3596 lines, 0 differences of bytes).

use core::{error::Error, fmt, fmt::Write as _, iter};
use std::{borrow::Cow, fs, path::Path, sync::OnceLock};

use crate::{Pc1500, memory};

// --- Addresses of the system RAM that this module uses -------------

/// `RAM_ST_H` ($7863): page where RAM starts, the one the `RESET` probe ($E098) finds. With the
/// emulator's CE-161, $00.
pub const RAM_ST_H: u16 = 0x7863;
/// `RAM_END_H` ($7864): page after the end of RAM. With the CE-161, $48 (RAM ends at $47FF); $58 on
/// a PC-1500A.
pub const RAM_END_H: u16 = 0x7864;
/// `BASPRG_ST` ($7865/66): first line of the BASIC program.
pub const BASPRG_ST: u16 = 0x7865;
/// `BASPRG_END` ($7867/68): address of the $FF that closes the program.
pub const BASPRG_END: u16 = 0x7867;
/// `BASPRG_EDT` ($7869/6A): where `LINE_SEARCH` ($D2E0) starts looking for lines. It is the same as
/// `BASPRG_ST` except after a `MERGE`.
pub const BASPRG_EDT: u16 = 0x7869;
/// `BREAKPARAM` ($788A): running state. Measured: $00 at the prompt, $50 waiting for an `INPUT`,
/// $20 in the pause of a `PRINT`, $80 after `STOP`/BREAK (what `CONT` looks at, $C8CA
/// `BII (BREAKPARAM),$E0`).
pub const BREAKPARAM: u16 = 0x788A;
/// `VAR_START` ($7899/9A): base of the dimensioned variables, which grow down from `RAM_END_H`:00
/// (`DEL_DIM_VAR`, $D091).
pub const VAR_START: u16 = 0x7899;
/// `DATA_PTR` ($78BE/BF): pointer of `READ`, `(start + 3) | $8000` after `INIT_SYS_ADDR_2`
/// ($CFD2-$CFDA).
pub const DATA_PTR: u16 = 0x78BE;

/// Bits of [`BREAKPARAM`] that say the ROM is NOT at the prompt but inside a program (`INPUT` $50,
/// pause of `PRINT` $20). Bit 7 ($80, after `STOP`/BREAK) does not count: the prompt after a `STOP`
/// is a normal prompt.
const BREAKPARAM_BUSY: u8 = 0x70;

/// Low byte of the start address that `NEW0` sets (`BCMD_NEW_4` $C840-$C853: `RAM_ST_H`:$C5, right
/// after the area of reserved keys `RAM_ST_H`:$08-$C4).
const NEW0_START_LOW: u8 = 0xC5;

// --- Bytes and tokens -----------------------------------------------------

/// End of line in the input buffer and in the program.
const CR: u8 = 0x0D;
/// End of program mark (high byte of "line number" $FF).
const END_MARK: u8 = 0xFF;
/// Highest valid line number: $FF as the high byte is the end mark.
pub const MAX_LINE_NUMBER: u16 = 0xFEFF;
/// Signature of a keyword table in its first byte ($C000 = $55).
const TABLE_SIGNATURE: u8 = 0x55;
/// Token of `REM`: after it, the rest of the line goes raw.
pub const TOKEN_REM: u16 = 0xF1AB;
/// Token of `PI` (the keyboard's π character).
pub const TOKEN_PI: u16 = 0xF15D;
/// Token of `SQR` (the keyboard's √ character).
pub const TOKEN_SQR: u16 = 0xF16B;
/// Longest body (with the `$0D`) that can be edited on the machine.
///
/// The input buffer is 80 characters and `FIND_NEXT_LINE` ($DF72, `LDI UL,$4C`) only looks for the
/// `$0D` in 77 bytes, which `REM` ($C676) depends on.
pub const MAX_EDITABLE_BODY: usize = 79;
/// Header of a CE-158 `.BAS` file: 27 bytes.
pub const CE158_HEADER_LEN: usize = 27;
/// Signature of the CE-158 header (bytes 0-4): `$01 "@COM"`.
const CE158_MAGIC: [u8; 5] = [0x01, b'@', b'C', b'O', b'M'];
/// Bytes of the name in the CE-158 header (bytes 5-20, padded with NUL).
const CE158_NAME_LEN: usize = 16;

// --- Errors and warnings ---------------------------------------------------

/// Why a program or a keyword table could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BasicError {
    /// The keyword table at `base` does not have the ROM's format.
    BadKeywordTable { base: u16, reason: &'static str },
    /// A token (byte >= $E0) inside a string: `TOK_INBUF`'s `ERROR 28` ($F9A5, $FA2D `LDI UH,$1C`).
    /// `line` is `None` if a single body was tokenized.
    TokenInString { line: Option<u16> },
    /// A character of the text that does not exist on the PC-1500 (column from 1, in characters of
    /// the text line).
    BadCharacter { line: u16, column: usize, ch: char },
    /// A line number outside 1..=65279 (`text_line` is the line of the text file, from 1).
    BadLineNumber { text_line: usize, number: u64 },
    /// A line whose tokenized body does not fit in the length byte (more than 254 bytes plus the
    /// `$0D`).
    LineTooLong { line: u16, len: usize },
    /// A badly formed tokenized program at `offset`.
    BadTokenized { offset: usize, reason: &'static str },
    /// A CE-158 header that is not one.
    BadCe158Header(&'static str),
}

impl fmt::Display for BasicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadKeywordTable { base, reason } => {
                write!(f, "keyword table at ${base:04X} not valid: {reason}")
            }
            Self::TokenInString { line: Some(line) } => {
                write!(f, "line {line}: token inside a string (ERROR 28)")
            }
            Self::TokenInString { line: None } => {
                f.write_str("token inside a string (ERROR 28)")
            }
            Self::BadCharacter { line, column, ch } => write!(
                f,
                "line {line}, column {column}: the character '{ch}' does not exist on the PC-1500 \
                 (use \\xNN for a raw byte)"
            ),
            Self::BadLineNumber { text_line, number } => write!(
                f,
                "line {text_line} of the text: line number {number} outside 1..={MAX_LINE_NUMBER}"
            ),
            Self::LineTooLong { line, len } => write!(
                f,
                "line {line}: {len} tokenized bytes, more than the 255 a line takes"
            ),
            Self::BadTokenized { offset, reason } => {
                write!(f, "tokenized program not valid at byte {offset}: {reason}")
            }
            Self::BadCe158Header(reason) => write!(f, "CE-158 header not valid: {reason}"),
        }
    }
}

impl Error for BasicError {}

/// Something odd but not fatal found while reading a program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// BASIC line number, if the warning is about a specific line.
    pub line: Option<u16>,
    pub kind: WarningKind,
}

/// See [`Warning`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WarningKind {
    /// Text lines that do not start with a number (`'` header comments of the corpus...) and have
    /// been skipped. Empty ones do not count.
    SkippedNonProgramLines(usize),
    /// The line number was already there: the last one wins, as in the editor.
    DuplicateLine,
    /// The line had apostrophes outside `REM`: the ROM always deletes them, also inside strings
    /// ($F962 `CPI A,$27`).
    ApostropheDeleted,
    /// Body (with the `$0D`) of more than [`MAX_EDITABLE_BODY`] bytes: it loads and runs, but it
    /// cannot be edited on the machine.
    BodyLongerThan79 { len: usize },
    /// The lines of a tokenized file are not in increasing order (they are loaded as they are; the
    /// ROM will not find some of them properly).
    UnsortedLines,
    /// The CE-158 header announces more bytes than the file has.
    Ce158Truncated { declared: usize, present: usize },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "line {line}: ")?;
        }
        match &self.kind {
            WarningKind::SkippedNonProgramLines(n) => {
                write!(f, "{n} text lines without a number ignored")
            }
            WarningKind::DuplicateLine => f.write_str("repeated number, the last one wins"),
            WarningKind::ApostropheDeleted => {
                f.write_str("the PC-1500 deletes the apostrophes (') outside REM")
            }
            WarningKind::BodyLongerThan79 { len } => write!(
                f,
                "{len} bytes, more than {MAX_EDITABLE_BODY}: it cannot be edited on the machine"
            ),
            WarningKind::UnsortedLines => f.write_str("lines out of order"),
            WarningKind::Ce158Truncated { declared, present } => write!(
                f,
                "the CE-158 header announces {declared} bytes and the file has {present}"
            ),
        }
    }
}

// --- Keyword tables -------------------------------------------

/// An entry of one of the ROM's keyword tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyword {
    /// Address of its control byte (0 in the synthetic tables).
    pub addr: u16,
    /// The keyword's text (`b"PRINT"`).
    pub text: Vec<u8>,
    /// The 2-byte token ($F097 for `PRINT`).
    pub token: u16,
    /// Address of the routine that runs it (0 in the synthetic ones).
    pub vector: u16,
    /// Bit 4 of its attributes: it is the last word of its letter, and if it does not fit the
    /// tokenizer moves to the next table (`TOK_INBUF_25` $FA36-$FA3E: `ANI A,$10` /
    /// `BZS TOK_INBUF_16`).
    pub last_of_letter: bool,
}

/// A keyword table, as the ROM walks it.
///
/// # Format (checked on the A04's `.lst`)
/// - `base + $00` = $55 (signature).
/// - `base + $20 + 2·(letter-'A')`: big-endian pointer to the SECOND character of the first word of
///   that letter, or $0000 if there is none (in the main ROM H, J, K, Q, X, Y and Z). The ROM only
///   looks at the high byte.
/// - From `base + $54`, entries `[ctl][text][token hi][token lo] [vector hi][vector lo]`. The low
///   nibble of `ctl` is the length of the text; the high one are the attributes of the PREVIOUS
///   entry (macro `CNIB` of `PC-1500_Macros.lib`), so those of entry k are in the `ctl` of k+1.
/// - It ends with a `ctl` with low nibble 0 ($C34E = $D0 in the A04).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeywordTable {
    base: u16,
    pv: bool,
    entries: Vec<Keyword>,
    first_by_letter: [Option<usize>; 26],
    end: u16,
}

/// Tokens of the CE-150 (printer/cassette) and CE-158 (RS-232/parallel) peripherals, to LIST
/// programs of real machines that use them. They are those of the tables of those ROMs (CE-150
/// $B800, CE-158 $8000/$8800 with `PV`=1), which the emulator does not have.
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

/// Address of the main ROM's table.
pub const MAIN_TABLE_BASE: u16 = 0xC000;
/// First entry of a table (`base + $54`).
const FIRST_ENTRY_OFFSET: u16 = 0x54;
/// Pointers by letter (`base + $20`).
const LETTER_POINTERS_OFFSET: u16 = 0x20;
/// Limit of entries when walking a table: a broken table (or memory with no table) cannot keep the
/// walk from ending.
const MAX_TABLE_ENTRIES: usize = 512;

impl KeywordTable {
    /// Reads the table at `table_base` of a ROM image `rom` that starts at `rom_base`. Outside the
    /// image, $FF is read.
    pub fn parse(rom: &[u8], rom_base: u16, table_base: u16) -> Result<Self, BasicError> {
        Self::parse_with(table_base, false, |addr| {
            rom.get(usize::from(addr.wrapping_sub(rom_base)))
                .copied()
                .unwrap_or(0xFF)
        })
    }

    /// Reads the table at `base` reading each byte with `read` (e.g. from the emulated memory with
    /// whichever `PV` is due, see [`Pc1500::basic_keywords`]). `pv` is only kept, to know where it
    /// came from.
    pub fn parse_with<F: Fn(u16) -> u8>(base: u16, pv: bool, read: F) -> Result<Self, BasicError> {
        let bad = |reason| BasicError::BadKeywordTable { base, reason };
        if read(base) != TABLE_SIGNATURE {
            return Err(bad("the signature $55 is missing"));
        }
        let mut entries: Vec<Keyword> = Vec::new();
        let mut addr = base.wrapping_add(FIRST_ENTRY_OFFSET);
        // Attributes of each entry = high nibble of the `ctl` of the next one (or of the
        // terminator).
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
                return Err(bad("it does not end"));
            }
            let at = |offset: u8| read(addr.wrapping_add(u16::from(offset)));
            let text: Vec<u8> = (1..=len).map(at).collect();
            if text.iter().any(|&b| b >= 0xE0) {
                return Err(bad("keyword text with bytes >= $E0"));
            }
            let token = u16::from(at(len + 1)) << 8 | u16::from(at(len + 2));
            let vector = u16::from(at(len + 3)) << 8 | u16::from(at(len + 4));
            if token < 0xE100 {
                // The tokenizer recognizes the end of the text by a byte > $E0 ($F9F9 `CPI A,$E0`,
                // `CIN` of the '.' at $F9EF).
                return Err(bad("token below $E100"));
            }
            entries.push(Keyword { addr, text, token, vector, last_of_letter: false });
            let Some(next) = addr.checked_add(5 + u16::from(len)) else {
                return Err(bad("it goes beyond the memory"));
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
                return Err(bad("a letter pointer points at no entry"));
            };
            *slot = Some(index);
        }

        Ok(Self { base, pv, entries, first_by_letter, end: addr })
    }

    /// The table of the A04 main ROM ($C000), read only once from the emulator's ROM image.
    pub fn main_rom() -> &'static Self {
        static TABLE: OnceLock<KeywordTable> = OnceLock::new();
        TABLE.get_or_init(|| {
            Self::parse(memory::PC1500_ROM_BYTES, MAIN_TABLE_BASE, MAIN_TABLE_BASE)
                .unwrap_or_else(|_| Self::empty(MAIN_TABLE_BASE))
        })
    }

    /// The tokens of the CE-150/CE-158 peripherals (see `PERIPHERAL_KEYWORDS`) as a synthetic
    /// table: by default they are only used to detokenize (see [`Keywords`]). Within each letter
    /// they go in the order of the list; `addr` and `vector` are 0.
    pub fn peripherals() -> &'static Self {
        static TABLE: OnceLock<KeywordTable> = OnceLock::new();
        TABLE.get_or_init(|| Self::from_list(&PERIPHERAL_KEYWORDS))
    }

    /// Synthetic table from pairs (text, token), grouped by their first letter in the order given.
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

    /// The entries, in the order of the table.
    #[must_use]
    pub fn entries(&self) -> &[Keyword] {
        &self.entries
    }

    /// Index of the first entry of `letter` (`b'A'..=b'Z'`).
    #[must_use]
    pub fn first_for_letter(&self, letter: u8) -> Option<usize> {
        let index = usize::from(letter.checked_sub(b'A')?);
        *self.first_by_letter.get(index)?
    }

    /// Address of the table (0 in the synthetic ones).
    #[must_use]
    pub const fn base(&self) -> u16 {
        self.base
    }

    /// `PV` with which the table was read.
    #[must_use]
    pub const fn pv(&self) -> bool {
        self.pv
    }

    /// Address of the terminator byte (low nibble 0).
    #[must_use]
    pub const fn end(&self) -> u16 {
        self.end
    }

    /// The text of `token`: the first entry that has it, as `LIST`'s walk does (`TOK_PROCESS_5/6`
    /// $FAB7-$FAEF).
    #[must_use]
    pub fn name(&self, token: u16) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|e| e.token == token)
            .map(|e| e.text.as_slice())
    }
}

/// The set of tables with which it tokenizes (in search order) and detokenizes.
///
/// To tokenize only the search tables are used; to detokenize, these and then the names of the
/// peripherals ([`KeywordTable::peripherals`]), which the emulated machine does not have but files
/// of real machines may bring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keywords {
    search: Vec<KeywordTable>,
    peripherals_searched: bool,
}

impl Keywords {
    /// Only the main ROM's table: the same as the ROM does on an emulator with nothing mapped at
    /// $8000-$BFFF. For the emulated machine as it is, better [`Pc1500::basic_keywords`].
    #[must_use]
    pub fn main_rom() -> Self {
        Self::from_tables(vec![KeywordTable::main_rom().clone()])
    }

    /// The tables given, in the order the tokenizer will search them.
    #[must_use]
    pub const fn from_tables(search: Vec<KeywordTable>) -> Self {
        Self { search, peripherals_searched: false }
    }

    /// Adds the tokens of the CE-150/CE-158 peripherals also when TOKENIZING (after the other
    /// tables), to prepare files for a real machine that has them. It is not what the emulated ROM
    /// would do, which would leave them as single letters.
    #[must_use]
    pub fn with_peripherals(mut self) -> Self {
        if !self.peripherals_searched {
            self.search.push(KeywordTable::peripherals().clone());
            self.peripherals_searched = true;
        }
        self
    }

    /// The search tables, in order.
    #[must_use]
    pub fn tables(&self) -> &[KeywordTable] {
        &self.search
    }

    /// The text of `token` in the first table that has it (the search ones and then those of the
    /// peripherals).
    #[must_use]
    pub fn name(&self, token: u16) -> Option<&[u8]> {
        self.search
            .iter()
            .find_map(|t| t.name(token))
            .or_else(|| KeywordTable::peripherals().name(token))
    }

    /// [`tokenize_body`] with these tables.
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

/// Internal result of the tokenizer: the body and what is worth warning about.
struct Tokenized {
    body: Vec<u8>,
    apostrophes_deleted: bool,
}

/// Tokenizes the body of a line exactly as `TOK_INBUF` ($F957).
///
/// `src` is what comes after the line number, without `$0D`; the keywords are looked for in
/// `tables` in order. Reading past the end gives `$0D`, as the ROM's input buffer does.
///
/// The rules, in the ROM's order:
/// 1. `'` ($27) is always skipped ($F962), also in strings.
/// 2. `$0D` ends the line.
/// 3. `"` toggles the "inside a string" state and is copied ($F96E).
/// 4. Inside a string, a byte >= $E0 is `ERROR 28` ($F9A5) and the rest is copied as it is.
/// 5. Outside, the space is dropped ($F9AB).
/// 6. Whatever is below `A` or in $5B..$DF (lowercase letters included) is copied
///    ($F9AF/$F9B3/$F9B7).
/// 7. A byte >= $E0 is a token already present: 2 bytes are copied.
/// 8. A letter `A`-`Z` looks for a keyword: table by table, from the first entry of its letter,
///    comparing the rest of the text with what follows in the entry and skipping spaces and
///    apostrophes there ($F9E1/$F9E5). A `.` accepts the current entry ($F9E9). If it does not fit,
///    the next entry, unless it was the last of its letter, which moves to the next table. If there
///    is none, the letter is copied and it goes on with the next character: that is why there are
///    keywords inside identifiers.
/// 9. The token is written big-endian; if it is `REM` ($F1AB), the rest of the line is copied raw
///    ($FA0D-$FA1B).
pub fn tokenize_body(src: &[u8], tables: &[KeywordTable]) -> Result<Vec<u8>, BasicError> {
    tokenize(src, tables).map(|t| t.body)
}

fn tokenize(src: &[u8], tables: &[KeywordTable]) -> Result<Tokenized, BasicError> {
    let at = |i: usize| src.get(i).copied().unwrap_or(CR);
    let mut out = Vec::with_capacity(src.len());
    let mut y = 0;
    let mut in_string = false;
    // How far the tokenized part goes (the rest, after `REM`, is raw): for the apostrophes warning.
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
            // Token already present: the 2 bytes as they are (`LIN Y` of the low byte even if it is
            // the `$0D`, as the ROM).
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

/// The search of rule 8 of [`tokenize_body`]: the token and where to go on in `src`, or `None`. `y`
/// points at the character after the letter.
fn find_keyword(src: &[u8], y: usize, letter: u8, tables: &[KeywordTable]) -> Option<(u16, usize)> {
    let at = |i: usize| src.get(i).copied().unwrap_or(CR);
    for table in tables {
        let Some(mut index) = table.first_for_letter(letter) else {
            continue;
        };
        while let Some(entry) = table.entries.get(index) {
            // The comparison starts at the SECOND character of the text (where the letter's pointer
            // points).
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

// --- The program ----------------------------------------------------------

/// A program line: number and tokenized body WITHOUT the `$0D`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramLine {
    pub number: u16,
    pub body: Vec<u8>,
}

/// End of line when writing text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eol {
    /// `\r\n`, the convention of the corpus's `.bas` files (and of `.txt`).
    CrLf,
    /// `\r`, that of the CE-158(X)'s `.ASC` files.
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

/// Options of the ASCII text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AsciiOptions {
    /// `LIST` format (the default), in reading and writing:
    /// - When reading, a `:` right after the number is removed (the LCD shows `10:PRINT`) and, if
    ///   two spaces follow `REM`, one is (the LCD and this writer put a space after each keyword).
    /// - When writing, `REM` is followed by a space if the rest of the line starts with a space
    ///   (`REM  * JACKPOT*`, as the LCD).
    ///
    /// With `false` the text is taken as it would be typed on the machine (faithful to the ROM byte
    /// by byte).
    pub list_format: bool,
}

impl Default for AsciiOptions {
    fn default() -> Self {
        Self { list_format: true }
    }
}

/// The 27-byte header of a CE-158 tokenized `.BAS`.
///
/// `$01 "@COM"`, name (16 bytes, padded with NUL), start address, length − 1 and end address (that
/// of the `$FF`), all big-endian. It is what `CSAVE` writes in the CE-158's ROM: `SUB_9CEC` sends
/// the 27 bytes of `OUT_BUF` ($7B60-$7B7A), with the start at +$15 (from `PRGST2XREG`,
/// `STATUS_1M2_UREG` $DFFA of the A04: X = `BASPRG_ST`, U = `BASPRG_END` − 1 − X), the length − 1
/// at +$17 and `BASPRG_END` at +$19 ($9980). After it come the bytes of the program WITHOUT the
/// `$FF`: in Jeff Birt's files (`BATHY.BAS`, 933 = 27 + 906 bytes, length − 1 = $0389; `DUN`,
/// $1CBD) and in the RAM dump `BASPRG_ST..BASPRG_END`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ce158Header {
    /// Name, without the NUL padding.
    pub name: Vec<u8>,
    pub start: u16,
    pub size_minus_one: u16,
    pub end: u16,
}

/// A tokenized BASIC program: lines with number and body.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    lines: Vec<ProgramLine>,
}

impl Program {
    /// Program with these lines, in this order (without checking anything).
    #[must_use]
    pub const fn from_lines(lines: Vec<ProgramLine>) -> Self {
        Self { lines }
    }

    /// The lines.
    #[must_use]
    pub fn lines(&self) -> &[ProgramLine] {
        &self.lines
    }

    /// Bytes it takes in RAM, without the final `$FF`.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.lines.iter().map(|l| l.body.len() + 4).sum()
    }

    /// Reads a tokenized program: lines up to the end of the bytes or up to a high byte $FF
    /// (whatever follows is ignored). Each body must end in `$0D` and each number be in 1..=65279;
    /// the order only gives a warning.
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
                return Err(bad(offset, "line cut short"));
            };
            let number = u16::from(hi) << 8 | u16::from(lo);
            if number == 0 {
                return Err(bad(offset, "line number 0"));
            }
            if len == 0 {
                return Err(bad(offset + 2, "length 0"));
            }
            let body_start = offset + 3;
            let Some(body) = bytes.get(body_start..body_start + usize::from(len)) else {
                return Err(bad(offset, "line cut short"));
            };
            let Some((&CR, body)) = body.split_last() else {
                return Err(bad(body_start + usize::from(len) - 1, "the body does not end in $0D"));
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

    /// The bytes of the program as they go in RAM, WITHOUT the final `$FF` (as the corpus's `.bin`
    /// files and what there is between `BASPRG_ST` and `BASPRG_END`).
    #[must_use]
    pub fn to_tokenized(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.byte_len());
        for line in &self.lines {
            out.push((line.number >> 8) as u8);
            out.push((line.number & 0xFF) as u8);
            // `from_ascii` does not let through bodies of more than 254 bytes; with `from_lines` it
            // is up to whoever builds it.
            out.push(u8::try_from(line.body.len() + 1).unwrap_or(u8::MAX));
            out.extend_from_slice(&line.body);
            out.push(CR);
        }
        out
    }

    /// Reads a text listing.
    ///
    /// - Lines end in CRLF, CR (the CE-158's `.ASC` files) or LF.
    /// - Leading spaces and tabs are removed. A line that does not start with a digit is skipped
    ///   (and counted in a warning): the corpus's `'` headers.
    /// - The number is the leading string of digits (1..=65279). With
    ///   [`AsciiOptions::list_format`], a `:` right after it is removed.
    /// - π is the `PI` token and √ that of `SQR`; the tab is a space; `\xNN` (two hexadecimal
    ///   digits) is the byte $NN as it is; the rest of printable ASCII goes through the same. Any
    ///   other character is an error with its line and column.
    /// - A repeated number replaces the previous one (as the editor does) and the lines are sorted.
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

    /// The listing as text: `{number} {body}` and `eol` per line, with each keyword followed by a
    /// space, as `LIST` (`70 A=RND (5)+CHR$ 65`, `IF A=&FFGOTO 5120`). Bytes outside $20..=$7E, the
    /// backslash and unknown tokens are written as `\xNN`. [`Program::from_ascii`] with the same
    /// options turns it back into exactly the same bytes.
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

    /// The listing in the format of the CE-158(X)'s `.ASC` files (`CSAVEa`, Jeff Birt's
    /// `BATHY.ASC`/`HEY.ASC`): number right-aligned in 5 characters, a space, the body, CR, and one
    /// more CR at the end. It is read with [`Program::from_ascii`] like any other.
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

    /// Reads a `.BAS` with a CE-158 header (see [`Ce158Header`]), leniently: the signature
    /// `$01 "@COM"` is required, at most `length − 1 + 1` bytes after the header are taken and the
    /// final `$FF` is optional (Jeff Birt's files do not have it; others, with length − 1 counting
    /// the `$FF`, do). The addresses are not used.
    pub fn from_ce158(bytes: &[u8]) -> Result<(Self, Ce158Header, Vec<Warning>), BasicError> {
        if bytes.len() < CE158_HEADER_LEN {
            return Err(BasicError::BadCe158Header("less than 27 bytes"));
        }
        if !bytes.starts_with(&CE158_MAGIC) {
            return Err(BasicError::BadCe158Header("it does not start with $01 \"@COM\""));
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

    /// The program with the CE-158 header that `CSAVE` would write on the real machine loaded at
    /// `start` (see [`Ce158Header`]): `name` truncated to 16 bytes (anything not printable ASCII
    /// becomes `_`), length − 1, `start + length` as the end address and the bytes of the program
    /// without the `$FF`.
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

/// The 2 bytes of a token, in the order they go in the program.
const fn token_bytes(token: u16) -> [u8; 2] {
    [(token >> 8) as u8, (token & 0xFF) as u8]
}

/// Splits into lines by CRLF, CR or LF.
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

/// Turns the text of a line (after the number) into the bytes that would be typed (see
/// [`Program::from_ascii`]). `column_base` is how many characters of the text line come before, for
/// the errors.
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

/// Rule of [`AsciiOptions::list_format`] for `REM`: if two spaces follow the token, one is removed
/// (the one `LIST` puts after the keyword).
fn collapse_rem_double_space(body: &mut Vec<u8>) {
    if let Some(rest) = rem_rest_offset(body)
        && body.get(rest..rest + 2) == Some(b"  ")
    {
        body.remove(rest);
    }
}

/// Position of the first byte after the `REM` token of a tokenized body, if any (outside strings,
/// as the interpreter would see it).
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

/// Adds to `out` the body `body` as the ROM lists it (see [`Program::to_ascii`]).
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

/// Printable ASCII as it is; the rest (and `\`) as `\xNN`.
fn push_escaped(out: &mut String, bytes: &[u8]) {
    for &b in bytes {
        if (0x20..=0x7E).contains(&b) && b != b'\\' {
            out.push(char::from(b));
        } else {
            write!(out, "\\x{b:02X}").ok();
        }
    }
}

/// Text of a file: UTF-8 if it is (without BOM); otherwise, Latin-1 (the corpus's comments have
/// stray bytes that are not UTF-8).
#[must_use]
pub fn decode_text(bytes: &[u8]) -> Cow<'_, str> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    str::from_utf8(bytes).map_or_else(
        |_| Cow::Owned(bytes.iter().map(|&b| char::from(b)).collect()),
        Cow::Borrowed,
    )
}

// --- File formats ----------------------------------------------------

/// Format of a program file when reading it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasicFormat {
    /// Text listing.
    Ascii,
    /// Tokenized bytes as they go in RAM (with or without the final `$FF`).
    Tokenized,
    /// Tokenized with the CE-158's 27-byte header.
    Ce158,
}

/// Format of a program file when writing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFormat {
    /// Text with CRLF (`.txt`, the corpus's convention).
    AsciiCrLf,
    /// Text with CR in the style of the CE-158(X)'s `.ASC` files (`.asc`).
    AsciiCr,
    /// Tokenized bytes without header or `$FF` (`.bin`).
    Tokenized,
    /// Tokenized with a CE-158 header (`.bas` and no extension: what `CSAVE "X"` does on the
    /// CE-158).
    Ce158,
}

impl SaveFormat {
    /// The format due by the extension (without the dot, upper or lower case): `.asc` CR,
    /// `.txt`/`.sbs` CRLF, `.bin`/`.tok`/`.img` tokenized and the rest (`.bas`, no extension)
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

    /// [`SaveFormat::for_extension`] with the extension of `path`.
    #[must_use]
    pub fn for_path(path: &Path) -> Self {
        Self::for_extension(path.extension().and_then(|e| e.to_str()))
    }
}

/// Works out the format of a program file by its contents (and the extension, without the dot):
/// - `$01 "@COM"` at the start is CE-158, whatever the extension.
/// - `.bin`, `.tok` and `.img` are tokenized.
/// - Otherwise, a file with control bytes that do not appear in a text (any below $20 except tab,
///   CR, LF, FF and the end-of-file Ctrl-Z) that reads whole as a tokenized program is tokenized: a
///   tokenized program almost always has them (the high byte of line numbers below 8192, the short
///   lengths).
/// - The rest, text.
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

/// A program read from a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedProgram {
    pub program: Program,
    pub format: BasicFormat,
    pub warnings: Vec<Warning>,
}

/// Reads the bytes of a program file in `format` (see [`detect_format`]).
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

/// The bytes of the file of `program` in `format`. `name` (the name of the CE-158 header) and
/// `start` (its start address) only count for [`SaveFormat::Ce158`].
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

// --- The emulated machine ---------------------------------------------------

/// Why a program could not be loaded into the machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BasicLoadError {
    /// The ROM is not waiting for the keyboard at BASIC's prompt (it is running, in an `INPUT`, in
    /// the pause of a `PRINT`, off...). The memory has not been touched.
    NotAtPrompt,
    /// `RAM_ST_H`/`RAM_END_H` do not describe a RAM (the ROM has not booted yet).
    NoRam,
    /// The program (with its `$FF`) needs `need` bytes and from its start to the end of RAM there
    /// are `avail`. The memory has not been touched.
    TooBig { need: usize, avail: usize },
    /// The file is not a valid program.
    Format(BasicError),
    /// The file could not be read.
    Io(String),
}

impl fmt::Display for BasicLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAtPrompt => f.write_str(
                "the PC-1500 is not at the BASIC prompt (press CL or BREAK)",
            ),
            Self::NoRam => f.write_str("the PC-1500 has not booted yet"),
            Self::TooBig { need, avail } => write!(
                f,
                "the program needs {need} bytes and there are only {avail} free"
            ),
            Self::Format(e) => e.fmt(f),
            Self::Io(e) => write!(f, "read error: {e}"),
        }
    }
}

impl Error for BasicLoadError {}

impl From<BasicError> for BasicLoadError {
    fn from(e: BasicError) -> Self {
        Self::Format(e)
    }
}

/// Why the machine's program could not be saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BasicSaveError {
    /// The program in RAM is corrupt.
    Program(BasicError),
    /// The file could not be written.
    Io(String),
}

impl fmt::Display for BasicSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program(e) => write!(f, "program in memory not valid: {e}"),
            Self::Io(e) => write!(f, "write error: {e}"),
        }
    }
}

impl Error for BasicSaveError {}

/// What [`Pc1500::load_basic_program`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadReport {
    /// `BASPRG_ST`: where the program starts.
    pub start: u16,
    /// `BASPRG_END`: the address of its `$FF`.
    pub end: u16,
    /// Bytes of the program, without the `$FF`.
    pub bytes: usize,
    /// Lines.
    pub lines: usize,
    /// Free bytes between the `$FF` and the end of RAM.
    pub free: usize,
    /// Warnings when reading the file (empty with [`Pc1500::load_basic_program`]).
    pub warnings: Vec<Warning>,
}

/// What [`Pc1500::save_basic_file`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveReport {
    pub format: SaveFormat,
    /// Bytes written to the file.
    pub bytes: usize,
    /// Lines of the program.
    pub lines: usize,
}

/// Pages of the expansion tables that the ROM walks after the main one, with `PV`=0 and then with
/// `PV`=1 (`DEC_OPN` $FA58: steps of 2 KB, $B8, $B0, ..., $80).
const EXPANSION_TABLE_PAGES: [u8; 8] = [0xB8, 0xB0, 0xA8, 0xA0, 0x98, 0x90, 0x88, 0x80];

impl Pc1500 {
    /// The keyword tables of the emulated machine, in the order the ROM's tokenizer searches them
    /// (`TOK_INBUF` $F9C3: the main one at $C000 and then, with `DEC_OPN`, each table with the
    /// signature $55 at $B800, $B000, ..., $8000 with `PV`=0 and again with `PV`=1). They are read
    /// from the emulated memory, so they include any expansion table that is mapped. A table that
    /// cannot be read is skipped.
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

    /// A byte of ME0 at $8000-$BFFF as the CPU would see it with `pv`, whatever `PV` it has now
    /// (see `Pc1500::read_banked`, in `memory.rs`: today there is only the virtual table of
    /// `CLOAD`/`CSAVE` at $9800 with `PV`=0). Outside that area `PV` changes nothing.
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

    /// Fixes the boot program that `MemoryBus::new()` puts in RAM (bathyscaph at $00C5, see
    /// [`Pc1500::new`]) so that it is a valid BASIC program, as if it had been loaded with
    /// [`Pc1500::load_basic_program`]:
    ///
    /// - Writes the end `$FF` at `BASPRG_END` (if the file does not have it). Without it, the ROM
    ///   goes on looking for lines in the zeros behind it: typing a new last line after booting put
    ///   it in an absurd place and `LIST` showed garbage (`65535:~~ ...`).
    /// - Sets `VAR_START` = the end of RAM (`RAM_END_H`:00, $4800 on a PC-1500 and $5800 on a
    ///   PC-1500A), what `DEL_DIM_VAR` ($D091) would do. The cold start does not do it (only `RUN`,
    ///   `NEW` and `CLEAR` do), so it stayed at $FFFF and a `DIM` in direct mode, or a `GOTO`
    ///   without `RUN`, used an absurd base.
    ///
    /// The boot screen (`NEW0? :CHECK`), `LIST` and `RUN` do not change; in memory those 3 bytes
    /// do.
    pub(crate) fn fix_boot_fixture(&mut self) {
        let start = self.read_word(BASPRG_ST);
        let end = self.read_word(BASPRG_END);
        let user_end = self.user_memory_end().wrapping_add(1);
        if start <= end && end < user_end {
            self.write_byte(u32::from(end), END_MARK);
        }
        self.write_word(VAR_START, user_end & 0xFF00);
    }

    /// `true` if the ROM is at BASIC's prompt waiting for the keyboard, the only moment the program
    /// can be changed from outside: the CPU asleep in `WAIT_4_KB` ([`Pc1500::is_waiting_for_key`])
    /// and [`BREAKPARAM`] without the "inside a program" bits ($50 in an `INPUT`, $20 in the pause
    /// of a `PRINT`). After a `STOP` or a BREAK ($80) it does count as a prompt.
    #[must_use]
    pub fn is_at_basic_prompt(&self) -> bool {
        self.is_waiting_for_key() && self.read_byte(u32::from(BREAKPARAM)) & BREAKPARAM_BUSY == 0
    }

    /// Loads `program` as the machine's BASIC program, replacing whatever there was, as typing it
    /// after a `NEW` would leave it (checked with `LIST`, the up/down arrows, editing lines, `RUN`,
    /// `CONT` and `MODE`):
    ///
    /// 1. Only at the prompt ([`Pc1500::is_at_basic_prompt`]); otherwise,
    ///    [`BasicLoadError::NotAtPrompt`] without touching anything.
    /// 2. It starts at the current `BASPRG_ST`, or at `RAM_ST_H`:$C5 (what `NEW0` sets) if that is
    ///    not inside the RAM.
    /// 3. It has to fit with its `$FF` before `RAM_END_H`:00 (`PRGLINE_TDI_6` $CF7F
    ///    `CPA (RAM_END_H)`); otherwise, [`BasicLoadError::TooBig`] without touching anything.
    /// 4. It writes the bytes and the `$FF`, `BASPRG_ST` = `BASPRG_EDT` = start and `BASPRG_END` =
    ///    the address of the `$FF` (as `NEW`, $C853-$C85C).
    /// 5. `VAR_START` = `RAM_END_H`:00 (`DEL_DIM_VAR`, $D091).
    /// 6. The part of `INIT_SYS_ADDR` ($CFCC-$D00C) that makes sense with the ROM waiting for the
    ///    keyboard: `DATA_PTR` = (start + 3) | $8000; $78A2/A3, $78B4/B5, `ERL` ($789B) and
    ///    $7895-$7898 to 0; `ON_ERR_ADD_H` ($78B8) |= $80; `CURS_CTRL` ($7B0E) &= $1F; `BREAKPARAM`
    ///    = 0 (so `CONT` gives `ERROR 1` instead of going on inside the old program);
    ///    `GOSB_STK_PTR_L` ($7891) = $FF and `FORNXT_STK_PTR` ($7890) = $38.
    ///
    /// NOT touched: $7871-$787E (the cursor and its blinking, in use inside `WAIT_4_KB`), the line
    /// search addresses, $789C-$78B7 (filling them with $FF, as the rest of `INIT_SYS_ADDR`, broke
    /// the `MODE` and up keys), the fixed variables, `TRON` or `WAIT`. `ROM_ST` stays at $FFFF.
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

    /// Where a program that replaces the current one starts and where the RAM ends
    /// (`RAM_END_H`:00): steps 2 and 3 of [`Pc1500::load_basic_program`]. `CLOAD` uses it too (see
    /// `host_io.rs`).
    pub(crate) fn program_load_area(&self) -> Result<(u16, u16), BasicLoadError> {
        let ram_st_h = self.read_byte(u32::from(RAM_ST_H));
        let ram_end_h = self.read_byte(u32::from(RAM_END_H));
        if ram_end_h <= ram_st_h || ram_end_h == 0xFF {
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

    /// Writes `bytes` (a program without `$FF`) at `start`, the `$FF` after it and the pointers
    /// `BASPRG_ST`/`BASPRG_EDT`/`BASPRG_END`, as `NEW` ($C853-$C85C). It returns the address of the
    /// `$FF`.
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

    /// See step 6 of [`Pc1500::load_basic_program`].
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

    /// Reads `bytes` (see [`detect_format`], `ext` without the dot) with the machine's keywords
    /// ([`Pc1500::basic_keywords`]) and loads it with [`Pc1500::load_basic_program`]. The warnings
    /// go in the [`LoadReport`].
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

    /// [`Pc1500::load_basic_bytes`] of a file, with the default options.
    pub fn load_basic_file(&mut self, path: &Path) -> Result<LoadReport, BasicLoadError> {
        let bytes = fs::read(path).map_err(|e| BasicLoadError::Io(e.to_string()))?;
        let ext = path.extension().and_then(|e| e.to_str());
        self.load_basic_bytes(&bytes, ext, &AsciiOptions::default())
    }

    /// The bytes of the machine's BASIC program without the `$FF`: the lines from `BASPRG_ST` up to
    /// the one that starts with $FF, not going past `BASPRG_END` (nor the end of RAM if it makes no
    /// sense).
    #[must_use]
    pub fn basic_program_bytes(&self) -> Vec<u8> {
        let start = self.read_word(BASPRG_ST);
        let end = self.read_word(BASPRG_END);
        let ram_end = (u16::from(self.read_byte(u32::from(RAM_END_H))) << 8)
            .min(self.user_memory_end().wrapping_add(1));
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

    /// The machine's BASIC program (see [`Pc1500::basic_program_bytes`]).
    pub fn basic_program(&self) -> Result<Program, BasicError> {
        Program::from_tokenized(&self.basic_program_bytes()).map(|(program, _)| program)
    }

    /// Saves the machine's BASIC program to `path`, in `format` or, if `None`, in the one due by
    /// the extension ([`SaveFormat::for_path`]: `.bas` CE-158, `.asc` CR text, `.txt` CRLF text,
    /// `.bin` tokenized). The CE-158 header carries the file's name and the current start address.
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
        assert_eq!(with_blank, ["a", ""], "empty line in between");
    }

    #[test]
    fn escapes_round_trip() {
        let src = encode_text("A\\x0D\\xF0\\y", 10, 0).unwrap_or_default();
        assert_eq!(src, b"A\x0D\xF0\\y", "\\xNN and a lone backslash");
        let mut out = String::new();
        push_escaped(&mut out, b"A\\\x01~");
        assert_eq!(out, "A\\x5C\\x01~", "escape of non-printable bytes");
    }

    #[test]
    fn save_format_by_extension() {
        assert_eq!(SaveFormat::for_extension(Some("BAS")), SaveFormat::Ce158, ".bas");
        assert_eq!(SaveFormat::for_extension(None), SaveFormat::Ce158, "no extension");
        assert_eq!(SaveFormat::for_extension(Some("asc")), SaveFormat::AsciiCr, ".asc");
        assert_eq!(SaveFormat::for_extension(Some("txt")), SaveFormat::AsciiCrLf, ".txt");
        assert_eq!(SaveFormat::for_extension(Some("bin")), SaveFormat::Tokenized, ".bin");
    }
}
