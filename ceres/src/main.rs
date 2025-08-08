mod app;
mod pc1500_app;
mod video;

#[cfg(target_os = "macos")]
mod macos;

use ceres_std::{CERES_STYLIZED, ORGANIZATION, QUALIFIER, clap::Parser};
use winit::event_loop::EventLoop;

const WIN_MULTIPLIER: u32 = 2;

use ceres_core::Model;
use ceres_std::{ScalingOption, ShaderOption};

#[derive(Debug, Clone)]
enum CeresEvent {
    ChangeShader(ShaderOption),
    ChangeScaling(ScalingOption),
    ChangeSpeed(f32),
    OpenRomFile,
    TogglePause,
    ChangeModel(Model),
}

fn main() -> anyhow::Result<()> {
    let args = ceres_std::Cli::parse();

    #[cfg(target_os = "macos")]
    let main_event_loop = {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        EventLoop::<CeresEvent>::with_user_event()
            .with_default_menu(false)
            .build()?
    };

    #[cfg(target_os = "macos")]
    {
        // TODO: Re-enable macos menu support for PC-1500
        // macos::set_event_proxy(main_event_loop.create_proxy());
        // macos::create_menu_bar();
    }

    #[cfg(not(target_os = "macos"))]
    let main_event_loop = { EventLoop::<CeresEvent>::with_user_event().build()? };

    let project_dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, CERES_STYLIZED)
        .ok_or_else(|| {
            anyhow::anyhow!("Failed to get project directories for '{}'", CERES_STYLIZED)
        })?;

    match args.system() {
        ceres_std::System::GameBoy => {
            let mut main_window = app::App::new(
                project_dirs,
                args.model(),
                args.file(),
                args.shader_option(),
                args.scaling_option().into(),
            )?;
            main_event_loop.run_app(&mut main_window)?;
        }
        ceres_std::System::Pc1500 => {
            // TODO: Implement PC-1500 window app similar to GameBoy app
            // For now, PC-1500 functionality is available through:
            // - cargo run --bin pc1500-egui (GUI version)
            // - cargo run --bin pc1500_memory_test_cli (CLI tests)
            eprintln!("PC-1500 desktop app not yet implemented.");
            eprintln!("Use: cargo run -p ceres-egui --bin pc1500-egui");
            std::process::exit(1);
            
            /* DISABLED UNTIL PROPER APP STRUCT IS IMPLEMENTED
            let mut pc1500_window = pc1500_app::App::new(
                project_dirs,
                args.pc1500_model(),
                args.file().map(|p| p.to_path_buf()),
                args.shader_option(),
                args.scaling_option().into(),
            )?;
            main_event_loop.run_app(&mut pc1500_window)?;
            */
        }
    }

    Ok(())
}
