pub mod display;
pub mod keyboard;
mod lh5801;
mod lh5810;
mod memory;
mod pd1990ac;

use std::time::Duration;

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
}
