//! The part of the GUI that loads and saves the host's BASIC programs (the "BASIC program" panel,
//! drag and drop, the command line argument), split into pure functions so that it can be tested
//! without egui. The real work (tokenizing, formats, writing the RAM) is in `ceres_core::basic`.
//!
//! # The load queue
//! A program can only be put into the machine with the ROM waiting for the keyboard at the prompt
//! (`Pc1500::is_at_basic_prompt`). On clicking "Load BASIC" (or dropping a file, or passing it on
//! the command line) the file is read and interpreted at once, so that a format error shows
//! straight away, and the program waits in a queue ([`PendingLoad`]) that is applied as soon as the
//! ROM reaches the prompt. So it also works right after starting or clicking Reset, while the ROM
//! is still in its cold start (~40 frames). If [`PENDING_TIMEOUT_TICKS`] emulated cycles go by
//! without reaching the prompt (a program running, an `INPUT` waiting...), it is given up with a
//! warning.

use std::path::{Path, PathBuf};

use ceres_core::{
    TICKS_PER_FRAME,
    basic::{
        AsciiOptions, BasicError, BasicFormat, DecodedProgram, Keywords, LoadReport, SaveFormat,
        Warning, decode_program, detect_format,
    },
};

/// Emulated frames (~7 s at 1x) that a load waits for the prompt.
pub const PENDING_TIMEOUT_FRAMES: usize = 600;
/// [`PENDING_TIMEOUT_FRAMES`] in CPU cycles.
pub const PENDING_TIMEOUT_TICKS: usize = PENDING_TIMEOUT_FRAMES * TICKS_PER_FRAME;

/// Message when the load is given up.
pub const NOT_AT_PROMPT_MESSAGE: &str =
    "The PC-1500 is not at the BASIC prompt (press CL / BREAK)";

/// The panel's format drop-down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatChoice {
    /// When loading, by the contents (`detect_format`); when saving, by the extension
    /// (`SaveFormat::for_extension`).
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
            Self::Auto => "Auto (by extension)",
            Self::AsciiCrLf => "ASCII CRLF (.txt)",
            Self::AsciiCr => "ASCII CR (.asc)",
            Self::Tokenized => "Tokenized (.bin)",
            Self::Ce158 => "CE-158 (.bas)",
        }
    }

    /// The format with which `bytes` is read (`ext` without the dot).
    #[must_use]
    pub fn load_format(self, ext: Option<&str>, bytes: &[u8]) -> BasicFormat {
        match self {
            Self::Auto => detect_format(ext, bytes),
            Self::AsciiCrLf | Self::AsciiCr => BasicFormat::Ascii,
            Self::Tokenized => BasicFormat::Tokenized,
            Self::Ce158 => BasicFormat::Ce158,
        }
    }

    /// The format with which it is saved to `path`, or `None` to let the extension decide (`.bas`
    /// CE-158, `.asc` CR, `.txt` CRLF, `.bin` tokenized).
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

/// Short name of a save format, for the status.
#[must_use]
pub const fn save_format_name(format: SaveFormat) -> &'static str {
    match format {
        SaveFormat::AsciiCrLf => "ASCII CRLF",
        SaveFormat::AsciiCr => "ASCII CR",
        SaveFormat::Tokenized => "tokenized",
        SaveFormat::Ce158 => "CE-158",
    }
}

/// Short name of a read format, for the status.
#[must_use]
pub const fn load_format_name(format: BasicFormat) -> &'static str {
    match format {
        BasicFormat::Ascii => "ASCII",
        BasicFormat::Tokenized => "tokenized",
        BasicFormat::Ce158 => "CE-158",
    }
}

/// What to do with a file passed on the command line or dropped on the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupFile {
    /// `.lh5` machine code (and, as always, any unknown extension).
    Lh5(PathBuf),
    /// BASIC program: `.bas`, `.asc`, `.txt`, `.sbs`, `.bin`, `.tok`,
    /// `.img`.
    Basic(PathBuf),
}

impl StartupFile {
    /// Decides by the extension (upper or lower case).
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        if is_basic_path(&path) { Self::Basic(path) } else { Self::Lh5(path) }
    }
}

/// `true` if the extension of `path` is that of a BASIC program.
#[must_use]
pub fn is_basic_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|e| matches!(e.as_str(), "bas" | "asc" | "txt" | "sbs" | "bin" | "tok" | "img"))
}

/// Reads the bytes of a program file with the chosen format and the machine's keywords.
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

