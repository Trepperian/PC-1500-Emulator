/// Test simple para verificar el funcionamiento del timer PC-1500
/// Compila con: rustc --edition 2021 pc1500_timer_test.rs -L target/debug/deps --extern ceres_core=target/debug/libceres_core.rlib

use std::io::{self, Write};

// Simulación de AudioCallback para test
struct DummyAudio;
impl ceres_core::AudioCallback for DummyAudio {
    fn audio_sample(&mut self, _left: i16, _right: i16) {}
}

fn main() -> io::Result<()> {
    println!("🧪 PC-1500 Timer Test");
    println!("====================");
    
    // Crear instancia del PC-1500
    let mut pc1500 = ceres_core::pc1500::Pc1500::new(
        ceres_core::pc1500::Pc1500Model::Pc1500,
        DummyAudio
    );

    println!("✅ PC-1500 emulator created successfully");
    println!("📊 Initial State:");
    println!("   - CPU Timer Register: 0x{:04X}", pc1500.cpu_timer_register());
    println!("   - System Timer Counter: 0x{:03X}", pc1500.timer_counter());
    println!("   - Timer Enabled: {}", pc1500.timer_enabled());
    
    // Simular ejecución de algunas instrucciones
    println!("\n🔄 Running CPU cycles...");
    for i in 0..10 {
        pc1500.run_cpu();
        if i % 3 == 0 {
            println!("   Cycle {}: Timer={:03X}, CPU_Timer={:04X}, Enabled={}", 
                    i, pc1500.timer_counter(), pc1500.cpu_timer_register(), pc1500.timer_enabled());
        }
    }
    
    println!("\n📈 Final State:");
    println!("   - CPU Timer Register: 0x{:04X}", pc1500.cpu_timer_register());
    println!("   - System Timer Counter: 0x{:03X}", pc1500.timer_counter());
    println!("   - Timer Enabled: {}", pc1500.timer_enabled());
    println!("   - Total Cycles Run: {}", pc1500.cycles_run());
    
    println!("\n✅ Timer implementation test completed!");
    println!("📋 Timer Specifications Summary:");
    println!("   - 9-bit polynomial counter (0x000-0x1FF)");
    println!("   - Configured by AM0/AM1 CPU instructions");
    println!("   - Operates at φF = 31.25kHz (with 4MHz crystal)");
    println!("   - Issues interrupt when reaching 1FFH");
    println!("   - Timer increments every 32μsec");
    
    Ok(())
}
