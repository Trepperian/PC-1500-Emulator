# 🧪## **📋 Descripción**

Esta carpeta contiene todos los archivos de test para verificar las funcionalidades del emulador PC-1500.

## **📁 Archivos de Test**

### **🎮 Tests de Teclado y Control**
- **`pc1500_keyboard_mapping_test.rs`** - Mapeo de teclas físicas a PC-1500
- **`pc1500_keyboard_ita_test.rs`** - Funcionalidad del teclado con ITA
- **`pc1500_ita_keyboard_integration_test.rs`** - Integración completa ITA+teclado
- **`pc1500_ita_accumulator_test.rs`** - Verificación acumulador con ITA
- **`pc1500_ita_instruction_test.rs`** - Test de instrucción ITA individual
- **`pc1500_ita_advanced_test.rs`** - Test avanzado de ITA con performance

### **🔧 Tests de Sistema y CPU**
- **`pc1500_control_instructions_test.rs`** - **NUEVO**: Tests de instrucciones de control LH5801
- **`pc1500_memory_test_cli.rs`** - Sistema de memoria del PC-1500
- **`pc1500_display_test.rs`** - Sistema de display y rendering
- **`pc1500_test.rs`** - Test general del sistema PC-1500Emulator Tests

Esta carpeta contiene todos los archivos de test para verificar las funcionalidades del emulador PC-1500.

## 📋 **Archivos de Test Disponibles**

### **🎹 Tests de Teclado**
- **`pc1500_keyboard_mapping_test.rs`** - Test comprehensivo del mapeo de teclas físicas → PC-1500
- **`pc1500_keyboard_ita_test.rs`** - Test de integración del teclado con la instrucción ITA

### **🔧 Tests de CPU e Instrucciones**
- **`pc1500_ita_instruction_test.rs`** - Test de la instrucción ITA (In To Accumulator)
- **`pc1500_ita_accumulator_test.rs`** - Verificación que ITA guarda correctamente en el acumulador

### **💾 Tests de Memoria**
- **`pc1500_memory_test_cli.rs`** - Test CLI del sistema de memoria del PC-1500

### **🖥️ Tests de Display**
- **`pc1500_display_test.rs`** - Test del sistema de display del PC-1500

### **🔄 Tests Generales**
- **`pc1500_test.rs`** - Test general del sistema PC-1500

### **🗺️ Módulos de Soporte**
- **`pc1500_keyboard_mapper.rs`** - Módulo de mapeo PhysicalKey → PC-1500 Key (usado por tests)

## 🚀 **Cómo Ejecutar los Tests**

### **Ejecutar un test específico:**
```bash
# Test de mapeo de teclado
cargo run -p ceres --bin pc1500_keyboard_mapping_test

# Test de instrucción ITA
cargo run -p ceres --bin pc1500_ita_accumulator_test

# Test de memoria
cargo run -p ceres --bin pc1500_memory_test_cli

# Test de display
cargo run -p ceres --bin pc1500_display_test

# Test de integración ITA
cargo run -p ceres --bin pc1500_ita_instruction_test

# Test general del sistema
cargo run -p ceres --bin pc1500_test
```

### **Ejecutar todos los tests:**
```bash
# Compilar todos los tests
cargo build -p ceres --bins

# O ejecutar uno por uno con el package especificado
cargo run -p ceres --bin pc1500_keyboard_mapping_test
cargo run -p ceres --bin pc1500_ita_accumulator_test
cargo run -p ceres --bin pc1500_memory_test_cli
cargo run -p ceres --bin pc1500_display_test
cargo run -p ceres --bin pc1500_ita_instruction_test
cargo run -p ceres --bin pc1500_keyboard_ita_test
cargo run -p ceres --bin pc1500_test
```

## ✅ **Estado de los Tests**

