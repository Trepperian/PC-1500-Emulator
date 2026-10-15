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

/// Real nanoseconds per LH5801 machine cycle, from the PC-1500's real
/// oscillator (2.6MHz), which the CPU divides by 2 inside: ~1.3MHz. This
/// constant used to exist with the wrong units
/// (`Duration::from_nanos(2_600_000 / 2)` = 1.3ms, not ~769ns, a factor of
/// ~1690) and, worse, nothing in the repository used it: `step_frame()`
/// (below) is only a budget of CPU cycles, with no idea of real time. See
/// `FRAME_DURATION`, which does use it.
const NANOS_PER_TICK: u64 = 1_000_000_000 / (2_600_000 / 2);

const TICKS_PER_FRAME: usize = 15000;

/// The real time (the PC-1500's own clock, not the host's) that one call
/// to [`Pc1500::step_frame`] stands for: `TICKS_PER_FRAME` real LH5801
/// machine cycles at ~1.3MHz.
///
/// `step_frame()` itself knows nothing of real time: it is a loop that
/// runs until it has spent exactly `TICKS_PER_FRAME` cycles, without
/// looking at the clock. Before this constant, the GUI
/// (`ceres-egui::Pc1500App`) called `step_frame()` once per repaint, and
/// the repaints were not paced to real time either (only to `vsync`), so
/// the emulation's speed was tied to the user's monitor refresh rate, not
/// to the real PC-1500. This constant is what any caller needs to pace its
/// calls to `step_frame()` to real time.
pub const FRAME_DURATION: Duration = Duration::from_nanos(TICKS_PER_FRAME as u64 * NANOS_PER_TICK);

pub struct Pc1500 {
    lh5801: Lh5801,
    lh5810: Lh5810,
    pd1990ac: Pd1990ac,
    memory: MemoryBus,
    keyboard: Keyboard,
    display: DisplayController,
}

impl Pc1500 {
    /// A PC-1500 with the CE-161 expansion (user RAM `&0100-&47FF`).
    #[must_use]
    pub fn new() -> Self {
        Self::with_memory(MemoryBus::new())
    }

    /// A PC-1500A with the CE-161 expansion: 4 KB more RAM, user RAM
    /// `&0100-&57FF`. Same ROM: A04 is the PC-1500A's ROM too (the version
    /// table at the top of Jeff Birt's listing: "A04 = ROM2 - PC1500 =
    /// PC1500A = PC2").
    #[must_use]
    pub fn new_pc1500a() -> Self {
        Self::with_memory(MemoryBus::new_pc1500a())
    }

    /// Last address of user RAM of the emulated model.
    #[must_use]
    pub fn user_memory_end(&self) -> u16 {
        self.memory.user_memory_end as u16
    }

    fn with_memory(memory: MemoryBus) -> Self {
        Self {
            lh5801: Lh5801::new(),
            memory,
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

    /// Executes one CPU instruction together with the peripherals (timers,
    /// keyboard strobe, interrupts): the unit of work `step_frame` repeats.
    /// Lets a caller observe the machine between two instructions, e.g. a
    /// test harness that hooks ROM routines by address, without losing the
    /// hardware timing the ROM relies on (`step_cpu` alone skips it).
    pub fn step_instruction(&mut self) {
        self.run();
    }

    /// Read-only access to the LH5801 CPU, to look at its registers (e.g.
    /// from a test harness that loads a `.lh5` and checks the state it leaves
    /// after running it with [`Pc1500::step_cpu`] / [`Pc1500::read_byte`]).
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

    /// Loads a file of LH5801 machine code (the `.lh5` format) into the
    /// PC-1500's user memory and sets the Program Counter to start running at
    /// the load address.
    ///
    /// # The file
    /// ```text
    /// bytes 0-1: the load address (u16 little-endian)
    /// bytes 2-3: the length of the code (u16 little-endian)
    /// bytes 4+ : the LH5801 machine code
    /// ```
    ///
    /// # Errors
    /// Returns [`lh5_loader::Lh5LoadError`] if the file does not exist, its
    /// format is wrong or the load address is outside user memory (`0x0100`-
    /// `0x47FF`, or `0x57FF` on a PC-1500A).
    pub fn load_lh5_file(&mut self, path: &Path) -> Result<(), lh5_loader::Lh5LoadError> {
        use lh5_loader::{read_lh5_file, validate_load_parameters};

        // A new `Pc1500` (`Pc1500::new()`) starts with a CPU reset pending
        // (`reset_flag`): the FIRST `step_cpu()`, whenever it runs, takes it by
        // jumping to the ROM's real boot vector ($FFFE), overwriting any PC set
        // before, including the one this method sets below. If nobody has taken
        // that reset yet (e.g. by calling `step_cpu()` once by hand, as the
        // compiler's test harness does), the program loaded never runs a single
        // instruction: it is lost as soon as the GUI's loop starts. Taking it
        // here, inside the load, makes `load_lh5_file` safe to call straight
        // after `Pc1500::new()` without every caller having to know.
        self.step_cpu();

        let (load_address, machine_code) = read_lh5_file(path)?;
        validate_load_parameters(load_address, machine_code.len(), self.user_memory_end())?;

        let start = load_address as usize;
        let end = start + machine_code.len();

        // Write the code straight into standard_user_memory.
        let offset = start - memory::STANDARD_USER_MEMORY_BEGIN as usize;
        self.memory.standard_user_memory[offset..offset + machine_code.len()]
            .copy_from_slice(&machine_code);

        // Point PC at the load address so that the CPU runs the code
        self.lh5801.set_pc(load_address);
        // If the CPU was still stopped (the previous program reached
        // END/STOP), take it out of HALT; otherwise step_cpu() runs nothing
        // even with PC at the program just loaded (see Lh5801::clear_halt).
        self.lh5801.clear_halt();

        println!(
            "LH5 file loaded: {} at 0x{:04X}..0x{:04X} ({} bytes), PC=0x{:04X}",
            path.display(),
            start,
            end - 1,
            machine_code.len(),
            load_address,
        );

        Ok(())
    }
}
