//! Loads a tokenized BASIC program (a `.bin` file) as the program of the
//! PC-1500's own BASIC, to run the original of a program next to its
//! compiled `.lh5`. It takes the dull compiler's default output (`dull
//! game.bas -o game.bin`, with the CE-158's 27-byte `@COM` header and the
//! `&FF` end marker) and lhasm's (neither).

use std::{fs, io, path::Path};

/// Where BASIC's program starts on a PC-1500 with the CE-161 after `NEW0`
/// (the ROM keeps its reserve area at `&0008-&00C4`): the address lhasm's
/// `-m ce161` and the compiler write in the header.
pub const BASIC_START: u16 = 0x00C5;

/// Bytes of the `@COM` header: `&01`, `@COM`, the name (16 bytes), the start
/// address, the length - 1 and the entry address (16 bits each, high byte
/// first).
const HEADER_LEN: usize = 27;

/// Errors loading a BASIC program.
#[derive(Debug)]
pub enum BasicLoadError {
    /// An I/O error reading the file.
    IoError(io::Error),
    /// The file holds no program.
    Empty,
    /// The program does not fit in user memory from its start address.
    TooBig,
    /// The start address is outside user memory.
    InvalidStartAddress,
}

impl From<io::Error> for BasicLoadError {
    fn from(err: io::Error) -> Self {
        Self::IoError(err)
    }
}

impl std::fmt::Display for BasicLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "I/O error: {e}"),
            Self::Empty => write!(f, "The file holds no BASIC program"),
            Self::TooBig => write!(f, "The BASIC program is too big for user memory"),
            Self::InvalidStartAddress => write!(f, "The BASIC program's start address is outside user memory"),
        }
    }
}

impl std::error::Error for BasicLoadError {}

/// Reads a `.bin` file: its start address and the program's lines, without
/// the end marker.
pub fn read_basic_file(path: &Path) -> Result<(u16, Vec<u8>), BasicLoadError> {
    parse_basic(&fs::read(path)?)
}

/// As [`read_basic_file`], from the file's bytes: with the `@COM` header,
/// the start address is the header's; without it, [`BASIC_START`].
pub fn parse_basic(data: &[u8]) -> Result<(u16, Vec<u8>), BasicLoadError> {
    let (start, program) = if data.len() >= HEADER_LEN && data[0] == 0x01 && &data[1..5] == b"@COM" {
        (u16::from_be_bytes([data[21], data[22]]), &data[HEADER_LEN..])
    } else {
        (BASIC_START, data)
    };
    let program = program.strip_suffix(&[0xFF]).unwrap_or(program);
    if program.is_empty() {
        return Err(BasicLoadError::Empty);
    }
    Ok((start, program.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `10 END` as the PC-1500 stores it: line number, length, `END`, CR.
    const LINE: [u8; 6] = [0x00, 0x0A, 0x03, 0xF1, 0x8E, 0x0D];

    #[test]
    fn a_program_without_header_starts_where_basic_does() {
        assert_eq!(parse_basic(&LINE).unwrap(), (BASIC_START, LINE.to_vec()));
        let mut with_end = LINE.to_vec();
        with_end.push(0xFF);
        assert_eq!(parse_basic(&with_end).unwrap(), (BASIC_START, LINE.to_vec()));
    }

    #[test]
    fn the_header_gives_the_start_address() {
        let mut file = vec![0x01, b'@', b'C', b'O', b'M'];
        file.extend_from_slice(&[b'P'; 16]);
        file.extend_from_slice(&[0x40, 0xC5, 0x00, 0x06, 0xFF, 0xFF]);
        file.extend_from_slice(&LINE);
        file.push(0xFF);
        assert_eq!(parse_basic(&file).unwrap(), (0x40C5, LINE.to_vec()));
    }

    #[test]
    fn an_empty_file_is_an_error() {
        assert!(matches!(parse_basic(&[0xFF]), Err(BasicLoadError::Empty)));
    }
}
