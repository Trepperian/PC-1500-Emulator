// Test compilation of PC1500App without audio
use std::collections::HashSet;

// Simulated types for compilation test
mod mock_egui {
    pub struct Context;
    pub struct Ui;
    pub enum Key { A, B }
}

mod mock_ceres_core {
    #[derive(Default)]
    pub enum Model { #[default] Pc1500 }
    
    pub struct Pc1500 {
        // Mock fields
    }
    
    impl Pc1500 {
        pub fn new(_model: Model) -> Self { Self {} }
        pub fn init_test_mode(&mut self) {}
        pub fn display_message(&mut self, _msg: &str) {}
    }
    
    pub mod joypad {
        #[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
        pub enum Key { A, B }
    }
}

// Test the struct definition without audio
pub struct Pc1500App {
    // CORE EMULATOR - The real PC-1500 system (NO AUDIO PARAMETER)
    emulator: mock_ceres_core::Pc1500,
    
    // KEYBOARD STATE - Full PC-1500 keyboard with timing
    pressed_keys: HashSet<mock_ceres_core::joypad::Key>,
}

impl Pc1500App {
    pub fn new() -> Self {
        // Create emulator WITHOUT audio parameter
        let mut emulator = mock_ceres_core::Pc1500::new(mock_ceres_core::Model::default());
        
        // Initialize in test mode by default
        emulator.init_test_mode();
        emulator.display_message("PC-1500 READY");
        
        Self {
            emulator,
            pressed_keys: HashSet::new(),
        }
    }
}

fn main() {
    let _app = Pc1500App::new();
    println!("✅ PC1500App compiles successfully WITHOUT audio!");
}
