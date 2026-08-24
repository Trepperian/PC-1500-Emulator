mod pc1500_app;

use std::path::PathBuf;

use eframe::egui;
use pc1500_app::Pc1500App;

fn main() -> anyhow::Result<()> {
    // Argumento opcional: ruta a un archivo .lh5 a cargar al arrancar
    let lh5_file: Option<PathBuf> = std::env::args().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 500.0])
            .with_title("PC-1500 Emulator"),
        renderer: eframe::Renderer::Wgpu,
        vsync: true,
        depth_buffer: 0,
        stencil_buffer: 0,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "PC-1500 Emulator",
        options,
        Box::new(move |cc| Ok(Box::new(Pc1500App::new(cc, lh5_file.as_deref())))),
    )
    .map_err(Into::into)
}