| Test | Estado | Descripción |
|------|--------|-------------|
| `pc1500_keyboard_mapping_test` | ✅ PASANDO | Mapeo de teclas funcionando |
| `pc1500_ita_accumulator_test` | ✅ PASANDO | ITA guarda en acumulador |
| `pc1500_ita_instruction_test` | ✅ PASANDO | Instrucción ITA implementada |
| `pc1500_control_instructions_test` | ✅ PASANDO | **NUEVO**: Tests instrucciones control LH5801 |
| `pc1500_memory_test_cli` | ✅ PASANDO | Sistema de memoria funcional |
| `pc1500_display_test` | ✅ PASANDO | Display test básico |
| `pc1500_keyboard_ita_test` | ✅ PASANDO | Integración teclado-CPU |
| `pc1500_test` | ✅ PASANDO | Test general sistema |

## 📊 **Funcionalidades Verificadas**

### **✅ Sistema de Teclado PC-1500**
- [x] Enum Key con todos los códigos de teclas PC-1500
- [x] Struct Keyboard con matriz 6x16
- [x] Mapeo PhysicalKey → PC-1500 Key

### **✅ Instrucciones de Control LH5801** ⭐ **NUEVO**
- [x] **18 instrucciones** implementadas con opcodes oficiales
- [x] **Archivo dedicado**: `control_instructions.rs` 
- [x] **Estructura CPU extendida** con campos específicos PC-1500
- [x] **Tests básicos** de funcionamiento completados
- [x] Sistema de eventos presionar/soltar
- [x] Integración con Hash trait para HashSet

### **✅ Instrucción ITA (In To Accumulator)**
- [x] Opcode 0xBA implementado en CPU (LH5801 oficial)
- [x] ITA lee IN0-IN7 (pines 66-73)
- [x] ITA guarda valor en acumulador
- [x] ITA actualiza flags del CPU (zero, negative)
- [x] ITA consume 3 ciclos de CPU

### **✅ Sistema de Memoria**
- [x] MemoryBus con integración de teclado
- [x] Método read_keyboard_input() para ITA
- [x] Mapeo de memoria PC-1500

### **✅ Aplicación egui**
- [x] Interfaz gráfica PC-1500
- [x] Mapeo de teclas físicas en tiempo real
- [x] Panel de debug con log de eventos
- [x] Ventana de ayuda con mapeos

## 🏗️ **Estructura del Proyecto**

```
Ceres/
├── tests/                          # 📁 Esta carpeta
│   ├── pc1500_keyboard_mapping_test.rs
│   ├── pc1500_ita_accumulator_test.rs
│   ├── pc1500_ita_instruction_test.rs
│   ├── pc1500_memory_test_cli.rs
│   ├── pc1500_display_test.rs
│   ├── pc1500_keyboard_ita_test.rs
│   └── pc1500_test.rs
├── ceres-core/                     # 🧠 Core del emulador
│   └── src/pc1500/
│       ├── joypad.rs              # 🎹 Sistema de teclado
│       ├── cpu.rs                 # 🔧 CPU con instrucción ITA
│       └── memory/mod.rs          # 💾 Sistema de memoria
├── ceres-egui/                     # 🖥️ Interfaz gráfica
│   └── src/
│       ├── pc1500_app.rs          # 📱 App principal PC-1500
│       ├── pc1500_keyboard.rs     # ⌨️ Mapeo egui
│       └── pc1500_main.rs         # 🚀 Entry point
└── ceres/                          # 🎮 Aplicaciones principales
    └── src/
        ├── pc1500_keyboard_mapper.rs  # 🗺️ Mapper físico→virtual
        ├── pc1500_window.rs           # 🖼️ Ventana PC-1500
        └── pc1500_app.rs              # 📺 App console
```

## 🎯 **Próximos Tests a Implementar**

- [ ] Test de integración completa CPU + Memoria + Teclado
- [ ] Test de performance del sistema de teclado
- [ ] Test de múltiples teclas presionadas simultáneamente
- [ ] Test de carga/guardado de estado del emulador
- [ ] Test de interrupciones del teclado

---

*Todos los tests están organizados para verificar las funcionalidades principales del emulador PC-1500 y asegurar que el sistema funcione correctamente.*
