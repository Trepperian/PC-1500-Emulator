/// Memory bus and address space management for PC-1500
///
/// The PC-1500 has a 64KB address space with the following layout:
/// 0x0000-0x3FFF: ROM (16KB)
/// 0x7600-0x764F: Display RAM First Section (80 bytes)
/// 0x7700-0x774F: Display RAM Second Section (80 bytes)
/// 0x8000-0x9FFF: RAM (8KB)
/// 0xFC00-0xFFFF: I/O Space
use super::display::DisplayController;
use super::joypad::Keyboard;

/// PC-1500 ROM dump - includes the actual firmware from the calculator
const PC1500_ROM: &[u8] = include_bytes!("/Users/mateo/Desktop/Carrera/TFG/Ceres/PC-1500_A04.ROM");

/// Memory map constants
pub mod map {
    pub const ROM_START: u16 = 0x0000;
    pub const ROM_END: u16 = 0x3FFF;
    pub const ROM_SIZE: usize = 0x4000; // 16KB

    pub const RAM_START: u16 = 0x8000;
    pub const RAM_END: u16 = 0x9FFF;
    pub const RAM_SIZE: usize = 0x2000; // 8KB

    // PC-1500A has 8KB RAM (0x8000-0x9FFF)
    pub const RAM_END_A: u16 = 0x9FFF;

    // PC-1500 Display memory - two separate sections (as per real hardware)
    pub const DISPLAY_RAM_START_1: u16 = 0x7600;
    pub const DISPLAY_RAM_END_1: u16 = 0x764F; // 80 bytes (0x7600-0x764F)
    pub const DISPLAY_RAM_START_2: u16 = 0x7700;
    pub const DISPLAY_RAM_END_2: u16 = 0x774F; // 80 bytes (0x7700-0x774F)
    pub const DISPLAY_RAM_SIZE: usize = 160; // 160 bytes total (80+80)

    pub const IO_START: u16 = 0xFC00;
    pub const IO_END: u16 = 0xFFFF;

    // I/O Register addresses
    pub const DISPLAY_CTRL: u16 = 0xFC10;
    pub const TIMER_CTRL: u16 = 0xFC20;
    pub const INTERRUPT_CTRL: u16 = 0xFC30;
}

/// Main memory bus for PC-1500
pub struct MemoryBus {
    rom: &'static [u8],
    ram: [u8; map::RAM_SIZE],
    display_controller: DisplayController,
    keyboard: Keyboard,
    io_registers: [u8; 0x400], // I/O space 0xFC00-0xFFFF
}

impl MemoryBus {
    pub fn new() -> Self {
        Self {
            rom: PC1500_ROM,
            ram: [0; map::RAM_SIZE],
            display_controller: DisplayController::new(),
            keyboard: Keyboard::new(),
            io_registers: [0; 0x400],
        }
    }

    /// Get ROM information
    pub fn rom_info(&self) -> (usize, &'static str) {
        if self.rom.len() < 1024 {
            (self.rom.len(), "PC-1500 ROM dump (partial)")
        } else {
            (self.rom.len(), "PC-1500 ROM dump")
        }
    }

    /// Read a byte from the address space
    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            // Display memory has priority over ROM in these ranges
            map::DISPLAY_RAM_START_1..=map::DISPLAY_RAM_END_1 => self
                .display_controller
                .read_vram(addr - map::DISPLAY_RAM_START_1),
            map::DISPLAY_RAM_START_2..=map::DISPLAY_RAM_END_2 => self
                .display_controller
                .read_vram((addr - map::DISPLAY_RAM_START_2) + 80), // Second section offset (80 bytes)
            map::ROM_START..=map::ROM_END => {
                let offset = (addr - map::ROM_START) as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    // Return 0x00 for unloaded ROM areas (common in embedded systems)
                    0x00
                }
            }
            map::RAM_START..=map::RAM_END => self.ram[(addr - map::RAM_START) as usize],
            map::IO_START..=map::IO_END => self.read_io_register(addr),
            _ => {
                // Unmapped memory returns 0xFF
                0xFF
            }
        }
    }

    /// Write a byte to the address space
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            // Display memory has priority over ROM in these ranges
            map::DISPLAY_RAM_START_1..=map::DISPLAY_RAM_END_1 => {
                self.display_controller
                    .write_vram(addr - map::DISPLAY_RAM_START_1, value);
            }
            map::DISPLAY_RAM_START_2..=map::DISPLAY_RAM_END_2 => {
                self.display_controller
                    .write_vram((addr - map::DISPLAY_RAM_START_2) + 80, value); // Second section offset (80 bytes)
            }
            map::ROM_START..=map::ROM_END => {
                // ROM is read-only, ignore writes
            }
            map::RAM_START..=map::RAM_END => {
                self.ram[(addr - map::RAM_START) as usize] = value;
            }
            map::IO_START..=map::IO_END => {
                self.write_io_register(addr, value);
            }
            _ => {
                // Unmapped memory, ignore writes
            }
        }
    }

    /// Get reference to display controller for rendering
    pub fn display(&self) -> &DisplayController {
        &self.display_controller
    }

    /// Get mutable reference to display controller
    pub fn display_mut(&mut self) -> &mut DisplayController {
        &mut self.display_controller
    }

    /// Get reference to keyboard controller
    pub fn keyboard(&self) -> &Keyboard {
        &self.keyboard
    }

    /// Get mutable reference to keyboard controller  
    pub fn keyboard_mut(&mut self) -> &mut Keyboard {
        &mut self.keyboard
    }

    /// Read keyboard input for ITA instruction
    /// This method is called by the CPU's ITA instruction to read
    /// the current keyboard state from IN0-IN7 pins
    pub fn read_keyboard_input(&self) -> u8 {
        self.keyboard.read_input_pins_in0_in7()
    }

    /// Write to output port for ATP instruction
    /// This method is called by the CPU's ATP instruction to send
    /// accumulator contents to the output port
    pub fn write_output_port(&mut self, value: u8) {
        // In PC-1500, this would control external devices or peripherals
        // For emulation, we can log or handle this as needed
        // This could control printer, external storage, etc.
        println!("ATP: Writing 0x{:02X} to output port", value);
    }

    /// Read from I/O register space
    fn read_io_register(&self, addr: u16) -> u8 {
        match addr {
            map::DISPLAY_CTRL => {
                // TODO: Implement display control register
                0x00
            }
            map::TIMER_CTRL => {
                // TODO: Implement timer control register
                0x00
            }
            map::INTERRUPT_CTRL => {
                // TODO: Implement interrupt control register
                0x00
            }
            _ => {
                let offset = (addr - map::IO_START) as usize;
                if offset < self.io_registers.len() {
                    self.io_registers[offset]
                } else {
                    0xFF
                }
            }
        }
    }

    /// Write to I/O register space
    fn write_io_register(&mut self, addr: u16, value: u8) {
        match addr {
            map::DISPLAY_CTRL => {
                // TODO: Implement display control register
                self.display_controller.set_control(value);
            }
            map::TIMER_CTRL => {
                // TODO: Implement timer control register
            }
            map::INTERRUPT_CTRL => {
                // TODO: Implement interrupt control register
            }
            _ => {
                let offset = (addr - map::IO_START) as usize;
                if offset < self.io_registers.len() {
                    self.io_registers[offset] = value;
                }
            }
        }
    }
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self::new()
    }
}
