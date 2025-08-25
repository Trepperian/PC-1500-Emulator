use crate::{Pc1500, lh5810};

const PC1500_ROM_BYTES: &[u8] =
    include_bytes!("../../Sharp_PC-1500_ROM_Disassembly/PC-1500_ROM-A04.bin");
const INITIAL_VALUE: u8 = 0xFF;

const STANDARD_USER_MEMORY_BEGIN: u32 = 0x4000;
const STANDARD_USER_MEMORY_END: u32 = 0x57FF;
const STANDARD_USER_MEMORY_SIZE: usize =
    (STANDARD_USER_MEMORY_END - STANDARD_USER_MEMORY_BEGIN + 1) as usize;

const STANDARD_USER_SYSTEM_MEMORY_BEGIN: u32 = 0x7600;
const STANDARD_USER_SYSTEM_MEMORY_END: u32 = 0x7FFF;
const STANDARD_USER_SYSTEM_MEMORY_SIZE: usize =
    (STANDARD_USER_SYSTEM_MEMORY_END - STANDARD_USER_SYSTEM_MEMORY_BEGIN + 1) as usize;

const ROM_BEGIN: u32 = 0xC000;
const ROM_END: u32 = 0xFFFF;

pub struct MemoryBus {
    pub rom: &'static [u8],
    pub standard_user_memory: [u8; STANDARD_USER_MEMORY_SIZE],
    pub standard_user_system_memory: [u8; STANDARD_USER_SYSTEM_MEMORY_SIZE],
}

impl MemoryBus {
    pub fn new() -> Self {
        let mut standard_user_memory = [0; STANDARD_USER_MEMORY_SIZE];

        const BATHYSCAP: &[u8] = include_bytes!("../../bathyscaph.bin");

        // Copy from 0x40C5
        standard_user_memory[0x40C5 - STANDARD_USER_MEMORY_BEGIN as usize
            ..0x40C5 - STANDARD_USER_MEMORY_BEGIN as usize + BATHYSCAP.len()]
            .copy_from_slice(&BATHYSCAP);

        let mut standard_user_system_memory = [INITIAL_VALUE; STANDARD_USER_SYSTEM_MEMORY_SIZE];

        standard_user_system_memory[0x7861 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = 0x40;
        standard_user_system_memory[0x7862 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = 0xC5;

        standard_user_system_memory[0x7865 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = 0x40;
        standard_user_system_memory[0x7866 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = 0xC5;

        let end = 0x40C5 + BATHYSCAP.len() as u16;
        let [end_high, end_low] = end.to_be_bytes();
        standard_user_system_memory[0x7867 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = end_high;
        standard_user_system_memory[0x7868 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = end_low;

        Self {
            rom: PC1500_ROM_BYTES,
            standard_user_memory,
            standard_user_system_memory,
        }
    }
}

impl Pc1500 {
    fn mirror_addresses(&self, addr: u32) -> u32 {
        if addr >= 0x7000 && addr <= 0x75FF {
            return addr & 0x1FF | 0x7600;
        }

        // if addr >= 0x7C00 && addr <= 0x7FFF {
        //     return addr - 0x400;
        // }

        addr
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        let addr = self.mirror_addresses(addr);

        match addr {
            // ME1
            // LH5810 registers
            0x1F005 => self.lh5810.get_reg(lh5810::Reg::U),
            0x1F006 => self.lh5810.get_reg(lh5810::Reg::L),
            0x1F007 => self.lh5810.get_reg(lh5810::Reg::F),
            0x1F008 => self.lh5810.get_reg(lh5810::Reg::OPC),
            0x1F009 => self.lh5810.get_reg(lh5810::Reg::G),
            0x1F00A => self.lh5810.get_reg(lh5810::Reg::MSK),
            0x1F00B => self.lh5810.get_reg(lh5810::Reg::IF),
            0x1F00C => self.lh5810.get_reg(lh5810::Reg::DDA),
            0x1F00D => self.lh5810.get_reg(lh5810::Reg::DDB),
            0x1F00E => self.lh5810.get_reg(lh5810::Reg::OPA),
            0x1F00F => self.lh5810.get_reg(lh5810::Reg::OPB),
            // ME0
            STANDARD_USER_MEMORY_BEGIN..=STANDARD_USER_MEMORY_END => {
                self.memory.standard_user_memory[(addr - STANDARD_USER_MEMORY_BEGIN) as usize]
            }
            STANDARD_USER_SYSTEM_MEMORY_BEGIN..=STANDARD_USER_SYSTEM_MEMORY_END => {
                self.memory.standard_user_system_memory
                    [(addr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize]
            }
            ROM_BEGIN..=ROM_END => self.memory.rom[(addr - ROM_BEGIN) as usize],
            // Unmapped
            _ => {
                println!("Read from unmapped address: {:#06X}", addr);
                INITIAL_VALUE
            }
        }
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        let addr = self.mirror_addresses(addr);

        match addr {
            // ME1
            0x1F004 => self
                .lh5810
                .set_reg(lh5810::Reg::RESET, value, self.lh5801.timer_state()),
            0x1F005 => self
                .lh5810
                .set_reg(lh5810::Reg::U, value, self.lh5801.timer_state()),
            0x1F006 => self
                .lh5810
                .set_reg(lh5810::Reg::L, value, self.lh5801.timer_state()),
            0x1F007 => self
                .lh5810
                .set_reg(lh5810::Reg::F, value, self.lh5801.timer_state()),
            0x1F008 => self
                .lh5810
                .set_reg(lh5810::Reg::OPC, value, self.lh5801.timer_state()),
            0x1F009 => self
                .lh5810
                .set_reg(lh5810::Reg::G, value, self.lh5801.timer_state()),
            0x1F00A => self
                .lh5810
                .set_reg(lh5810::Reg::MSK, value, self.lh5801.timer_state()),
            0x1F00B => self
                .lh5810
                .set_reg(lh5810::Reg::IF, value, self.lh5801.timer_state()),
            0x1F00C => self
                .lh5810
                .set_reg(lh5810::Reg::DDA, value, self.lh5801.timer_state()),
            0x1F00D => self
                .lh5810
                .set_reg(lh5810::Reg::DDB, value, self.lh5801.timer_state()),
            0x1F00E => self
                .lh5810
                .set_reg(lh5810::Reg::OPA, value, self.lh5801.timer_state()),
            0x1F00F => self
                .lh5810
                .set_reg(lh5810::Reg::OPB, value, self.lh5801.timer_state()),
            // ME0
            STANDARD_USER_MEMORY_BEGIN..=STANDARD_USER_MEMORY_END => {
                self.memory.standard_user_memory[(addr - STANDARD_USER_MEMORY_BEGIN) as usize] =
                    value;
            }
            STANDARD_USER_SYSTEM_MEMORY_BEGIN..=STANDARD_USER_SYSTEM_MEMORY_END => {
                self.memory.standard_user_system_memory
                    [(addr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize] = value;
            }
            ROM_BEGIN..=ROM_END => {
                // ROM is read-only, ignore writes
            }
            // Unmapped
            _ => {
                println!("Write to unmapped address: {:#06X}", addr);
            }
        }
    }
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self::new()
    }
}