/// A load waiting for the ROM to reach the prompt (see the module's comment).
#[derive(Clone, Debug)]
pub struct PendingLoad {
    decoded: DecodedProgram,
    /// Where it comes from (for the status).
    source: String,
    waited_ticks: usize,
    last_ticks: Option<usize>,
}

/// See [`PendingLoad::poll`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingState {
    /// Load now.
    Ready,
    /// Keep waiting.
    Waiting,
    /// Abandonar.
    TimedOut,
}

impl PendingLoad {
    #[must_use]
    pub const fn new(decoded: DecodedProgram, source: String) -> Self {
        Self { decoded, source, waited_ticks: 0, last_ticks: None }
    }

    /// The program read, its format and the warnings.
    #[must_use]
    pub const fn decoded(&self) -> &DecodedProgram {
        &self.decoded
    }

    /// Where it comes from (path or file name).
    #[must_use]
    pub const fn source(&self) -> &str {
        self.source.as_str()
    }

    /// Called after each step of the emulation with `at_prompt` (`Pc1500::is_at_basic_prompt`) and
    /// `ticks` (`Pc1500::ticks`). The waiting time is counted in emulated cycles (it does not run
    /// while paused); if `ticks` goes back, there was a Reset, and it counts from 0.
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

/// The warnings on one line, separated by `; `.
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

/// The status after a load that went well.
#[must_use]
pub fn load_status(source: &str, format: BasicFormat, report: &LoadReport, cold_boot_screen: bool) -> String {
    let mut status = format!(
        "Loaded {source} ({}): {} lines, {} bytes at ${:04X}-${:04X}, {} free",
        load_format_name(format),
        report.lines,
        report.bytes,
        report.start,
        report.end,
        report.free
    );
    if cold_boot_screen {
        status.push_str(" \u{2014} press CL to clear NEW0? (do not type NEW0)");
    }
    if !report.warnings.is_empty() {
        status.push_str(". Warnings: ");
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
        assert_eq!(StartupFile::from_path("a".into()), StartupFile::Lh5("a".into()), "no extension");
        for name in ["a.bas", "A.BAS", "a.asc", "a.txt", "a.bin", "a.tok"] {
            assert_eq!(StartupFile::from_path(name.into()), StartupFile::Basic(name.into()), "{name}");
        }
    }

    #[test]
    fn format_choices() {
        assert_eq!(FormatChoice::Auto.save_format(), None, "auto");
        assert_eq!(FormatChoice::AsciiCr.save_format(), Some(SaveFormat::AsciiCr), "CR");
        assert_eq!(FormatChoice::AsciiCrLf.load_format(Some("bin"), b"\x00\x0a"), BasicFormat::Ascii, "forced");
        assert_eq!(FormatChoice::Auto.load_format(Some("bin"), b"10 A"), BasicFormat::Tokenized, ".bin");
        assert_eq!(FormatChoice::Auto.load_format(Some("bas"), b"10 A"), BasicFormat::Ascii, "text");
        assert!(FormatChoice::ALL.iter().all(|c| !c.label().is_empty()), "with a label");
    }

    #[test]
    fn pending_load_waits_in_emulated_time() {
        let mut p = pending();
        assert_eq!(p.poll(false, 1000), PendingState::Waiting, "first time");
        assert_eq!(p.poll(false, 1000 + PENDING_TIMEOUT_TICKS), PendingState::Waiting, "right at the limit");
        assert_eq!(p.poll(true, 2000 + PENDING_TIMEOUT_TICKS), PendingState::Ready, "at the prompt");
        assert_eq!(p.poll(false, 2001 + PENDING_TIMEOUT_TICKS), PendingState::TimedOut, "past the limit");
        // A Reset (the cycles go back to 0) makes it wait neither more nor less: it counts what was
        // emulated since the reset.
        let mut across_reset = pending();
        assert_eq!(across_reset.poll(false, 5_000_000), PendingState::Waiting, "before the reset");
        assert_eq!(across_reset.poll(false, 100), PendingState::Waiting, "reset");
        assert_eq!(across_reset.poll(true, 200), PendingState::Ready, "it booted");
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
        assert!(status.starts_with("Loaded j.bas (ASCII): 60 lines, 1452 bytes at $01C5-$0771"), "{status}");
        assert!(status.contains("Warnings: line 20:"), "{status}");
        assert!(load_status("j", BasicFormat::Ce158, &report, true).contains("press CL"), "cold start");
    }
}
