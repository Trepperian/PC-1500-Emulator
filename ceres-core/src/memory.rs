use crate::{Pc1500, lh5810};

const PC1500_ROM_BYTES: &[u8] =
    include_bytes!("../../Sharp_PC-1500_ROM_Disassembly/PC-1500_ROM-A04.bin");
const INITIAL_VALUE: u8 = 0xFF;

// Historial de expansiones de RAM modeladas (cada una sustituyó a la
// anterior — son módulos reales de Sharp, mutuamente excluyentes, que
// ocupan la MISMA ranura de expansión, no memoria adicional):
//   - CE-151 (4KB): "STANDARD USER MEMORY (RAM) 2KB" en 0x4000-0x47FF
//     más "CE-151 (RAM) 4KB" en 0x4800-0x57FF — 6144 bytes.
//   - CE-155 (8KB): "CE-155 (RAM) 2KB" en 0x3800-0x3FFF, la misma
//     "STANDARD USER MEMORY 2KB" en el medio, y "CE-155 (RAM) 6KB" en
//     0x4800-0x5FFF — 10240 bytes.
//   - CE-161 (16KB, actual): confirmado en 3 fuentes independientes
//     (imagen del manual de servicio, `lhasm.c` — `{ "CE161", ..., 1,
//     0x4000, 0, 0xE161 }`, "Add &4000 before RAM" — y coherencia
//     aritmética con el mapa oficial): añade el módulo COMPLETO
//     ANTES de la RAM estándar, no repartido a ambos lados como
//     CE-155. Mapa real de servicio (para PC-1500 base, la única
//     variante con ROM disponible en este proyecto — no existe volcado
//     de ROM de PC-1500A):
//       0000H-00FFH: ÁREA RESERVADA (RAM) — 256 bytes, NO usada por el
//         compilador (decisión explícita: no se ha confirmado qué la
//         usa en hardware real, y el coste de evitarla es marginal).
//       0100H-3FFFH: CE-161 (RAM) — 16128 bytes utilizables.
//       4000H-47FFH: STANDARD USER MEMORY (RAM) — 2048 bytes, la misma
//         de siempre, inherente a la máquina base, no parte de ningún
//         módulo de expansión.
//       4800H-67FFH: NOT USED (según el mapa de servicio — no modelado).
//     Total contiguo utilizable: 0x0100-0x47FF = 18176 bytes (frente a
//     los 10240 de CE-155, +77.5%).
const STANDARD_USER_MEMORY_BEGIN: u32 = 0x0100;
const STANDARD_USER_MEMORY_END: u32 = 0x47FF;
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

        const BATHYSCAP: &[u8] = include_bytes!("../../jackpot.bin");

        // Dirección de carga del fixture histórico tokenizado. Antes
        // `0x40C5` (elegida cuando `STANDARD_USER_MEMORY` llegaba hasta
        // `0x5FFF` con CE-155, dejando ~8000 bytes de margen por
        // encima) — con CE-161 el techo real bajó a `0x47FF`, dejando
        // solo ~1850 bytes de margen desde `0x40C5`, insuficiente para
        // programas tokenizados más grandes que bathyscaph.bin/
        // bombing.bin (ej. donkeykong.bin, 1870 bytes, ya no cabía:
        // panic real "range end index 18195 out of range for slice of
        // length 18176"). Movida al mismo INICIO de la RAM de usuario
        // (`STANDARD_USER_MEMORY_BEGIN`) para maximizar el margen
        // disponible de cara a futuros programas del corpus, ahora que
        // cargar el original tokenizado antes de comparar contra
        // nuestro `.lh5` es la metodología estándar para cada programa
        // nuevo que se pruebe.
        const FIXTURE_LOAD_ADDR: u16 = STANDARD_USER_MEMORY_BEGIN as u16;

        standard_user_memory[0..BATHYSCAP.len()].copy_from_slice(&BATHYSCAP);

        let mut standard_user_system_memory = [INITIAL_VALUE; STANDARD_USER_SYSTEM_MEMORY_SIZE];

        let [addr_high, addr_low] = FIXTURE_LOAD_ADDR.to_be_bytes();

        standard_user_system_memory[0x7861 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_high;
        standard_user_system_memory[0x7862 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_low;

        standard_user_system_memory[0x7865 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_high;
        standard_user_system_memory[0x7866 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_low;

        standard_user_system_memory[0x7869 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_high;
        standard_user_system_memory[0x786A - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_low;

        let end = FIXTURE_LOAD_ADDR + BATHYSCAP.len() as u16;
        let [end_high, end_low] = end.to_be_bytes();
        standard_user_system_memory[0x7867 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = end_high;
        standard_user_system_memory[0x7868 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = end_low;

        standard_user_system_memory[0x79D0 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = 0;
             standard_user_system_memory[0x79D0 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = 0;

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
