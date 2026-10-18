mod pc1500_app;

use std::path::PathBuf;

use eframe::egui;
use pc1500_app::Pc1500App;

fn main() -> anyhow::Result<()> {
    // Arguments: an optional .lh5 file to load at start, `--basic file.bin`
    // for a tokenized BASIC program instead, and `--pc1500a` to emulate a
    // PC-1500A (4 KB more RAM, &0100-&57FF with the CE-161).
    let mut pc1500a = false;
    let mut lh5_file: Option<PathBuf> = None;
    let mut basic_file: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pc1500a" => pc1500a = true,
            "--basic" => basic_file = Some(PathBuf::from(args.next().ok_or_else(|| anyhow::anyhow!("--basic needs a file"))?)),
            _ => lh5_file = Some(PathBuf::from(arg)),
        }
    }
    let app_name = if pc1500a { "PC-1500A Emulator" } else { "PC-1500 Emulator" };
    // The window says what it was started with, to tell apart two windows
    // run side by side (a program's BASIC original and its compiled code).
    let file_name = |path: &PathBuf| path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let title = match (&basic_file, &lh5_file) {
        (Some(path), _) => format!("{app_name} - {} (BASIC)", file_name(path)),
        (None, Some(path)) => format!("{app_name} - {} (compiled)", file_name(path)),
        (None, None) => app_name.to_string(),
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 500.0])
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
        Box::new(move |cc| Ok(Box::new(Pc1500App::new(cc, lh5_file.as_deref(), basic_file.as_deref(), pc1500a)))),
    )
    .map_err(Into::into)
}
