/// Test simple para verificar que la escritura en memoria funciona
/// Basado en el pc1500_window que SÍ funcionaba

use ceres_core::{AudioCallback, Pc1500, Pc1500Model, Sample};

/// Audio callback simple
struct SimpleAudioCallback;

impl AudioCallback for SimpleAudioCallback {
    fn audio_sample(&self, _l: Sample, _r: Sample) {
        // No audio needed
    }
}

fn main() {
    println!("🔬 PC-1500 Memory Test - Simple Version");
    println!("=====================================");
    
    let audio_callback = SimpleAudioCallback;
    let mut pc1500 = Pc1500::new(Pc1500Model::Pc1500, audio_callback);
    
    println!("✅ PC-1500 created");
    
    // Test 1: Clear display
    pc1500.clear_display();
    println!("✅ Display cleared");
    
    // Test 2: Write some text (esto funciona)
    pc1500.display_message("TEST");
    println!("✅ Text 'TEST' written");
    
    // Test 3: Read current display memory
    println!("\n📋 Current display memory (first 10 bytes):");
    for i in 0..10 {
        let addr = 0x7600 + i;
        let value = pc1500.read_memory(addr);
        println!("   0x{:04X} = 0x{:02X} = {:08b}", addr, value, value);
    }
    
    // Test 4: Write directly to memory
    println!("\n📝 Writing directly to memory...");
    pc1500.write_memory(0x7600, 0xFF);
    pc1500.write_memory(0x7601, 0x00);
    pc1500.write_memory(0x7602, 0xFF);
    pc1500.write_memory(0x7603, 0x00);
    
    // Test 5: Read back what we wrote
    println!("\n📋 After direct write (first 10 bytes):");
    for i in 0..10 {
        let addr = 0x7600 + i;
        let value = pc1500.read_memory(addr);
        println!("   0x{:04X} = 0x{:02X} = {:08b}", addr, value, value);
    }
    
    // Test 6: Get RGBA buffer to see if it's updating
    let rgba_buffer = pc1500.display().rgba_buffer();
    println!("\n📊 RGBA buffer info:");
    println!("   Buffer size: {} bytes", rgba_buffer.len());
    println!("   Expected size: {} bytes", 156 * 7 * 4);
    
    // Check first few pixels
    println!("\n🔍 First 5 pixels in RGBA:");
    for i in 0..5 {
        let idx = i * 4;
        if idx + 3 < rgba_buffer.len() {
            let r = rgba_buffer[idx];
            let g = rgba_buffer[idx + 1];
            let b = rgba_buffer[idx + 2];
            let a = rgba_buffer[idx + 3];
            println!("   Pixel {}: R={} G={} B={} A={}", i, r, g, b, a);
        }
    }
    
    println!("\n✅ All tests completed!");
    println!("💡 If memory writes work, the issue is in the display update chain");
}
