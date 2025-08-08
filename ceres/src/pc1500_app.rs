/// PC-1500 Application - Enhanced Console Display
///
/// This is a console application for the PC-1500 emulator that shows the display state
/// in a more visual way than the basic test. Future GUI implementation will go here.

use ceres_core::{AudioCallback, Pc1500, Pc1500Model, Sample};

/// Simple audio callback implementation for PC-1500
struct Pc1500AudioCallback;

impl AudioCallback for Pc1500AudioCallback {
    fn audio_sample(&self, _l: Sample, _r: Sample) {
        // PC-1500 beeper implementation would go here
    }
}

fn main() -> anyhow::Result<()> {
    println!("🖥️  PC-1500 Emulator with Enhanced Display");
    println!("==========================================");
    
    // Create PC-1500 system
    let audio_callback = Pc1500AudioCallback;
    let mut pc1500 = Pc1500::new(Pc1500Model::Pc1500, audio_callback);
    
    // Initialize test mode
    println!("\n1. Initializing PC-1500 system...");
    pc1500.init_test_mode();
    
    // Show initial state
    println!("\n2. Initial system state:");
    pc1500.print_test_state();
    
    // Run some test instructions
    println!("\n3. Running test instructions...");
    pc1500.run_test_instructions(10);
    
    // Display the screen state
    println!("\n4. Display state:");
    display_screen(&mut pc1500);
    
    // Run a few frames to see activity
    println!("\n5. Running emulation frames...");
    for frame in 1..=5 {
        pc1500.step_frame();
        println!("  Frame {}: CPU at PC=0x{:04X}, Cycles={}", 
                frame, pc1500.cpu().p(), pc1500.cycles_run());
    }
    
    // Final display state
    println!("\n6. Final display state:");
    display_screen(&mut pc1500);
    
    println!("\n✅ PC-1500 Emulator Enhanced Display Test Complete!");
    println!("Display buffer is {} bytes (156x8x4 RGBA)", pc1500.pixel_data_rgba().len());
    
    Ok(())
}

fn display_screen(pc1500: &mut Pc1500<Pc1500AudioCallback>) {
    let rgba_data = pc1500.pixel_data_rgba();
    
    println!("  Display RGBA buffer: {} bytes", rgba_data.len());
    
    if rgba_data.len() == 156 * 8 * 4 {
        println!("  ✅ Display buffer size correct (156x8x4)");
        
        // Show a visual representation of the screen (simplified)
        println!("  Screen preview (first 4 rows of 32 pixels each):");
        println!("  ┌────────────────────────────────┐");
        
        for row in 0..4 {
            print!("  │");
            for col in 0..32 {
                let pixel_idx = (row * 156 + col) * 4;
                if pixel_idx + 3 < rgba_data.len() {
                    // Check if pixel is on (non-zero RGB values)
                    let r = rgba_data[pixel_idx];
                    let g = rgba_data[pixel_idx + 1]; 
                    let b = rgba_data[pixel_idx + 2];
                    
                    if r > 0 || g > 0 || b > 0 {
                        print!("█"); // Pixel on
                    } else {
                        print!("░"); // Pixel off
                    }
                }
            }
            println!("│");
        }
        println!("  └────────────────────────────────┘");
        
        // Show first 32 RGBA values in hex
        println!("  First 32 RGBA bytes:");
        print!("  ");
        for i in 0..32.min(rgba_data.len()) {
            print!("{:02X} ", rgba_data[i]);
            if (i + 1) % 16 == 0 {
                println!();
                print!("  ");
            }
        }
        println!();
        
    } else {
        println!("  ❌ Display buffer size mismatch!");
    }
}
