//! La parte de la interfaz que carga y guarda programas BASIC del host
//! (panel "Programa BASIC", arrastrar y soltar, argumento de la línea de
//! órdenes), separada en funciones puras para poder probarla sin egui. El
//! trabajo de verdad (tokenizar, formatos, escribir la RAM) está en
//! `ceres_core::basic`.
//!
//! # La cola de carga
//! Un programa solo se puede meter en la máquina con la ROM esperando
//! teclado en el prompt (`Pc1500::is_at_basic_prompt`). Al pulsar
//! "Cargar" (o soltar un archivo, o pasarlo por la línea de órdenes) el
//! archivo se lee y se interpreta en el acto, para que un error de formato
//! se vea enseguida, y el programa queda en una cola ([`PendingLoad`]) que
//! se aplica en cuanto la ROM llega al prompt. Así funciona también justo
//! después de arrancar o de pulsar Reset, cuando la ROM todavía está en su
//! arranque en frío (~40 frames). Si pasan [`PENDING_TIMEOUT_TICKS`] ciclos
//! emulados sin llegar al prompt (un programa en marcha, un `INPUT`
//! esperando...), se abandona con un aviso.

use std::path::{Path, PathBuf};

use ceres_core::{
    TICKS_PER_FRAME,
    basic::{
        AsciiOptions, BasicError, BasicFormat, DecodedProgram, Keywords, LoadReport, SaveFormat,
        Warning, decode_program, detect_format,
    },
};

/// Frames emulados (~7 s a 1x) que una carga espera al prompt.
pub const PENDING_TIMEOUT_FRAMES: usize = 600;
/// [`PENDING_TIMEOUT_FRAMES`] en ciclos de CPU.
pub const PENDING_TIMEOUT_TICKS: usize = PENDING_TIMEOUT_FRAMES * TICKS_PER_FRAME;

/// Mensaje cuando la carga se abandona.
pub const NOT_AT_PROMPT_MESSAGE: &str =
    "La PC-1500 no est\u{e1} en el prompt BASIC (pulsa CL / BREAK)";

/// El desplegable de formato del panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatChoice {
    /// Al cargar, por el contenido (`detect_format`); al guardar, por la
    /// extensión (`SaveFormat::for_extension`).
    Auto,
    AsciiCrLf,
    AsciiCr,
    Tokenized,
    Ce158,
}

impl FormatChoice {
    pub const ALL: [Self; 5] = [Self::Auto, Self::AsciiCrLf, Self::AsciiCr, Self::Tokenized, Self::Ce158];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (por extensi\u{f3}n)",
            Self::AsciiCrLf => "ASCII CRLF (.txt)",
            Self::AsciiCr => "ASCII CR (.asc)",
            Self::Tokenized => "Tokenizado (.bin)",
            Self::Ce158 => "CE-158 (.bas)",
        }
    }

    /// El formato con que se lee `bytes` (`ext` sin punto).
    #[must_use]
    pub fn load_format(self, ext: Option<&str>, bytes: &[u8]) -> BasicFormat {
        match self {
            Self::Auto => detect_format(ext, bytes),
            Self::AsciiCrLf | Self::AsciiCr => BasicFormat::Ascii,
            Self::Tokenized => BasicFormat::Tokenized,
            Self::Ce158 => BasicFormat::Ce158,
        }
    }

    /// El formato con que se guarda en `path`, o `None` para que lo
    /// decida la extensión (`.bas` CE-158, `.asc` CR, `.txt` CRLF, `.bin`
    /// tokenizado).
    #[must_use]
    pub const fn save_format(self) -> Option<SaveFormat> {
        match self {
            Self::Auto => None,
            Self::AsciiCrLf => Some(SaveFormat::AsciiCrLf),
            Self::AsciiCr => Some(SaveFormat::AsciiCr),
            Self::Tokenized => Some(SaveFormat::Tokenized),
            Self::Ce158 => Some(SaveFormat::Ce158),
        }
    }
}

/// Nombre corto de un formato de guardado, para el estado.
#[must_use]
pub const fn save_format_name(format: SaveFormat) -> &'static str {
    match format {
        SaveFormat::AsciiCrLf => "ASCII CRLF",
        SaveFormat::AsciiCr => "ASCII CR",
        SaveFormat::Tokenized => "tokenizado",
        SaveFormat::Ce158 => "CE-158",
    }
}

