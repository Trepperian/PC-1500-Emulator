use ceres::pc1500::PC1500;

fn main() {
    println!("🎮 PC-1500 Manual Memory Test");
    println!("=============================");
    
    let mut pc1500 = PC1500::new();
    
    // Limpiar el display primero
    pc1500.clear_display();
    println!("✅ Display cleared");
    
    // TEST 1: Escribir directamente en memoria
    println!("\n📝 TEST 1: Writing direct memory patterns");
    
    // Primera columna: patrón alternante (0xAA = 10101010)
    pc1500.write_memory(0x7600, 0xAA);
    println!("   0x7600 = 0xAA (alternating pattern in column 0)");
    
    // Segunda columna: todos los píxeles ON (0xFF = 11111111)
    pc1500.write_memory(0x7601, 0xFF);
    println!("   0x7601 = 0xFF (all pixels ON in column 1)");
    
    // Tercera columna: todos los píxeles OFF (0x00 = 00000000)
    pc1500.write_memory(0x7602, 0x00);
    println!("   0x7602 = 0x00 (all pixels OFF in column 2)");
    
    // Cuarta columna: patrón personalizado (0x55 = 01010101)
    pc1500.write_memory(0x7603, 0x55);
    println!("   0x7603 = 0x55 (inverse alternating in column 3)");
    
    // TEST 2: Crear una línea horizontal en el medio (row 3-4)
    println!("\n📝 TEST 2: Drawing horizontal line (rows 3-4)");
    for col in 10..30 {
        pc1500.write_memory(0x7600 + col, 0x18); // 0x18 = 00011000 (bits 3 y 4)
    }
    println!("   Horizontal line from column 10 to 29");
    
    // TEST 3: Crear una línea vertical
    println!("\n📝 TEST 3: Drawing vertical line (column 50)");
    pc1500.write_memory(0x7600 + 50, 0xFF); // Toda la columna 50
    println!("   Vertical line at column 50");
    
    // TEST 4: Escribir tu propio patrón
    println!("\n📝 TEST 4: Custom pattern - Simple house");
    let house_pattern = [
        0x04, // 00000100 - roof peak
        0x0E, // 00001110 - roof
        0x1F, // 00011111 - roof base
        0x11, // 00010001 - walls
        0x11, // 00010001 - walls
        0x11, // 00010001 - walls
        0x1F, // 00011111 - floor
    ];
    
    for (i, &pattern) in house_pattern.iter().enumerate() {
        pc1500.write_memory(0x7600 + 70 + i, pattern);
    }
    println!("   Simple house drawn at columns 70-76");
    
    // TEST 5: Leer de vuelta lo que escribimos
    println!("\n📝 TEST 5: Reading back memory values");
    println!("   0x7600 = 0x{:02X} (should be 0xAA)", pc1500.read_memory(0x7600));
    println!("   0x7601 = 0x{:02X} (should be 0xFF)", pc1500.read_memory(0x7601));
    println!("   0x7602 = 0x{:02X} (should be 0x00)", pc1500.read_memory(0x7602));
    println!("   0x7603 = 0x{:02X} (should be 0x55)", pc1500.read_memory(0x7603));
    
    // Mostrar información del display
    println!("\n📊 Display Information:");
    println!("   Display size: 156x7 pixels");
    println!("   Memory range: 0x7600 to 0x764F and 0x7700 to 0x774F (160 bytes total)");
    println!("   First section: 0x7600-0x764F (80 bytes), Second section: 0x7700-0x774F (80 bytes)");
    println!("   Each byte controls two symbols with 4+3 dots encoding");
    
    println!("\n✅ All memory operations completed!");
    println!("💡 TIP: Run 'cargo run --bin pc1500_window' to see the visual result!");
}
