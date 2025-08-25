mod pc1500_app;

use eframe::egui;
use pc1500_app::Pc1500App;

fn main() -> anyhow::Result<()> {
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
        Box::new(move |cc| Ok(Box::new(Pc1500App::new(cc)))),
    )
    .map_err(Into::into)
}
