//! Loads files of LH5801 machine code (.lh5).
//!
//! The file:
//! - Bytes 0-1: the load address (u16, little-endian)
//! - Bytes 2-3: the length of the code (u16, little-endian)
//! - Bytes 4+:  the LH5801 machine code

use std::{fs, io, path::Path};

/// Errors loading .lh5 files
#[derive(Debug)]
pub enum Lh5LoadError {
    /// An I/O error reading the file
    IoError(io::Error),
    /// The file is too small (under the 4 bytes of the header)
    FileTooSmall,
    /// The format is wrong (code_length is not the real size)
    InvalidFormat,
    /// The code is too big (more than the memory there is)
    CodeTooLarge,
    /// The load address is wrong (outside user memory)
    InvalidLoadAddress,
}

impl From<io::Error> for Lh5LoadError {
    fn from(err: io::Error) -> Self {
        Self::IoError(err)
    }
}

impl std::fmt::Display for Lh5LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "I/O error: {e}"),
            Self::FileTooSmall => write!(f, "The file is too small (under the 4 bytes of the header)"),
            Self::InvalidFormat => write!(f, "Wrong format: the length declared is not the real size"),
            Self::CodeTooLarge => write!(f, "The code is too big for user memory"),
            Self::InvalidLoadAddress => write!(f, "The load address is outside user memory (0x0100-0x47FF; 0x57FF on a PC-1500A)"),
        }
    }
}

impl std::error::Error for Lh5LoadError {}

/// Reads a .lh5 file and returns its load address and machine code.
///
/// # The binary format
/// ```text
/// +--------+--------+--------+--------+------------------+
/// | addr_L | addr_H | len_L  | len_H  | machine code ... |
/// +--------+--------+--------+--------+------------------+
///   byte 0   byte 1   byte 2   byte 3   bytes 4+
/// ```
pub fn read_lh5_file(path: &Path) -> Result<(u16, Vec<u8>), Lh5LoadError> {
    let data = fs::read(path)?;

    if data.len() < 4 {
        return Err(Lh5LoadError::FileTooSmall);
    }

    let load_address = u16::from_le_bytes([data[0], data[1]]);
    let code_length = u16::from_le_bytes([data[2], data[3]]) as usize;

    if data.len() < 4 + code_length {
        return Err(Lh5LoadError::InvalidFormat);
    }

    let machine_code = data[4..4 + code_length].to_vec();

    Ok((load_address, machine_code))
}

/// Checks that a load address and a code size fit the PC-1500's memory
/// map.
///
/// The valid range is `0x0100`-`user_memory_end` (user memory with the 16 KB
/// CE-161 expansion fitted: `0x47FF` on a PC-1500, `0x57FF` on a PC-1500A;
/// see the comment by `STANDARD_USER_MEMORY_BEGIN` in `memory.rs`).
pub fn validate_load_parameters(load_address: u16, code_size: usize, user_memory_end: u16) -> Result<(), Lh5LoadError> {
    const USER_MEMORY_START: u16 = 0x0100;

    if load_address < USER_MEMORY_START || load_address > user_memory_end {
        return Err(Lh5LoadError::InvalidLoadAddress);
    }

    let end_address = load_address as usize + code_size;
    if end_address > user_memory_end as usize + 1 {
        return Err(Lh5LoadError::CodeTooLarge);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(bytes).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_read_valid_lh5() {
        let bytes = [
            0x00, 0x40, // load_address = 0x4000
            0x03, 0x00, // code_length = 3
            0xB5, 0x42, 0xFD, // code
        ];
        let f = write_tmp(&bytes);
        let (addr, code) = read_lh5_file(f.path()).unwrap();
        assert_eq!(addr, 0x4000);
        assert_eq!(code, vec![0xB5, 0x42, 0xFD]);
    }

    #[test]
    fn test_file_too_small() {
        let f = write_tmp(&[0x00, 0x40]);
        assert!(matches!(read_lh5_file(f.path()), Err(Lh5LoadError::FileTooSmall)));
    }

    #[test]
    fn test_invalid_format() {
        let bytes = [
            0x00, 0x40, // load_address = 0x4000
            0x0A, 0x00, // code_length = 10
            0xB5, 0x42, 0xFD, // only 3 bytes of code
        ];
        let f = write_tmp(&bytes);
        assert!(matches!(read_lh5_file(f.path()), Err(Lh5LoadError::InvalidFormat)));
    }

    #[test]
    fn test_validate_valid() {
        assert!(validate_load_parameters(0x0100, 100, 0x47FF).is_ok());
        assert!(validate_load_parameters(0x4000, 512, 0x47FF).is_ok());
        assert!(validate_load_parameters(0x4700, 100, 0x47FF).is_ok());
    }

    #[test]
    fn test_validate_invalid_address() {
        // Below the real start (0x0100, after the CE-161's reserved area
        // of 256 bytes).
        assert!(matches!(
            validate_load_parameters(0x0000, 10, 0x47FF),
            Err(Lh5LoadError::InvalidLoadAddress)
        ));
        assert!(matches!(
            validate_load_parameters(0x00FF, 10, 0x47FF),
            Err(Lh5LoadError::InvalidLoadAddress)
        ));
        // Above the real end (0x47FF, the end of the built-in STANDARD USER
        // MEMORY; there is no CE-155 any more taking it up to 0x5FFF).
        assert!(matches!(
            validate_load_parameters(0x4800, 10, 0x47FF),
            Err(Lh5LoadError::InvalidLoadAddress)
        ));
    }

    #[test]
    fn test_validate_code_too_large() {
        // Starting at 0x0100, 0x4700 bytes reach exactly 0x47FF (ok)
        assert!(validate_load_parameters(0x0100, 0x4700, 0x47FF).is_ok());
        // One byte more overflows
        assert!(matches!(
            validate_load_parameters(0x0100, 0x4701, 0x47FF),
            Err(Lh5LoadError::CodeTooLarge)
        ));
    }

    /// A PC-1500A has 4 KB more RAM: &4800-&57FF is user memory there.
    #[test]
    fn test_validate_pc1500a() {
        assert!(validate_load_parameters(0x4800, 10, 0x57FF).is_ok());
        assert!(validate_load_parameters(0x0100, 0x5700, 0x57FF).is_ok());
        assert!(matches!(
            validate_load_parameters(0x5800, 10, 0x57FF),
            Err(Lh5LoadError::InvalidLoadAddress)
        ));
        assert!(matches!(
            validate_load_parameters(0x0100, 0x5701, 0x57FF),
            Err(Lh5LoadError::CodeTooLarge)
        ));
    }
}
