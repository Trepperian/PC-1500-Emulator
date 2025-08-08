// PC-1500 Core System Implementation
// Based on Sharp PC-1500 Technical Manual

use crate::AudioCallback;

/// PC-1500 System Models
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pc1500Model {
    /// Original PC-1500 with 2KB RAM
    Pc1500,
    /// PC-1500A with 8KB RAM
    Pc1500A,
}

impl Default for Pc1500Model {
    fn default() -> Self {
        Self::Pc1500
    }
}

/// PC-1500 CPU State (LH5801)
#[derive(Debug, Default)]
pub struct Lh5801Cpu {
    // Main registers
    a: u8,      // Accumulator
    x: u8,      // X register
    y: u8,      // Y register
    s: u8,      // S register
    p: u16,     // Program Counter (16-bit)
    u: u16,     // U register (16-bit)
    
    // Status flags
    flags: u8,  // Status register
    
    // Control state
    is_halted: bool,
    interrupt_enabled: bool,
}

impl Lh5801Cpu {
    pub const fn a(&self) -> u8 {
        self.a
    }
    
    pub const fn x(&self) -> u8 {
        self.x
    }
    
    pub const fn y(&self) -> u8 {
        self.y
    }
    
    pub const fn s(&self) -> u8 {
        self.s
    }
    
    pub const fn p(&self) -> u16 {
        self.p
    }
    
    pub const fn u(&self) -> u16 {
        self.u
    }
    
    pub const fn flags(&self) -> u8 {
        self.flags
    }
    
    pub const fn is_halted(&self) -> bool {
        self.is_halted
    }
}

/// PC-1500 LCD Display Controller
#[derive(Debug)]
pub struct Pc1500Display {
    // Display buffer: 156x8 pixels = 1248 pixels
    // Each pixel is 1 bit (black/white)
    buffer: [u8; 156], // 156 bytes for 156x8 display
    contrast: u8,
    enabled: bool,
}

impl Default for Pc1500Display {
    fn default() -> Self {
        Self {
            buffer: [0; 156],
            contrast: 0,
            enabled: false,
        }
    }
}

impl Pc1500Display {
    pub const DISPLAY_WIDTH: usize = 156;
    pub const DISPLAY_HEIGHT: usize = 8;
    pub const DISPLAY_BYTES: usize = 156;
    
    pub fn read_display_data(&self, addr: u8) -> u8 {
        if addr < 156 {
            self.buffer[addr as usize]
        } else {
            0xFF
        }
    }
    
    pub fn write_display_data(&mut self, addr: u8, val: u8) {
        if addr < 156 {
            self.buffer[addr as usize] = val;
        }
    }
    
    pub fn get_pixel_data_rgba(&self) -> Vec<u8> {
        let mut rgba_data = Vec::with_capacity(Self::DISPLAY_WIDTH * Self::DISPLAY_HEIGHT * 4);
        
        for byte in &self.buffer {
            for bit in 0..8 {
                let pixel_on = (byte >> (7 - bit)) & 1 != 0;
                let color = if pixel_on { 0x00 } else { 0xFF }; // Black or white
                
                rgba_data.push(color);      // R
                rgba_data.push(color);      // G
                rgba_data.push(color);      // B
                rgba_data.push(0xFF);       // A
            }
        }
        
        rgba_data
    }
}

/// PC-1500 Keyboard Interface
#[derive(Debug, Default)]
pub struct Pc1500Keyboard {
    // Keyboard matrix state
    matrix: [u8; 8], // 8 rows of key matrix
    current_row: u8,
}

impl Pc1500Keyboard {
    pub fn set_key_state(&mut self, row: u8, col: u8, pressed: bool) {
        if row < 8 && col < 8 {
            if pressed {
                self.matrix[row as usize] |= 1 << col;
            } else {
                self.matrix[row as usize] &= !(1 << col);
            }
        }
    }
    
    pub fn read_current_row(&self) -> u8 {
        if self.current_row < 8 {
            self.matrix[self.current_row as usize]
        } else {
            0xFF
        }
    }
    
    pub fn set_row_select(&mut self, row: u8) {
        self.current_row = row & 0x07;
    }
}

/// PC-1500 Memory Map
pub mod memory_map {
    // PC-1500 Memory Layout (from Technical Manual)
    pub const ROM_START: u16 = 0x0000;
    pub const ROM_END: u16 = 0x7FFF;   // 32KB ROM space
    
    pub const RAM_START: u16 = 0x8000;
    pub const RAM_END: u16 = 0x87FF;   // 2KB internal RAM (PC-1500)
    pub const RAM_END_A: u16 = 0x9FFF; // 8KB internal RAM (PC-1500A)
    
    pub const DISPLAY_RAM_START_1: u16 = 0x7600;
    pub const DISPLAY_RAM_END_1: u16 = 0x764F;   // 80 bytes display RAM section 1
    pub const DISPLAY_RAM_START_2: u16 = 0x7700;
    pub const DISPLAY_RAM_END_2: u16 = 0x774F;   // 80 bytes display RAM section 2
    
