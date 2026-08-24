pub mod display;
pub mod keyboard;
pub mod lh5_loader;
mod lh5801;
mod lh5810;
mod memory;
mod pd1990ac;

use std::time::Duration;
use std::path::Path;

use display::DisplayController;
pub use keyboard::Key;
use keyboard::Keyboard;
pub use lh5801::Lh5801;
use memory::MemoryBus;

use crate::{lh5810::Lh5810, pd1990ac::Pd1990ac};

pub const NANOS_PER_TICK: Duration = Duration::from_nanos(2600000 / 2);
const TICKS_PER_FRAME: usize = 15000;

pub struct Pc1500 {
    lh5801: Lh5801,
    lh5810: Lh5810,
    pd1990ac: Pd1990ac,
    memory: MemoryBus,
    keyboard: Keyboard,
    display: DisplayController,
}

impl Pc1500 {
    #[must_use]
    pub fn new() -> Self {
        Self {
            lh5801: Lh5801::new(),
            memory: MemoryBus::new(),
            keyboard: Keyboard::new(),
            display: DisplayController::new(),
            lh5810: Lh5810::new(),
            pd1990ac: Pd1990ac::new(),
        }
    }

    fn run(&mut self) {
        self.step_cpu();

        self.step();

        self.keyboard.set_ks(self.lh5810.get_reg(lh5810::Reg::DDA));

        if self.lh5810.int() {
            self.lh5801.set_ir2(true);
        }
    }

    pub fn step_frame(&mut self) {
        let start_ticks = self.lh5801.get_ticks();

        while self.lh5801.get_ticks() - start_ticks < TICKS_PER_FRAME {
            self.run();
        }
    }

    /// Acceso de solo lectura a la CPU LH5801, para inspeccionar registros
    /// (p. ej. desde un arnés de test que carga un `.lh5` y verifica el
    /// estado resultante tras ejecutarlo con [`Pc1500::step_cpu`] /
    /// [`Pc1500::read_byte`]).
    #[must_use]
    pub fn cpu(&self) -> &Lh5801 {
        &self.lh5801
    }

    pub fn display(&mut self) -> &DisplayController {
        self.update_display_buffer();
        &self.display
    }

    pub fn press(&mut self, key: Key) {
        self.keyboard.press(key);
    }

    pub fn release(&mut self, key: Key) {
        self.keyboard.release(key);
    }

    fn read_bit(byte: u8, position: u8) -> bool {
        ((byte >> position) & 0x01) != 0
    }

    fn step(&mut self) {
        if self.lh5810.new_opc() {
            let t = self.lh5810.get_reg(lh5810::Reg::OPC);
            self.pd1990ac.set_data(Self::read_bit(t, 0));
            self.pd1990ac.set_stb(Self::read_bit(t, 1));
            self.pd1990ac.set_clk(Self::read_bit(t, 2));
            self.pd1990ac.set_out_enable(Self::read_bit(t, 3));
            self.pd1990ac.set_c0(Self::read_bit(t, 3));
            self.pd1990ac.set_c1(Self::read_bit(t, 4));
            self.pd1990ac.set_c2(Self::read_bit(t, 5));

            self.pd1990ac.step(self.lh5801.timer_state());
            self.lh5810.set_new_opc(false);
        }

        self.lh5810.set_reg_bit(
            lh5810::Reg::OPB,
            5,
            self.pd1990ac.get_tp(self.lh5801.timer_state()),
        );
        self.lh5810
            .set_reg_bit(lh5810::Reg::OPB, 6, self.pd1990ac.get_data());

        self.lh5810.set_reg_bit(lh5810::Reg::OPB, 3, true); // Export model vs domestic model
        self.lh5810.set_reg_bit(lh5810::Reg::OPB, 4, false); // PB4 to GND

        self.lh5810.step(self.lh5801.timer_state());
    }

    /// Carga un archivo de código máquina LH5801 (formato `.lh5`) en la
    /// memoria de usuario del PC-1500 y configura el Program Counter para
    /// empezar la ejecución desde la dirección de carga.
    ///
    /// # Formato del archivo
    /// ```text
    /// bytes 0-1: dirección de carga (u16 little-endian)
    /// bytes 2-3: longitud del código (u16 little-endian)
    /// bytes 4+ : código máquina LH5801
    /// ```
    ///
    /// # Errores
    /// Devuelve [`lh5_loader::Lh5LoadError`] si el archivo no existe, tiene
    /// formato inválido o la dirección de carga está fuera del rango de
    /// memoria de usuario (`0x4000`–`0x57FF`).
    pub fn load_lh5_file(&mut self, path: &Path) -> Result<(), lh5_loader::Lh5LoadError> {
        use lh5_loader::{read_lh5_file, validate_load_parameters};

        // Un `Pc1500` recién creado (`Pc1500::new()`) arranca con un reset
        // de CPU pendiente (`reset_flag`): el PRIMER `step_cpu()` que se
        // ejecute, sea cuando sea, lo consume saltando al vector de
        // arranque real de la ROM ($FFFE) — machacando cualquier PC que
        // hayamos fijado nosotros antes, incluido el que este método está
        // a punto de fijar más abajo. Si nadie ha consumido ya ese reset
        // (p. ej. llamando a `step_cpu()` una vez a mano, como hace el
        // arnés de test del compilador), el programa cargado nunca llega
        // a ejecutar ni una instrucción: se pierde en cuanto arranca el
        // bucle de la GUI. Consumirlo aquí, dentro de la propia carga,
        // hace que `load_lh5_file` sea seguro de llamar directamente
        // desde `Pc1500::new()` sin que cada llamador tenga que saberlo.
        self.step_cpu();

        let (load_address, machine_code) = read_lh5_file(path)?;
        validate_load_parameters(load_address, machine_code.len())?;

        let start = load_address as usize;
        let end = start + machine_code.len();

        // Escribir el código directamente en standard_user_memory
        const MEM_BEGIN: usize = 0x4000;
        let offset = start - MEM_BEGIN;
        self.memory.standard_user_memory[offset..offset + machine_code.len()]
            .copy_from_slice(&machine_code);

        // Apuntar el PC a la dirección de carga para que la CPU ejecute el código
        self.lh5801.set_pc(load_address);

        println!(
            "Archivo LH5 cargado: {} — 0x{:04X}..0x{:04X} ({} bytes), PC=0x{:04X}",
            path.display(),
            start,
            end - 1,
            machine_code.len(),
            load_address,
        );

        Ok(())
    }
}
