use crate::{Pc1500, host_io, lh5810};

pub(crate) const PC1500_ROM_BYTES: &[u8] =
    include_bytes!("../../Sharp_PC-1500_ROM_Disassembly/PC-1500_ROM-A04.bin");
const INITIAL_VALUE: u8 = 0xFF;

/// Si es `true`, cada lectura o escritura a una dirección no mapeada se
/// anuncia por `stdout`. Apagado por defecto: el arranque en frío de la
/// ROM sondea a propósito memoria que no existe (RAM en $0000/$4800,
/// ROMs de expansión en $8000-$B800, periféricos CE-150/CE-158 en
/// $1B00A/$1B00B/$1DC00...) y esos ~30 mensajes por arranque solo
/// ensuciaban la salida de la GUI y de los tests, además de escribir en
/// `stdout` (bloqueante) desde el bucle caliente de la CPU. Ponerlo a
/// `true` solo para depurar el mapa de memoria.
const LOG_UNMAPPED: bool = false;

// The RAM expansions modelled, each one replacing the one before (they
// are real Sharp modules for the SAME expansion slot, so only one is ever
// fitted; they do not add up):
//   - CE-151 (4KB): "STANDARD USER MEMORY (RAM) 2KB" at 0x4000-0x47FF
//     plus "CE-151 (RAM) 4KB" at 0x4800-0x57FF: 6144 bytes.
//   - CE-155 (8KB): "CE-155 (RAM) 2KB" at 0x3800-0x3FFF, the same
//     "STANDARD USER MEMORY 2KB" in the middle, and "CE-155 (RAM) 6KB" at
//     0x4800-0x5FFF: 10240 bytes.
//   - CE-161 (16KB, the current one), confirmed by 3 independent sources
//     (the service manual's picture; `lhasm.c`, `{ "CE161", ..., 1,
//     0x4000, 0, 0xE161 }`, "Add &4000 before RAM"; and the arithmetic of
//     the official map): the WHOLE module goes BEFORE the standard RAM,
//     not on both sides of it as the CE-155. The service manual's map (for
//     the base PC-1500; the A04 ROM in use is also the PC-1500A's, per the
//     version table at the top of Jeff Birt's listing: "A04 = ROM2 -
//     PC1500 = PC1500A = PC2"):
//       0000H-00FFH: RESERVED AREA (RAM), 256 bytes of the CE-161
//         too. The ROM finds RAM from &0000 at boot, and after NEW0
//         keeps its RESERVE area at &0008-&00C4 and BASIC's program from
//         &00C5 (BCMD_NEW, $C80A: the RAM's first page + &C5). The
//         compiled programs do not use it (they start at &0100).
//       0100H-3FFFH: CE-161 (RAM), 16128 bytes for programs.
//       4000H-47FFH: STANDARD USER MEMORY (RAM), 2048 bytes, the base
//         machine's own, not part of any expansion module.
//       4800H-67FFH: NOT USED (in the service map; not modelled).
//     In one piece for programs: 0x0100-0x47FF = 18176 bytes (against the
//     CE-155's 10240, +77.5%).
/// First address of RAM: the CE-161 starts at &0000. It used to be &0100,
/// so the ROM saw RAM from &0100 and put BASIC at &01C5 after NEW0.
pub(crate) const STANDARD_USER_MEMORY_BEGIN: u32 = 0x0000;
const STANDARD_USER_MEMORY_END: u32 = 0x47FF;
/// The PC-1500A has 4 KB more internal RAM, `&4000-&57FF` instead of
/// `&4000-&47FF` (lhasm's `-m pc1500A`); with the CE-161 in front, user RAM
/// is `&0100-&57FF`. The array is always this big; `user_memory_end` says
/// how much of it the emulated model has.
pub(crate) const PC1500A_USER_MEMORY_END: u32 = 0x57FF;
const STANDARD_USER_MEMORY_SIZE: usize =
    (PC1500A_USER_MEMORY_END - STANDARD_USER_MEMORY_BEGIN + 1) as usize;

