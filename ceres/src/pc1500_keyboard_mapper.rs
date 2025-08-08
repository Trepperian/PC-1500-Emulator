/// PC-1500 specific keyboard mapping
/// 
/// This module handles the mapping between physical PC keyboard keys
/// and the PC-1500 virtual keyboard matrix.

use ceres_core::pc1500::joypad::Key as Pc1500Key;
use std::collections::HashSet;

// Enum simple para teclas físicas - simplificado para el test
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalKey {
    // Numbers
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    
    // Letters  
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    
    // Special keys
    Space, Enter,
    
    // Function keys
    F1, F2, F3, F4, F5, F6,
    
    // Operators and symbols
    Plus, Minus, Equals, Period, Slash,
    
    // Arrow keys
    ArrowLeft, ArrowRight, ArrowUp, ArrowDown,
}

/// Maps physical PC keyboard keys to PC-1500 keys
pub struct Pc1500KeyMapper {
    // Current pressed keys (for tracking state)
    pressed_keys: HashSet<Pc1500Key>,
}

impl Pc1500KeyMapper {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
        }
    }
    
    /// Map a physical key to PC-1500 key
    /// Returns Some(pc1500_key) if mapping exists, None otherwise
    pub fn map_physical_to_pc1500(&self, key: PhysicalKey) -> Option<Pc1500Key> {
        match key {
            // Numbers
            PhysicalKey::Num0 => Some(Pc1500Key::Zero),
            PhysicalKey::Num1 => Some(Pc1500Key::One),
            PhysicalKey::Num2 => Some(Pc1500Key::Two),
            PhysicalKey::Num3 => Some(Pc1500Key::Three),
            PhysicalKey::Num4 => Some(Pc1500Key::Four),
            PhysicalKey::Num5 => Some(Pc1500Key::Five),
            PhysicalKey::Num6 => Some(Pc1500Key::Six),
            PhysicalKey::Num7 => Some(Pc1500Key::Seven),
            PhysicalKey::Num8 => Some(Pc1500Key::Eight),
            PhysicalKey::Num9 => Some(Pc1500Key::Nine),
            
            // Letters
            PhysicalKey::A => Some(Pc1500Key::A),
            PhysicalKey::B => Some(Pc1500Key::B),
            PhysicalKey::C => Some(Pc1500Key::C),
            PhysicalKey::D => Some(Pc1500Key::D),
            PhysicalKey::E => Some(Pc1500Key::E),
            PhysicalKey::F => Some(Pc1500Key::F),
            PhysicalKey::G => Some(Pc1500Key::G),
            PhysicalKey::H => Some(Pc1500Key::H),
            PhysicalKey::I => Some(Pc1500Key::I),
            PhysicalKey::J => Some(Pc1500Key::J),
            PhysicalKey::K => Some(Pc1500Key::K),
            PhysicalKey::L => Some(Pc1500Key::L),
            PhysicalKey::M => Some(Pc1500Key::M),
            PhysicalKey::N => Some(Pc1500Key::N),
            PhysicalKey::O => Some(Pc1500Key::O),
            PhysicalKey::P => Some(Pc1500Key::P),
            PhysicalKey::Q => Some(Pc1500Key::Q),
            PhysicalKey::R => Some(Pc1500Key::R),
            PhysicalKey::S => Some(Pc1500Key::S),
            PhysicalKey::T => Some(Pc1500Key::T),
            PhysicalKey::U => Some(Pc1500Key::U),
            PhysicalKey::V => Some(Pc1500Key::V),
            PhysicalKey::W => Some(Pc1500Key::W),
            PhysicalKey::X => Some(Pc1500Key::X),
            PhysicalKey::Y => Some(Pc1500Key::Y),
            PhysicalKey::Z => Some(Pc1500Key::Z),
            
            // Special keys
            PhysicalKey::Space => Some(Pc1500Key::Space),
            PhysicalKey::Enter => Some(Pc1500Key::Enter),
            
            // Function keys
            PhysicalKey::F1 => Some(Pc1500Key::F1),
            PhysicalKey::F2 => Some(Pc1500Key::F2),
            PhysicalKey::F3 => Some(Pc1500Key::F3),
            PhysicalKey::F4 => Some(Pc1500Key::F4),
            PhysicalKey::F5 => Some(Pc1500Key::F5),
            PhysicalKey::F6 => Some(Pc1500Key::F6),
            
            // Operators and symbols
            PhysicalKey::Plus => Some(Pc1500Key::Plus),
            PhysicalKey::Minus => Some(Pc1500Key::Minus),
            PhysicalKey::Equals => Some(Pc1500Key::Equals),
            PhysicalKey::Period => Some(Pc1500Key::Dot),
            PhysicalKey::Slash => Some(Pc1500Key::Slash),
            
            // Arrow keys
            PhysicalKey::ArrowLeft => Some(Pc1500Key::Left),
            PhysicalKey::ArrowRight => Some(Pc1500Key::Right),
            PhysicalKey::ArrowUp => Some(Pc1500Key::Up),
            PhysicalKey::ArrowDown => Some(Pc1500Key::Down),
        }
    }
    
    /// Handle a key press event
    pub fn handle_key_pressed(&mut self, key: PhysicalKey) -> Option<Pc1500Key> {
        if let Some(pc1500_key) = self.map_physical_to_pc1500(key) {
            self.pressed_keys.insert(pc1500_key);
            Some(pc1500_key)
        } else {
            None
        }
    }
    
    /// Handle a key release event  
    pub fn handle_key_released(&mut self, key: PhysicalKey) -> Option<Pc1500Key> {
        if let Some(pc1500_key) = self.map_physical_to_pc1500(key) {
            self.pressed_keys.remove(&pc1500_key);
            Some(pc1500_key)
        } else {
            None
        }
    }
    
    /// Get all currently pressed PC-1500 keys
    pub fn get_pressed_keys(&self) -> &HashSet<Pc1500Key> {
        &self.pressed_keys
    }
    
    /// Check if a specific PC-1500 key is currently pressed
    pub fn is_pressed(&self, key: Pc1500Key) -> bool {
        self.pressed_keys.contains(&key)
    }
    
    /// Clear all pressed keys
    pub fn clear_all(&mut self) {
        self.pressed_keys.clear();
    }
    
    /// Get a help text showing the key mappings
    pub fn get_help_text() -> &'static str {
        "PC-1500 Keyboard Mappings:\n\
         Numbers: 0-9 → PC-1500 0-9\n\
         Letters: A-Z → PC-1500 A-Z (where available)\n\
         Space → SPACE\n\
         Enter → ENTER\n\
         F1-F6 → F1-F6\n\
         +/= → Plus\n\
         - → Minus\n\
         = → Equals\n\
         . → Dot\n\
         / → Slash\n\
         Arrow Keys → PC-1500 Arrow Keys"
    }
}
