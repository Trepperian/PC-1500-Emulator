//! Módulo para cargar archivos de código máquina LH5801 (.lh5)
//!
//! Formato del archivo:
//! - Bytes 0-1: Dirección de carga (u16, little-endian)
//! - Bytes 2-3: Longitud del código (u16, little-endian)
//! - Bytes 4+:  Código máquina LH5801 puro

use std::{fs, io, path::Path};

/// Errores al cargar archivos .lh5
#[derive(Debug)]
pub enum Lh5LoadError {
    /// Error de I/O al leer el archivo
    IoError(io::Error),
    /// Archivo demasiado pequeño (< 4 bytes de header)
    FileTooSmall,
    /// Formato inválido (code_length no coincide con tamaño real)
    InvalidFormat,
    /// Código demasiado grande (excede memoria disponible)
    CodeTooLarge,
    /// Dirección de carga inválida (fuera del rango de memoria usuario)
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
            Self::IoError(e) => write!(f, "Error I/O: {e}"),
            Self::FileTooSmall => write!(f, "Archivo demasiado pequeño (< 4 bytes de header)"),
            Self::InvalidFormat => write!(f, "Formato inválido: longitud declarada no coincide con el tamaño real"),
            Self::CodeTooLarge => write!(f, "Código demasiado grande para la memoria de usuario"),
            Self::InvalidLoadAddress => write!(f, "Dirección de carga fuera de la memoria de usuario (0x0100-0x47FF; 0x57FF en un PC-1500A)"),
        }
    }
}

impl std::error::Error for Lh5LoadError {}

/// Lee un archivo .lh5 y extrae la dirección de carga y el código máquina.
///
/// # Formato binario
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

/// Valida que una dirección de carga y un tamaño de código sean seguros
/// para el mapa de memoria del PC-1500.
///
/// Rango válido: `0x0100`–`user_memory_end` (memoria de usuario con la
/// expansión CE-161 de 16KB instalada: `0x47FF` en un PC-1500, `0x57FF` en
/// un PC-1500A — ver el comentario junto a `STANDARD_USER_MEMORY_BEGIN` en
/// `memory.rs`).
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
            0xB5, 0x42, 0xFD, // código
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
            0xB5, 0x42, 0xFD, // solo 3 bytes de código
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
        // Por debajo del inicio real (0x0100, tras el área reservada
        // de 256 bytes del CE-161).
        assert!(matches!(
            validate_load_parameters(0x0000, 10, 0x47FF),
            Err(Lh5LoadError::InvalidLoadAddress)
        ));
        assert!(matches!(
            validate_load_parameters(0x00FF, 10, 0x47FF),
            Err(Lh5LoadError::InvalidLoadAddress)
        ));
        // Por encima del final real (0x47FF, fin de la STANDARD USER
        // MEMORY incorporada — ya no hay CE-155 extendiendo hasta 0x5FFF).
        assert!(matches!(
            validate_load_parameters(0x4800, 10, 0x47FF),
            Err(Lh5LoadError::InvalidLoadAddress)
        ));
    }

    #[test]
    fn test_validate_code_too_large() {
        // Arranca en 0x0100 con 0x4700 bytes llega exactamente a 0x47FF (ok)
        assert!(validate_load_parameters(0x0100, 0x4700, 0x47FF).is_ok());
        // Un byte extra ya desborda
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
