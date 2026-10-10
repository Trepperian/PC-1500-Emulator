mod pc1500_app;

use std::path::PathBuf;

use eframe::egui;
use pc1500_app::Pc1500App;

fn main() -> anyhow::Result<()> {
    // Arguments: an optional .lh5 file to load at start, and `--pc1500a`
    // to emulate a PC-1500A (4 KB more RAM, &0100-&57FF with the CE-161).
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pc1500a = args.iter().any(|arg| arg == "--pc1500a");
    let lh5_file: Option<PathBuf> = args.iter().find(|arg| !arg.starts_with("--")).map(PathBuf::from);
    let title = if pc1500a { "PC-1500A Emulator" } else { "PC-1500 Emulator" };

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
        title,
        options,
        Box::new(move |cc| Ok(Box::new(Pc1500App::new(cc, lh5_file.as_deref(), pc1500a)))),
    )
    .map_err(Into::into)
}
