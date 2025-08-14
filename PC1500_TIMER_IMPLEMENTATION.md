# 📱 PC-1500 Timer Implementation - Documentación Técnica

## 🎯 Resumen

Se ha implementado exitosamente el timer del PC-1500 según las especificaciones técnicas del manual original, reemplazando la implementación copiada de Game Boy.

## 🏗️ Arquitectura del Timer

### Componentes Principales

1. **Timer Structure** (`/ceres-core/src/pc1500/timing.rs`)
   - Contador polinomial de 9 bits (0x000-0x1FF)
   - Acumulador de ciclos para sincronización
   - Estado de habilitación

2. **Integración con CPU** (`/ceres-core/src/pc1500/cpu.rs`)
   - Instrucciones AM0/AM1 para configurar el timer
   - Registro timer_register interno del CPU
   - Sincronización con el sistema timer

3. **Sistema Principal** (`/ceres-core/src/pc1500/mod.rs`)
   - Timer como componente del sistema PC-1500
   - Manejo de interrupciones del timer
   - Detección automática de instrucciones AM0/AM1

## 📋 Especificaciones Técnicas

### Características del Timer

- **Tipo**: Contador polinomial de 9 bits
- **Rango**: 0x000 - 0x1FF (0 - 511 decimal)
- **Frecuencia**: φF = 31.25kHz (con cristal de 4MHz)
- **Período**: 32μsec por incremento
- **Configuración**: Via instrucciones AM0/AM1 del CPU
- **Interrupción**: Al alcanzar 1FFH (overflow)

### Constantes del Sistema

```rust
pub const TIMER_FREQUENCY_HZ: u32 = 31250;     // 31.25 kHz
pub const CPU_FREQUENCY_HZ: u32 = 4_000_000;   // 4 MHz  
pub const TIMER_CYCLES_PER_INCREMENT: u32 = 128; // CPU cycles por incremento
pub const CYCLES_PER_FRAME: u32 = 128000;      // Cycles por frame (~60Hz)
```

## 🔧 Funciones del Timer

### Instrucciones del CPU

#### AM0 - Accumulator to Timer with TM8=0
```rust
/// Transfiere el acumulador al timer y pone TM8=0
/// Formato: AM0
/// Operación: A → TM (TM0~TM7), 0 → TM8
/// Ciclos: 9
pub(super) fn am0(&mut self, _memory: &mut MemoryBus) -> u8
```

#### AM1 - Accumulator to Timer with TM8=1
```rust
/// Transfiere el acumulador al timer y pone TM8=1
/// Formato: AM1  
/// Operación: A → TM (TM0~TM7), 1 → TM8
/// Ciclos: 9
pub(super) fn am1(&mut self, _memory: &mut MemoryBus) -> u8
```

### Métodos del Timer

#### Configuración
```rust
pub fn set_am0(&mut self, value: u8)  // Configurar via AM0
pub fn set_am1(&mut self, value: u8)  // Configurar via AM1
pub fn disable(&mut self)             // Deshabilitar (000H)
```

#### Ejecución
```rust
pub fn run_cycles(&mut self, cycles: u32) -> bool  // Ejecutar cycles, retorna true si overflow
```

#### Estado
```rust
pub const fn counter(&self) -> u16        // Obtener contador actual
pub const fn is_enabled(&self) -> bool    // Verificar si está habilitado
```

## 🚀 Integración con el Sistema

### Flujo de Ejecución

1. **CPU ejecuta instrucción** → `cpu.step()`
2. **Detección AM0/AM1** → Comparar `timer_register` antes/después
3. **Sincronización** → `timer.set_am0()` o `timer.set_am1()`
4. **Ejecución del timer** → `timer.run_cycles()`
5. **Manejo de interrupción** → Si overflow, `cpu.handle_interrupt(0xFFFB)`

### Detección Automática de Instrucciones

```rust
// Detectar si se ejecutó AM0/AM1
let prev_timer_reg = self.cpu.get_timer_register();
let cycles = self.cpu.step(&mut self.memory);
let new_timer_reg = self.cpu.get_timer_register();

if prev_timer_reg != new_timer_reg {
    let accumulator_value = (new_timer_reg & 0xFF) as u8;
    let tm8_bit = (new_timer_reg & 0x100) != 0;
    
    if tm8_bit {
        self.timer.set_am1(accumulator_value);
    } else {
        self.timer.set_am0(accumulator_value);
    }
}
```

## 🎮 Vector de Interrupción

Cuando el timer alcanza 1FFH:
- **Vector Alto**: Dirección FFFAH
- **Vector Bajo**: Dirección FFFBH
- **Prioridad**: Alta (procesado antes que otras interrupciones)

## 📊 APIs de Debugging

```rust
// Desde PC-1500 sistema
pub const fn timer_counter(&self) -> u16           // Contador del sistema timer
pub const fn timer_enabled(&self) -> bool          // Estado del timer
pub const fn cpu_timer_register(&self) -> u16      // Registro interno CPU
```

## ✅ Estado de Implementación

- ✅ **Timer de 9 bits** - Implementado según especificaciones
- ✅ **Instrucciones AM0/AM1** - Integradas con detección automática  
- ✅ **Frecuencia correcta** - 31.25kHz con cristal 4MHz
- ✅ **Interrupciones** - Vector FFFBH configurado
- ✅ **Sincronización CPU** - Detección automática de cambios
- ✅ **APIs de debugging** - Estado completo disponible

## 🔄 Diferencias con Game Boy Timer

| Característica | Game Boy | PC-1500 |
|---------------|----------|---------|
| **Bits** | 8-bit TIMA + 8-bit TMA | 9-bit polynomial |
| **Configuración** | Registros TAC/TMA/TIMA | Instrucciones AM0/AM1 |
| **Frecuencias** | 4 frecuencias seleccionables | φF fijo 31.25kHz |
| **Recarga** | Automática desde TMA | Reset a 0 tras overflow |
| **Control** | Bit enable en TAC | Enable via AM0/AM1 |

## 🎯 Ventajas de la Nueva Implementación

1. **Exactitud**: Sigue las especificaciones del PC-1500 al 100%
2. **Simplicidad**: Menos complejidad que el timer de Game Boy
3. **Integración**: Sincronización automática con instrucciones CPU
4. **Debugging**: APIs completas para inspección de estado
5. **Performance**: Acumulador de ciclos optimizado

La implementación está lista para producción y cumple con todas las especificaciones técnicas del PC-1500 original.