/// Nombre corto de un formato de lectura, para el estado.
#[must_use]
pub const fn load_format_name(format: BasicFormat) -> &'static str {
    match format {
        BasicFormat::Ascii => "ASCII",
        BasicFormat::Tokenized => "tokenizado",
        BasicFormat::Ce158 => "CE-158",
    }
}

/// Qué hacer con un archivo pasado por la línea de órdenes o soltado
/// sobre la ventana.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupFile {
    /// Código máquina `.lh5` (y, como siempre, cualquier extensión
    /// desconocida).
    Lh5(PathBuf),
    /// Programa BASIC: `.bas`, `.asc`, `.txt`, `.sbs`, `.bin`, `.tok`,
    /// `.img`.
    Basic(PathBuf),
}

impl StartupFile {
    /// Decide por la extensión (da igual mayúsculas o minúsculas).
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        if is_basic_path(&path) { Self::Basic(path) } else { Self::Lh5(path) }
    }
}

/// `true` si la extensión de `path` es de un programa BASIC.
#[must_use]
pub fn is_basic_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|e| matches!(e.as_str(), "bas" | "asc" | "txt" | "sbs" | "bin" | "tok" | "img"))
}

/// Lee los bytes de un archivo de programa con el formato elegido y las
/// palabras clave de la máquina.
pub fn decode(
    bytes: &[u8],
    path: &Path,
    choice: FormatChoice,
    keywords: &Keywords,
) -> Result<DecodedProgram, BasicError> {
    let ext = path.extension().and_then(|e| e.to_str());
    let format = choice.load_format(ext, bytes);
    decode_program(bytes, format, keywords, &AsciiOptions::default())
}

/// Una carga esperando a que la ROM llegue al prompt (ver el comentario
/// del módulo).
#[derive(Clone, Debug)]
pub struct PendingLoad {
    decoded: DecodedProgram,
    /// De dónde viene (para el estado).
    source: String,
    waited_ticks: usize,
    last_ticks: Option<usize>,
}

/// Ver [`PendingLoad::poll`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingState {
    /// Cargar ya.
    Ready,
    /// Seguir esperando.
    Waiting,
    /// Abandonar.
    TimedOut,
}

impl PendingLoad {
    #[must_use]
    pub const fn new(decoded: DecodedProgram, source: String) -> Self {
        Self { decoded, source, waited_ticks: 0, last_ticks: None }
    }

    /// El programa leído, su formato y los avisos.
    #[must_use]
    pub const fn decoded(&self) -> &DecodedProgram {
        &self.decoded
    }

    /// De dónde viene (ruta o nombre del archivo).
    #[must_use]
    pub const fn source(&self) -> &str {
        self.source.as_str()
    }

    /// Se llama tras cada avance de la emulación con `at_prompt`
    /// (`Pc1500::is_at_basic_prompt`) y `ticks` (`Pc1500::ticks`). El
    /// tiempo de espera se cuenta en ciclos emulados (en pausa no corre);
    /// si `ticks` retrocede es que hubo un Reset, y cuenta desde 0.
    pub const fn poll(&mut self, at_prompt: bool, ticks: usize) -> PendingState {
        let elapsed = match self.last_ticks {
            Some(last) if ticks >= last => ticks - last,
            Some(_) => ticks,
            None => 0,
        };
        self.last_ticks = Some(ticks);
        self.waited_ticks = self.waited_ticks.saturating_add(elapsed);
        if at_prompt {
            PendingState::Ready
        } else if self.waited_ticks > PENDING_TIMEOUT_TICKS {
            PendingState::TimedOut
        } else {
            PendingState::Waiting
        }
    }
}

/// Los avisos en una línea, separados por `; `.
#[must_use]
pub fn warnings_text(warnings: &[Warning]) -> String {
    let mut text = String::new();
    for (i, warning) in warnings.iter().enumerate() {
        if i > 0 {
            text.push_str("; ");
        }
        text.push_str(&warning.to_string());
    }
    text
}