const STANDARD_USER_SYSTEM_MEMORY_BEGIN: u32 = 0x7600;
const STANDARD_USER_SYSTEM_MEMORY_END: u32 = 0x7FFF;
/// End of the system RAM of a PC-1500 (see `MemoryBus::system_memory_end`).
const PC1500_SYSTEM_MEMORY_END: u32 = 0x7BFF;
const STANDARD_USER_SYSTEM_MEMORY_SIZE: usize =
    (STANDARD_USER_SYSTEM_MEMORY_END - STANDARD_USER_SYSTEM_MEMORY_BEGIN + 1) as usize;

const ROM_BEGIN: u32 = 0xC000;
const ROM_END: u32 = 0xFFFF;

pub struct MemoryBus {
    pub rom: &'static [u8],
    pub standard_user_memory: [u8; STANDARD_USER_MEMORY_SIZE],
    pub standard_user_system_memory: [u8; STANDARD_USER_SYSTEM_MEMORY_SIZE],
    /// Last address of user RAM: `&47FF` on a PC-1500, `&57FF` on a
    /// PC-1500A. Beyond it, reads and writes behave as unmapped memory.
    pub user_memory_end: u32,
    /// Last address of the system RAM: `&7BFF` on a PC-1500, whose ROM
    /// uses nothing above its input buffer (&7BB0-&7BFF); `&7FFF` on a
    /// PC-1500A, which lhasm gives an `LM` area at &7C01-&7FFF.
    pub system_memory_end: u32,
}

impl MemoryBus {
    /// Memory of a PC-1500 with the CE-161 expansion.
    pub fn new() -> Self {
        Self::with_user_memory_end(STANDARD_USER_MEMORY_END)
    }

    /// Memory of a PC-1500A with the CE-161 expansion.
    pub fn new_pc1500a() -> Self {
        Self::with_user_memory_end(PC1500A_USER_MEMORY_END)
    }

