/// Test ROM Loading Verification
/// 
/// This test verifies that PC-1500_A04.ROM is being loaded correctly

use ceres_core::pc1500::memory::MemoryBus;
use ceres_core::pc1500::memory::map;

fn main() {
    println!("🔍 PC-1500 ROM Loading Verification");
    println!("=====================================");
    
    let memory = MemoryBus::new();
    
    // Check ROM information
    let (rom_size, rom_description) = memory.rom_info();
    println!("📊 ROM Information:");
    println!("   Size: {} bytes ({} KB)", rom_size, rom_size / 1024);
    println!("   Description: {}", rom_description);
    
    // Verify ROM size matches expected PC-1500_A04.ROM size
    if rom_size == 16384 {
        println!("✅ ROM size is correct: 16KB (16384 bytes)");
    } else {
        println!("❌ ROM size is incorrect: Expected 16384 bytes, got {}", rom_size);
    }
    
    // Check ROM address space constants
    println!("\n📍 ROM Address Space:");
    println!("   ROM_START: 0x{:04X}", map::ROM_START);
    println!("   ROM_END:   0x{:04X}", map::ROM_END);
    println!("   ROM_SIZE:  {} bytes (0x{:04X})", map::ROM_SIZE, map::ROM_SIZE);
    
    // Verify the address space is correct for 16KB
    if map::ROM_SIZE == 0x4000 && map::ROM_END == 0x3FFF {
        println!("✅ ROM address space is correct for 16KB");
    } else {
        println!("❌ ROM address space configuration error");
    }
    
    // Test reading first few bytes of ROM to verify it's loaded
    println!("\n🔍 ROM Content Verification:");
    println!("   First 16 bytes of ROM:");
    print!("   ");
    for i in 0..16 {
        let byte = memory.read_byte(map::ROM_START + i);
        print!("{:02X} ", byte);
    }
    println!();
    
    // Check if ROM appears to contain meaningful data (not all zeros)
    let mut non_zero_count = 0;
    for i in 0..256 {
        if memory.read_byte(map::ROM_START + i) != 0x00 {
            non_zero_count += 1;
        }
    }
    
    if non_zero_count > 10 {
        println!("✅ ROM contains non-zero data ({} non-zero bytes in first 256)", non_zero_count);
        println!("✅ PC-1500_A04.ROM appears to be loaded correctly!");
    } else {
        println!("❌ ROM appears to be empty or not loaded correctly");
    }
    
    // Display memory verification
    println!("\n📺 Display Memory Configuration:");
    println!("   Section 1: 0x{:04X}-0x{:04X} ({} bytes)", 
            map::DISPLAY_RAM_START_1, map::DISPLAY_RAM_END_1, 
            map::DISPLAY_RAM_END_1 - map::DISPLAY_RAM_START_1 + 1);
    println!("   Section 2: 0x{:04X}-0x{:04X} ({} bytes)", 
            map::DISPLAY_RAM_START_2, map::DISPLAY_RAM_END_2,
            map::DISPLAY_RAM_END_2 - map::DISPLAY_RAM_START_2 + 1);
    println!("   Total: {} bytes", map::DISPLAY_RAM_SIZE);
    
    // RAM configuration
    println!("\n💾 RAM Configuration:");
    println!("   RAM_START: 0x{:04X}", map::RAM_START);
    println!("   RAM_END:   0x{:04X}", map::RAM_END);
    println!("   RAM_SIZE:  {} bytes ({} KB)", map::RAM_SIZE, map::RAM_SIZE / 1024);
    
    if map::RAM_SIZE == 0x2000 {
        println!("✅ RAM configuration is correct: 8KB");
    } else {
        println!("❌ RAM configuration error: Expected 8KB (0x2000 bytes)");
    }
    
    println!("\n🎯 SUMMARY:");
    println!("   ROM: PC-1500_A04.ROM (16KB) at 0x0000-0x3FFF");
    println!("   RAM: 8KB at 0x8000-0x9FFF"); 
    println!("   Display: 160 bytes at 0x7600-0x764F and 0x7700-0x774F");
    println!("   Status: ROM loading verification complete!");
}
