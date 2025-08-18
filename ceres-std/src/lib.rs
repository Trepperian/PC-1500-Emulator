//mod audio;
mod cli;
mod thread;

#[cfg(feature = "wgpu_renderer")]
pub mod wgpu_renderer;

pub use ceres_core::{keyboard, Model, display::DISPLAY_WIDTH, display::DISPLAY_HEIGHT};
pub use clap;
pub use cli::{
    AppOption, CERES_BIN, CERES_STYLIZED, Cli, ORGANIZATION, QUALIFIER,
    ShaderOption,
};
pub use thread::{Error, Pc1500Thread, PainterCallback, Pressable};

pub const PIXEL_BUFFER_SIZE: usize = 4 * DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize;
