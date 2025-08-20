const PC1500_ROM_BYTES: &[u8] = include_bytes!("../../../pc1500-roms/bin/PC-1500_A04.ROM");

const STANDARD_USER_MEMORY_BEGIN: u32 = 0x4000;
const STANDARD_USER_MEMORY_END: u32 = 0x47FF;
const STANDARD_USER_MEMORY_SIZE: usize =
    (STANDARD_USER_MEMORY_END - STANDARD_USER_MEMORY_BEGIN + 1) as usize;

pub const STANDARD_USER_SYSTEM_MEMORY_BEGIN: u32 = 0x7600;
const STANDARD_USER_SYSTEM_MEMORY_END: u32 = 0x7BFF;
const STANDARD_USER_SYSTEM_MEMORY_SIZE: usize =
    (STANDARD_USER_SYSTEM_MEMORY_END - STANDARD_USER_SYSTEM_MEMORY_BEGIN + 1) as usize;

const ROM_BEGIN: u32 = 0xC000;
const ROM_END: u32 = 0xFFFF;
const ROM_SIZE: usize = (ROM_END - ROM_BEGIN + 1) as usize;

pub struct MemoryBus {
    rom: &'static [u8],
    standard_user_memory: [u8; STANDARD_USER_MEMORY_SIZE],
    standard_user_system_memory: [u8; STANDARD_USER_SYSTEM_MEMORY_SIZE],
}

impl MemoryBus {
    pub fn new() -> Self {
        let standard_user_system_memory = [0xFF; STANDARD_USER_SYSTEM_MEMORY_SIZE];

        Self {
            rom: PC1500_ROM_BYTES,
            standard_user_memory: [0xFF; STANDARD_USER_MEMORY_SIZE],
            standard_user_system_memory,
        }
    }

    pub fn standard_user_memory(&self) -> &[u8; STANDARD_USER_MEMORY_SIZE] {
        &self.standard_user_memory
    }

    pub fn read_byte(&self, addr: u32, pv: bool, pu: bool) -> u8 {
        if addr > 0xFFFF {
            panic!("Attempted to read memory out of bounds at {:04X}", addr);
        }

        match addr {
            STANDARD_USER_MEMORY_BEGIN..=STANDARD_USER_MEMORY_END => {
                self.standard_user_memory[(addr - STANDARD_USER_MEMORY_BEGIN) as usize]
            }
            STANDARD_USER_SYSTEM_MEMORY_BEGIN..=STANDARD_USER_SYSTEM_MEMORY_END => {
                self.standard_user_system_memory
                    [(addr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize]
            }
            ROM_BEGIN..=ROM_END => self.rom[(addr - ROM_BEGIN) as usize],
            _ => {
                // Unmapped memory returns 0xFF
                println!("Reading unmapped memory at {:04X}", addr);
                0xFF
                // Panic for now
                // panic!("Attempted to read unmapped memory at {:04X}", addr);
            }
        }
    }

    pub fn write_byte(&mut self, addr: u32, pv: bool, pu: bool, value: u8) {
        match addr {
            STANDARD_USER_MEMORY_BEGIN..=STANDARD_USER_MEMORY_END => {
                self.standard_user_memory[(addr - STANDARD_USER_MEMORY_BEGIN) as usize] = value;
            }
            STANDARD_USER_SYSTEM_MEMORY_BEGIN..=STANDARD_USER_SYSTEM_MEMORY_END => {
                self.standard_user_system_memory
                    [(addr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize] = value;
            }
            ROM_BEGIN..=ROM_END => {
                // ROM is read-only, ignore writes
            }
            _ => {
                // Unmapped memory, ignore writes
                // panic!("Attempted to write to unmapped memory at {:04X}", addr);
                println!("Writing to unmapped memory at {:04X}", addr);
            }
        }
    }
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self::new()
    }
}