/// El estado tras una carga correcta.
#[must_use]
pub fn load_status(source: &str, format: BasicFormat, report: &LoadReport, cold_boot_screen: bool) -> String {
    let mut status = format!(
        "Cargado {source} ({}): {} l\u{ed}neas, {} bytes en ${:04X}-${:04X}, {} libres",
        load_format_name(format),
        report.lines,
        report.bytes,
        report.start,
        report.end,
        report.free
    );
    if cold_boot_screen {
        status.push_str(" \u{2014} pulsa CL para quitar NEW0? (no hagas NEW0)");
    }
    if !report.warnings.is_empty() {
        status.push_str(". Avisos: ");
        status.push_str(&warnings_text(&report.warnings));
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceres_core::basic::{Program, ProgramLine, WarningKind};

    fn pending() -> PendingLoad {
        let program = Program::from_lines(vec![ProgramLine { number: 10, body: b"A=1".to_vec() }]);
        let decoded = DecodedProgram { program, format: BasicFormat::Ascii, warnings: Vec::new() };
        PendingLoad::new(decoded, "x.bas".to_owned())
    }

    #[test]
    fn cli_routing_by_extension() {
        assert_eq!(StartupFile::from_path("a.lh5".into()), StartupFile::Lh5("a.lh5".into()), ".lh5");
        assert_eq!(StartupFile::from_path("a".into()), StartupFile::Lh5("a".into()), "sin extensi\u{f3}n");
        for name in ["a.bas", "A.BAS", "a.asc", "a.txt", "a.bin", "a.tok"] {
            assert_eq!(StartupFile::from_path(name.into()), StartupFile::Basic(name.into()), "{name}");
        }
    }

    #[test]
    fn format_choices() {
        assert_eq!(FormatChoice::Auto.save_format(), None, "auto");
        assert_eq!(FormatChoice::AsciiCr.save_format(), Some(SaveFormat::AsciiCr), "CR");
        assert_eq!(FormatChoice::AsciiCrLf.load_format(Some("bin"), b"\x00\x0a"), BasicFormat::Ascii, "forzado");
        assert_eq!(FormatChoice::Auto.load_format(Some("bin"), b"10 A"), BasicFormat::Tokenized, ".bin");
        assert_eq!(FormatChoice::Auto.load_format(Some("bas"), b"10 A"), BasicFormat::Ascii, "texto");
        assert!(FormatChoice::ALL.iter().all(|c| !c.label().is_empty()), "con r\u{f3}tulo");
    }

    #[test]
    fn pending_load_waits_in_emulated_time() {
        let mut p = pending();
        assert_eq!(p.poll(false, 1000), PendingState::Waiting, "primera vez");
        assert_eq!(p.poll(false, 1000 + PENDING_TIMEOUT_TICKS), PendingState::Waiting, "justo el l\u{ed}mite");
        assert_eq!(p.poll(true, 2000 + PENDING_TIMEOUT_TICKS), PendingState::Ready, "en el prompt");
        assert_eq!(p.poll(false, 2001 + PENDING_TIMEOUT_TICKS), PendingState::TimedOut, "pasado el l\u{ed}mite");
        // Un Reset (los ciclos vuelven a 0) no hace esperar de más ni de
        // menos: cuenta lo emulado desde el reset.
        let mut across_reset = pending();
        assert_eq!(across_reset.poll(false, 5_000_000), PendingState::Waiting, "antes del reset");
        assert_eq!(across_reset.poll(false, 100), PendingState::Waiting, "reset");
        assert_eq!(across_reset.poll(true, 200), PendingState::Ready, "arranc\u{f3}");
    }

    #[test]
    fn status_texts() {
        let report = LoadReport {
            start: 0x01C5,
            end: 0x0771,
            bytes: 1452,
            lines: 60,
            free: 0x408E,
            warnings: vec![Warning { line: Some(20), kind: WarningKind::DuplicateLine }],
        };
        let status = load_status("j.bas", BasicFormat::Ascii, &report, false);
        assert!(status.starts_with("Cargado j.bas (ASCII): 60 l\u{ed}neas, 1452 bytes en $01C5-$0771"), "{status}");
        assert!(status.contains("Avisos: l\u{ed}nea 20:"), "{status}");
        assert!(load_status("j", BasicFormat::Ce158, &report, true).contains("pulsa CL"), "arranque en fr\u{ed}o");
    }
}
