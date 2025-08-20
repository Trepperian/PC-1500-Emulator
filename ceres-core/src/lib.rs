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

pub const NANOS_PER_TICK: Duration = Duration::from_nanos(2600000 / 2);
const TICKS_PER_FRAME: usize = 1000;

pub struct Pc1500 {
    cpu: Lh5801,
    memory: MemoryBus,
    joypad: Keyboard,
    display: DisplayController,
}

impl Pc1500 {
    #[must_use]
    pub fn new() -> Self {
        let mut memory = MemoryBus::new();

        Self {
            cpu: Lh5801::new(&mut memory),
            memory,
            joypad: Keyboard::new(),
            display: DisplayController::new(),
        }
    }

    fn run(&mut self) {
        self.cpu.step(&mut self.memory);
    }

    pub fn step_frame(&mut self) {
        let start_ticks = self.cpu.get_ticks();

        while self.cpu.get_ticks() - start_ticks < TICKS_PER_FRAME {
            self.run();
        }
    }

    pub fn display(&mut self) -> &DisplayController {
        self.display.update_buffer(&self.memory);
        &self.display
    }

    pub fn press(&mut self, key: Key) {
        self.joypad.press(key);
    }

    pub fn release(&mut self, key: Key) {
        self.joypad.release(key);
    }
}
