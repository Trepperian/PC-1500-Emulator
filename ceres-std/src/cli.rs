use std::path::{Path, PathBuf};

pub const QUALIFIER: &str = "com.github";
pub const ORGANIZATION: &str = "remind-me-later";
pub const CERES_BIN: &str = "ceres";
pub const CERES_STYLIZED: &str = "Ceres";
const ABOUT: &str = "A (very experimental) Game Boy/Color emulator.";
const AFTER_HELP: &str = "GB bindings:

    | Gameboy | Emulator  |
    | ------- | --------- |
    | Dpad    | WASD      |
    | A       | K         |
    | B       | L         |
    | Start   | M         |
    | Select  | N         |

Other binsings:

    | System       | Emulator |
    | ------------ | -------- |
    | Fullscreen   | F        |
    | Scale filter | Z        |
";

pub trait AppOption: Default + Clone + Copy + clap::ValueEnum {
    fn str(self) -> &'static str;
    fn iter() -> impl Iterator<Item = Self>;
}

#[derive(Default, Clone, Copy, clap::ValueEnum)]
enum Model {
    Dmg,
    Mgb,
    #[default]
    Cgb,
}

#[derive(Default, Clone, Copy, clap::ValueEnum)]
pub enum System {
    #[default]
    GameBoy,
    Pc1500,
}

impl AppOption for System {
    fn str(self) -> &'static str {
        match self {
            Self::GameBoy => "gameboy",
            Self::Pc1500 => "pc1500",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [Self::GameBoy, Self::Pc1500].into_iter()
    }
}

impl AppOption for Model {
    fn str(self) -> &'static str {
        match self {
            Self::Dmg => "dmg",
            Self::Mgb => "mgb",
            Self::Cgb => "cgb",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [Self::Dmg, Self::Mgb, Self::Cgb].into_iter()
    }
}

impl From<Model> for ceres_core::Model {
    fn from(model: Model) -> Self {
        match model {
            Model::Dmg => Self::Dmg,
            Model::Mgb => Self::Mgb,
            Model::Cgb => Self::Cgb,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ShaderOption {
    #[default]
    Nearest,
    Scale2x,
    Scale3x,
    Lcd,
    Crt,
}

impl AppOption for ShaderOption {
    fn str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Scale2x => "scale2x",
            Self::Scale3x => "scale3x",
            Self::Lcd => "lcd",
            Self::Crt => "crt",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [
            Self::Nearest,
            Self::Scale2x,
            Self::Scale3x,
            Self::Lcd,
            Self::Crt,
        ]
        .into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum ScalingOption {
    PixelPerfect,
    #[default]
    Stretch,
}

impl AppOption for ScalingOption {
    fn str(self) -> &'static str {
        match self {
            Self::PixelPerfect => "pixel-perfect",
            Self::Stretch => "stretch",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [Self::PixelPerfect, Self::Stretch].into_iter()
    }
}

#[derive(clap::Parser)]
#[command(name = CERES_BIN, about = ABOUT, after_help = AFTER_HELP)]
pub struct Cli {
    #[arg(
        help = "ROM/BIOS file to emulate.",
        long_help = "ROM/BIOS file to emulate. For Game Boy, this should be a ROM file. \
           For PC-1500, this should be a BIOS file. Extension doesn't matter, \
           but the file format will be validated. Doesn't accept compressed (zip) files.",
        required = false
    )]
    file: Option<PathBuf>,
    #[arg(
        short = 't',
        long,
        help = "System to emulate",
        default_value = System::default().str(),
        value_enum,
        required = false
    )]
    system: System,
    #[arg(
        short,
        long,
        help = "Game Boy model to emulate (only for GameBoy system)",
        default_value = Model::default().str(),
        value_enum,
        required = false
    )]
    model: Model,
    #[arg(
        short,
        long,
        help = "Shader used",
        default_value = ShaderOption::default().str(),
        value_enum,
        required = false
    )]
    shader_option: ShaderOption,
    #[arg(
        short,
        long,
        help = "Pixel mode",
        default_value = ScalingOption::default().str(),
        value_enum,
        required = false
    )]
    pixel_mode: ScalingOption,
}

impl Cli {
    #[must_use]
    pub const fn system(&self) -> System {
        self.system
    }

    #[must_use]
    pub fn model(&self) -> ceres_core::Model {
        self.model.into()
    }

    #[must_use]
    pub fn pc1500_model(&self) -> ceres_core::Pc1500Model {
        // For now, default to PC-1500 (2KB model)
        // This could be extended to support PC-1500A via CLI
        ceres_core::Pc1500Model::Pc1500
    }

    #[must_use]
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    #[must_use]
    pub const fn shader_option(&self) -> ShaderOption {
        self.shader_option
    }

    #[must_use]
    pub const fn scaling_option(&self) -> ScalingOption {
        self.pixel_mode
    }

    #[must_use]
    pub const fn pixel_mode(&self) -> ScalingOption {
        self.pixel_mode
    }
}
