# 🏗️ Arquitectura PC-1500 Emulator - Limpiada y Organizada

## 📁 Estructura Final del Proyecto

### **Emulador PC-1500 (PRINCIPAL)**
```
ceres-egui/
├── src/
│   ├── pc1500_main.rs           # 🚀 PUNTO DE ENTRADA - Ejecuta: cargo run --bin pc1500-egui
│   ├── pc1500_app.rs            # 🎯 APLICACIÓN PRINCIPAL (986 líneas)
│   ├── pc1500_keyboard.rs       # ⌨️  Sistema de teclado y mapeo
│   └── pc1500_app_FINAL_WORKING.rs  # 💾 BACKUP DE SEGURIDAD
```

### **Core del Emulador PC-1500**
```
ceres-core/src/pc1500/
├── memory/mod.rs                # 🧠 Bus de memoria (ROM/RAM/Display)
├── cpu/                         # 💻 LH5801 CPU implementation
├── display/                     # 📺 Display controller 156x7
└── joypad/                      # ⌨️  Keyboard controller
```

### **ROM Auténtica**
```
pc1500-roms/bin/
└── PC-1500_A04.ROM             # 🔧 ROM auténtica 16KB (embedded)
```

## 🚀 Cómo Ejecutar

### **PC-1500 Emulator (Principal)**
```bash
cargo run --package ceres-egui --bin pc1500-egui
```

### **Game Boy Emulator (Separado)**
```bash
cargo run --package ceres-egui --bin ceres-egui
```

## ✅ Funcionalidades Implementadas

### **1. Interfaz PC-1500 Completa**
- ✅ Teclado calculadora auténtico con layout PC-1500
- ✅ Display 156x7 pixels auténtico
- ✅ Feedback visual (teclas amarillas al presionar)
- ✅ Mapeo de teclado físico a teclas PC-1500

### **2. Sistema ROM Completo**
- ✅ PC-1500_A04.ROM (16KB) embedded at compile-time
- ✅ Load ROM files dialog
- ✅ Memory mapping windows (ROM/RAM/Display)
- ✅ Memory inspector with hex view
- ✅ System information display

### **3. Memory Layout Auténtico**
```
0x0000-0x3FFF : ROM (16KB) - PC-1500_A04.ROM
0x7600-0x764F : Display Bank 0 (80 bytes)
0x7700-0x774F : Display Bank 1 (80 bytes) 
0x8000-0x9FFF : RAM (8KB)
0xFC00-0xFFFF : I/O Space
```

## 📋 Archivos Eliminados en la Limpieza

### **Archivos de Desarrollo Eliminados** ❌
- `pc1500_app_backup.rs` (50.8KB)
- `pc1500_app_broken_simple.rs` (17KB)
- `pc1500_app_broken2.rs` (50.8KB)
- `pc1500_app_clean.rs` (17KB)
- `pc1500_app_complete.rs` (22KB)
- `pc1500_app_simple_backup.rs` (17KB)

### **Archivos de Prueba Eliminados** ❌
- `test_pc1500_functionality.rs`
- `test_pc1500_rom.rs`
- `pc1500_two_symbol_test` (binario)
- `test_pc1500_rom` (binario)

## 🔒 Archivos de Seguridad

### **Backup Activo** 💾
- `pc1500_app_FINAL_WORKING.rs` - Copia exacta del archivo funcional

## 🎯 Estado Actual

- ✅ **Compilación**: Exitosa sin errores críticos
- ✅ **Funcionalidad**: Todos los controles operativos
- ✅ **ROM**: PC-1500_A04.ROM cargada y verificada
- ✅ **Interfaz**: Completa con feedback visual
- ✅ **Arquitectura**: Limpia y organizada

## 🚀 Ready to Use!

El emulador PC-1500 está completamente funcional y listo para usar. Todos los archivos de desarrollo fueron eliminados, manteniendo solo la versión final operativa con backup de seguridad.
