/// PC-1500 ROM Inspector
/// 
/// This utility inspects the embedded ROM and provides information about its contents

use ceres_core::{pc1500::{Pc1500, Pc1500Model}, AudioCallback};

// Simple audio callback for the inspector
struct DummyAudio;
impl AudioCallback for DummyAudio {
    fn audio_sample(&self, _left: i16, _right: i16) {}
}

fn main() {
    println!("PC-1500 ROM Inspector");
    println!("====================");
    
    // Create emulator instance to access ROM
    let audio = DummyAudio;
    let emulator = Pc1500::new(Pc1500Model::default(), audio);
    
    // Get ROM information
    let (rom_size, rom_desc) = emulator.get_rom_info();
    
    println!("ROM Information:");
    println!("- Description: {}", rom_desc);
    println!("- Size: {} bytes", rom_size);
    println!("- Expected size: 16KB (16384 bytes)");
    
    if rom_size < 16384 {
        println!("- Status: PARTIAL ROM (only first {} bytes loaded)", rom_size);
        println!("- Note: This is likely a ROM dump sample, not the complete firmware");
    } else {
        println!("- Status: COMPLETE ROM");
    }
    
    println!("\nROM Content Analysis:");
    println!("First 32 bytes of ROM:");
    
    for i in 0..32u16 {
        if i % 16 == 0 {
            print!("0x{:04X}: ", i);
        }
        
        let byte = emulator.read_rom_byte(i);
        print!("{:02X} ", byte);
        
        if i % 16 == 15 {
            println!();
        }
    }
    
    println!("\nMemory Map:");
    println!("- 0x0000-0x3FFF: ROM ({} KB)", rom_size / 1024);
    println!("- 0x8000-0x9FFF: RAM (8 KB)");
    println!("- 0x7600-0x764F: Display RAM 1 (80 bytes)");
    println!("- 0x7700-0x774F: Display RAM 2 (80 bytes)");
    println!("- 0xFC00-0xFFFF: I/O Space");
    
    let display_info = emulator.get_display_memory_info();
    println!("\nDisplay Information:");
    println!("- Memory range: 0x{:04X}-0x{:04X}", display_info.0, display_info.1);
    println!("- Size: {} bytes", display_info.2);
    println!("- Resolution: 156×8 pixels");
    
    println!("\nTo load a complete ROM:");
    println!("1. Obtain a full PC-1500 ROM dump (16KB)");
    println!("2. Replace pc1500-roms/bin/PC-1500_A04.ROM with the complete ROM");
    println!("3. Rebuild the project");
}
