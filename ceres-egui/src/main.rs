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
    // Argumento opcional: un archivo a cargar al arrancar, según su
    // extensión (ver `basic_io::StartupFile`): `.bas`/`.asc`/`.txt`/`.bin`
    // (y `.sbs`, `.tok`, `.img`) son programas BASIC, que esperan en cola
    // a que la ROM llegue al prompt; cualquier otro, como siempre, código
    // máquina `.lh5`.
    let startup = std::env::args_os()
        .nth(1)
        .map(|arg| basic_io::StartupFile::from_path(PathBuf::from(arg)));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // La carátula (ver `faceplate`) mide 679×299 unidades: a
            // 1280×760, con el panel de controles arriba, sale a casi el
            // doble del tamaño real de la PC-1500.
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([720.0, 420.0])
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
        Box::new(move |cc| Ok(Box::new(Pc1500App::new(cc, startup)))),
    )
    .map_err(Into::into)
}
