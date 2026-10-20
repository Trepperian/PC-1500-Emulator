mod basic_io;
mod faceplate;
mod host_keymap;
mod lcd_view;
mod pacing;
mod pc1500_app;

use std::path::PathBuf;

use eframe::egui;
use pc1500_app::Pc1500App;

fn main() -> anyhow::Result<()> {
    // Arguments: an optional file to load at start, by its extension (see
    // `basic_io::StartupFile`): `.bas`/`.asc`/`.txt`/`.bin` (and `.sbs`,
    // `.tok`, `.img`) are BASIC programs, which wait in a queue until the
    // ROM reaches its prompt; anything else is `.lh5` machine code.
    // `--basic file` loads a file as BASIC whatever its extension, and
    // `--pc1500a` emulates a PC-1500A (4 KB more RAM, &0100-&57FF with the
    // CE-161).
    let mut pc1500a = false;
    let mut startup: Option<basic_io::StartupFile> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--pc1500a") => pc1500a = true,
            Some("--basic") => {
                let path = args.next().ok_or_else(|| anyhow::anyhow!("--basic needs a file"))?;
                startup = Some(basic_io::StartupFile::Basic(PathBuf::from(path)));
            }
            _ => startup = Some(basic_io::StartupFile::from_path(PathBuf::from(arg))),
        }
    }
    let app_name = if pc1500a { "PC-1500A Emulator" } else { "PC-1500 Emulator" };
    // The window says what it was started with, to tell apart two windows
    // run side by side (a program's BASIC original and its compiled code).
    let file_name = |path: &PathBuf| path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let title = match &startup {
        Some(basic_io::StartupFile::Basic(path)) => format!("{app_name} - {} (BASIC)", file_name(path)),
        Some(basic_io::StartupFile::Lh5(path)) => format!("{app_name} - {} (compiled)", file_name(path)),
        None => app_name.to_string(),
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // La carátula (ver `faceplate`) mide 679×299 unidades: a
            // 1280×760, con el panel de controles arriba, sale a casi el
            // doble del tamaño real de la PC-1500.
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([720.0, 420.0])
            .with_title(title),
        renderer: eframe::Renderer::Wgpu,
        vsync: true,
        depth_buffer: 0,
        stencil_buffer: 0,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        app_name,
        options,
        Box::new(move |cc| Ok(Box::new(Pc1500App::new(cc, startup, pc1500a)))),
    )
    .map_err(Into::into)
}