    pub const IO_START: u16 = 0xFC00;
    pub const IO_END: u16 = 0xFFFF;
    
    // Specific I/O addresses (to be refined based on manual)
    pub const KEYBOARD_DATA: u16 = 0xFC00;
    pub const KEYBOARD_SELECT: u16 = 0xFC01;
    pub const DISPLAY_CTRL: u16 = 0xFC10;
    pub const TIMER_CTRL: u16 = 0xFC20;
    pub const INTERRUPT_CTRL: u16 = 0xFC30;
}

/// Main PC-1500 System
#[derive(Debug)]
pub struct Pc1500<A: AudioCallback> {
    model: Pc1500Model,
    cpu: Lh5801Cpu,
    ram: Vec<u8>,
    rom: Vec<u8>,
    display: Pc1500Display,
    keyboard: Pc1500Keyboard,
    
    // Timing
    cycles_run: u64,
    
    // Audio placeholder (minimal for now)
    audio_callback: A,
}

impl<A: AudioCallback> Pc1500<A> {
    pub fn new(model: Pc1500Model, rom: Vec<u8>, audio_callback: A) -> Self {
        let ram_size = match model {
            Pc1500Model::Pc1500 => 2048,   // 2KB
            Pc1500Model::Pc1500A => 8192,  // 8KB
        };
        
        Self {
            model,
            cpu: Lh5801Cpu::default(),
            ram: vec![0; ram_size],
            rom,
            display: Pc1500Display::default(),
            keyboard: Pc1500Keyboard::default(),
            cycles_run: 0,
            audio_callback,
        }
    }
    
    pub fn cpu(&self) -> &Lh5801Cpu {
        &self.cpu
    }
    
    pub fn display(&self) -> &Pc1500Display {
        &self.display
    }
    
    pub fn keyboard_mut(&mut self) -> &mut Pc1500Keyboard {
        &mut self.keyboard
    }
    
    pub fn get_display_rgba(&self) -> Vec<u8> {
        self.display.get_pixel_data_rgba()
    }
    
    // Memory access methods
    pub fn read_mem(&self, addr: u16) -> u8 {
        match addr {
            memory_map::ROM_START..=memory_map::ROM_END => {
                if (addr as usize) < self.rom.len() {
                    self.rom[addr as usize]
                } else {
                    0xFF
                }
            }
            memory_map::RAM_START..=memory_map::RAM_END => {
                let ram_addr = (addr - memory_map::RAM_START) as usize;
                if ram_addr < self.ram.len() {
                    self.ram[ram_addr]
                } else {
                    0xFF
                }
            }
            memory_map::DISPLAY_RAM_START..=memory_map::DISPLAY_RAM_END => {
                let display_addr = (addr - memory_map::DISPLAY_RAM_START) as u8;
                self.display.read_display_data(display_addr)
            }
            memory_map::KEYBOARD_DATA => {
                self.keyboard.read_current_row()
            }
            _ => 0xFF, // Unmapped memory
        }
    }
    
    pub fn write_mem(&mut self, addr: u16, val: u8) {
        match addr {
            memory_map::RAM_START..=memory_map::RAM_END => {
                let ram_addr = (addr - memory_map::RAM_START) as usize;
                if ram_addr < self.ram.len() {
                    self.ram[ram_addr] = val;
                }
            }
            memory_map::DISPLAY_RAM_START..=memory_map::DISPLAY_RAM_END => {
                let display_addr = (addr - memory_map::DISPLAY_RAM_START) as u8;
                self.display.write_display_data(display_addr, val);
            }
            memory_map::KEYBOARD_SELECT => {
                self.keyboard.set_row_select(val);
            }
            _ => {
                // Ignore writes to ROM and unmapped areas
            }
        }
    }
    
    // Placeholder for CPU execution
    pub fn step(&mut self) -> u64 {
        // TODO: Implement LH5801 instruction execution
        self.cycles_run += 1;
        1
    }
    
    pub fn reset(&mut self) {
        self.cpu = Lh5801Cpu::default();
        self.cycles_run = 0;
        // Keep RAM and display state
    }
}

/// PC-1500 System Builder
/// PC-1500 System Builder (following GameBoy pattern)
pub struct Pc1500Builder {
    model: Pc1500Model,
    bios: Option<Vec<u8>>,
}

impl Pc1500Builder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            model: Pc1500Model::default(),
            bios: None,
        }
    }

    #[must_use]
    pub const fn model(mut self, model: Pc1500Model) -> Self {
        self.model = model;
        self
    }

    pub fn bios(mut self, bios_rom: Vec<u8>) -> Self {
        self.bios = Some(bios_rom);
        self
    }

    pub fn build<A: AudioCallback>(self, audio_callback: A) -> Result<Pc1500<A>, String> {
        let mut pc1500 = Pc1500::new(self.model, audio_callback);
        
        if let Some(bios_data) = self.bios {
            pc1500.load_bios(&bios_data)?;
        }
        
        Ok(pc1500)
    }
}
