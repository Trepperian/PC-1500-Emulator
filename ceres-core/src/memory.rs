use crate::{Pc1500, lh5810};

const PC1500_ROM_BYTES: &[u8] =
    include_bytes!("../../Sharp_PC-1500_ROM_Disassembly/PC-1500_ROM-A04.bin");
const INITIAL_VALUE: u8 = 0xFF;

const STANDARD_USER_MEMORY_BEGIN: u32 = 0x3800;
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
    pub rom: &'static [u8],
    pub standard_user_memory: [u8; STANDARD_USER_MEMORY_SIZE],
    pub standard_user_system_memory: [u8; STANDARD_USER_SYSTEM_MEMORY_SIZE],
}

impl MemoryBus {
    pub fn new() -> Self {
        Self {
            rom: PC1500_ROM_BYTES,
            standard_user_memory: [INITIAL_VALUE; STANDARD_USER_MEMORY_SIZE],
            standard_user_system_memory: [INITIAL_VALUE; STANDARD_USER_SYSTEM_MEMORY_SIZE],
        }
    }
}

impl Pc1500 {
    fn mirror_addresses(&self, addr: u32) -> u32 {
        if addr >= 0x7000 && addr <= 0x75FF {
            return addr & 0x1FF | 0x7600;
        }

        if addr >= 0x7C00 && addr <= 0x7FFF {
            return addr - 0x400;
        }

        addr
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        let addr = self.mirror_addresses(addr);

        match addr {
            STANDARD_USER_MEMORY_BEGIN..=STANDARD_USER_MEMORY_END => {
                self.memory.standard_user_memory[(addr - STANDARD_USER_MEMORY_BEGIN) as usize]
            }
            STANDARD_USER_SYSTEM_MEMORY_BEGIN..=STANDARD_USER_SYSTEM_MEMORY_END => {
                self.memory.standard_user_system_memory
                    [(addr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize]
            }
            ROM_BEGIN..=ROM_END => self.memory.rom[(addr - ROM_BEGIN) as usize],
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

            _ => {
                let pu = self.lh5801.pu();
                let pv = self.lh5801.pv();
                // println!(
                //     "Reading unmapped memory at {:04X}, PU: {}, PV: {}",
                //     addr, pu, pv
                // );
                INITIAL_VALUE
                // Panic for now
                // panic!("Attempted to read unmapped memory at {:04X}", addr);
            }
        }
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        let addr = self.mirror_addresses(addr);

        match addr {
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

            _ => {
                // Unmapped memory, ignore writes
                // panic!("Attempted to write to unmapped memory at {:04X}", addr);
                let pu = self.lh5801.pu();
                let pv = self.lh5801.pv();
                // println!(
                //     "Writing to unmapped memory at {:04X}, PU: {}, PV: {}",
                //     addr, pu, pv
                // );
            }
        }
    }

    // pub fn clear_display_memory(&mut self) {
    //     for ind in 0..=0x4F {
    //         let adr = 0x7600 + ind;
    //         self.memory.standard_user_system_memory
    //             [(adr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize] = 0;
    //     }

    //     for ind in 0..=0x4F {
    //         let adr = 0x7700 + ind;
    //         self.memory.standard_user_system_memory
    //             [(adr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize] = 0;
    //     }
    // }
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self::new()
    }
}