    fn with_user_memory_end(user_memory_end: u32) -> Self {
        let mut standard_user_memory = [0; STANDARD_USER_MEMORY_SIZE];

        const BOOT_PROGRAM: &[u8] = include_bytes!("../../bathyscaph.bin");

        // The BASIC program in RAM at power-on, only to have something to
        // run while debugging (bathyscaph). It is where the ROM puts BASIC's
        // program after `NEW0` with the CE-161 (`&00C5`, see
        // `STANDARD_USER_MEMORY_BEGIN`), so a program loaded over it from
        // the window (`Pc1500::load_basic_program`) also starts there, as
        // on the real machine. It used to be `0x40C5` with the CE-155 and
        // then `0x0100`.
        const FIXTURE_LOAD_ADDR: u16 = 0x00C5;
        let fixture_at = (FIXTURE_LOAD_ADDR as u32 - STANDARD_USER_MEMORY_BEGIN) as usize;

        // The ROM finds the end of a BASIC program by an `&FF` after its
        // last line, and the end pointer (&7867) points at it. lhasm's
        // `.bin` files do not include it, so it is added here; a file that
        // already ends with it (the compiler's tokenized output) keeps its
        // own. Without it, a program that runs past its last line stops
        // with `ERROR 1`.
        let program_len = match BOOT_PROGRAM.last() {
            Some(0xFF) => BOOT_PROGRAM.len() - 1,
            _ => BOOT_PROGRAM.len(),
        };
        standard_user_memory[fixture_at..fixture_at + program_len].copy_from_slice(&BOOT_PROGRAM[..program_len]);
        standard_user_memory[fixture_at + program_len] = 0xFF;

        let mut standard_user_system_memory = [INITIAL_VALUE; STANDARD_USER_SYSTEM_MEMORY_SIZE];

        let [addr_high, addr_low] = FIXTURE_LOAD_ADDR.to_be_bytes();

        standard_user_system_memory[0x7861 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_high;
        standard_user_system_memory[0x7862 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_low;

        standard_user_system_memory[0x7865 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_high;
        standard_user_system_memory[0x7866 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_low;

        standard_user_system_memory[0x7869 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_high;
        standard_user_system_memory[0x786A - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = addr_low;

        let end = FIXTURE_LOAD_ADDR + program_len as u16;
        let [end_high, end_low] = end.to_be_bytes();
        standard_user_system_memory[0x7867 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = end_high;
        standard_user_system_memory[0x7868 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = end_low;

        standard_user_system_memory[0x79D0 - STANDARD_USER_SYSTEM_MEMORY_BEGIN as usize] = 0;

        let system_memory_end = if user_memory_end == PC1500A_USER_MEMORY_END {
            STANDARD_USER_SYSTEM_MEMORY_END
        } else {
            PC1500_SYSTEM_MEMORY_END
        };
        Self {
            rom: PC1500_ROM_BYTES,
            standard_user_memory,
            standard_user_system_memory,
            user_memory_end,
            system_memory_end,
        }
    }
}

impl Pc1500 {
    fn mirror_addresses(&self, addr: u32) -> u32 {
        if addr >= 0x7000 && addr <= 0x75FF {
            return addr & 0x1FF | 0x7600;
        }

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
            a if (STANDARD_USER_MEMORY_BEGIN..=self.memory.user_memory_end).contains(&a) => {
                self.memory.standard_user_memory[(addr - STANDARD_USER_MEMORY_BEGIN) as usize]
            }
            a if (STANDARD_USER_SYSTEM_MEMORY_BEGIN..=self.memory.system_memory_end).contains(&a) => {
                self.memory.standard_user_system_memory
                    [(addr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize]
            }
            ROM_BEGIN..=ROM_END => self.memory.rom[(addr - ROM_BEGIN) as usize],
            // La tabla virtual de `CLOAD`/`CSAVE`/`MERGE`/`CHAIN`, solo
            // con `PV`=0 (ver `host_io.rs`). Es de solo lectura: las
            // escrituras caen en el brazo "no mapeado" de `write_byte`.
            host_io::TABLE_BEGIN..=host_io::TABLE_END if !self.lh5801.pv() => {
                host_io::rom_byte(addr)
            }
            // Unmapped
            _ => {
                if LOG_UNMAPPED {
                    println!("Read from unmapped address: {addr:#06X}");
                }
                INITIAL_VALUE
            }
        }
    }

    /// Un byte de la ventana de expansión de ME0 ($8000-$BFFF) como lo
    /// vería la CPU con el `PV` dado, tenga el que tenga ahora (fuera de
    /// esa ventana, el de [`Pc1500::read_byte`]).
    ///
    /// `PV` (patilla del LH5801) elige entre dos bancos de ROMs de
    /// expansión en esa ventana: la CE-150 usa `PV`=0 $A000-$BFFF y la
    /// CE-158 `PV`=1 $8000-$9FFF. El emulador solo pone ahí la tabla
    /// virtual de `host_io.rs` ($9800-$9FFF con `PV`=0); lo demás se lee
    /// vacío ($FF), como sin nada enchufado. Lo usa el lector de tablas
    /// de palabras clave de `basic.rs`, que tiene que recorrer los dos
    /// bancos como el tokenizador de la ROM sin cambiar el `PV` real.
    pub(crate) fn read_banked(&self, addr: u16, pv: bool) -> u8 {
        let wide = u32::from(addr);
        if !(0x8000..0xC000).contains(&wide) {
            return self.read_byte(wide);
        }
        if !pv && (host_io::TABLE_BEGIN..=host_io::TABLE_END).contains(&wide) {
            return host_io::rom_byte(wide);
        }
        INITIAL_VALUE
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
            a if (STANDARD_USER_MEMORY_BEGIN..=self.memory.user_memory_end).contains(&a) => {
                self.memory.standard_user_memory[(addr - STANDARD_USER_MEMORY_BEGIN) as usize] =
                    value;
            }
            a if (STANDARD_USER_SYSTEM_MEMORY_BEGIN..=self.memory.system_memory_end).contains(&a) => {
                self.memory.standard_user_system_memory
                    [(addr - STANDARD_USER_SYSTEM_MEMORY_BEGIN) as usize] = value;
            }
            ROM_BEGIN..=ROM_END => {
                // ROM is read-only, ignore writes
            }
            // Unmapped
            _ => {
                if LOG_UNMAPPED {
                    println!("Write to unmapped address: {addr:#06X}");
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
