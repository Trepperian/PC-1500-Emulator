/// PC-1500 specific keyboard mapping for egui
/// 
/// This module handles the mapping between physical PC keyboard keys
/// and the PC-1500 virtual keyboard matrix.

use eframe::egui::Key;
use ceres_core::joypad::Key as Pc1500Key;
use std::collections::HashSet;

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
    pub fn map_physical_to_pc1500(&self, key: Key) -> Option<Pc1500Key> {
        match key {
            // Numbers
            Key::Num0 => Some(Pc1500Key::Zero),
            Key::Num1 => Some(Pc1500Key::One),
            Key::Num2 => Some(Pc1500Key::Two),
            Key::Num3 => Some(Pc1500Key::Three),
            Key::Num4 => Some(Pc1500Key::Four),
            Key::Num5 => Some(Pc1500Key::Five),
            Key::Num6 => Some(Pc1500Key::Six),
            Key::Num7 => Some(Pc1500Key::Seven),
            Key::Num8 => Some(Pc1500Key::Eight),
            Key::Num9 => Some(Pc1500Key::Nine),
            
            // Letters
            Key::A => Some(Pc1500Key::A),
            Key::B => Some(Pc1500Key::B),
            Key::C => Some(Pc1500Key::C),
            Key::D => Some(Pc1500Key::D),
            Key::E => Some(Pc1500Key::E),
            Key::F => Some(Pc1500Key::F),
            Key::G => Some(Pc1500Key::G),
            Key::H => Some(Pc1500Key::H),
            Key::I => Some(Pc1500Key::I),
            Key::J => Some(Pc1500Key::J),
            Key::K => Some(Pc1500Key::K),
            Key::L => Some(Pc1500Key::L),
            Key::M => Some(Pc1500Key::M),
            Key::N => Some(Pc1500Key::N),
            Key::O => Some(Pc1500Key::O),
            Key::P => Some(Pc1500Key::P),
            Key::Q => Some(Pc1500Key::Q),
            Key::R => Some(Pc1500Key::R),
            Key::S => Some(Pc1500Key::S),
            Key::T => Some(Pc1500Key::T),
            Key::U => Some(Pc1500Key::U),
            Key::V => Some(Pc1500Key::V),
            Key::W => Some(Pc1500Key::W),
            Key::X => Some(Pc1500Key::X),
            Key::Y => Some(Pc1500Key::Y),
            Key::Z => Some(Pc1500Key::Z),
            
            // Special keys
            Key::Space => Some(Pc1500Key::Space),
            Key::Enter => Some(Pc1500Key::Enter),
            
            // Function keys - Solo F5 y F6 disponibles en PC-1500
            // Key::F1 => Some(Pc1500Key::F1), // Removido - no existe en el layout real
            // Key::F2 => Some(Pc1500Key::F2), // Removido - no existe en el layout real  
            // Key::F3 => Some(Pc1500Key::F3), // Removido - no existe en el layout real
            // Key::F4 => Some(Pc1500Key::F4), // Removido - no existe en el layout real
            Key::F5 => Some(Pc1500Key::F5),
            Key::F6 => Some(Pc1500Key::F6),
            
            // Operators and symbols  
            Key::Minus => Some(Pc1500Key::Minus),        // - key  
            Key::Equals => Some(Pc1500Key::Equals),      // = key
            // Note: Plus key mapping varies by keyboard layout
            
            // Arrow keys
            Key::ArrowLeft => Some(Pc1500Key::Left),
            Key::ArrowRight => Some(Pc1500Key::Right),
            Key::ArrowUp => Some(Pc1500Key::Up),
            Key::ArrowDown => Some(Pc1500Key::Down),
            
            _ => None, // No mapping for this key
        }
    }
    
    /// Handle a key press event
    pub fn handle_key_pressed(&mut self, key: Key) -> Option<Pc1500Key> {
        if let Some(pc1500_key) = self.map_physical_to_pc1500(key) {
            self.pressed_keys.insert(pc1500_key);
            Some(pc1500_key)
        } else {
            None
        }
    }
    
    /// Handle a key release event  
    pub fn handle_key_released(&mut self, key: Key) -> Option<Pc1500Key> {
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
    
    /// Simulate a virtual key press (for virtual keyboard UI)
    pub fn simulate_press(&mut self, key: Pc1500Key) {
        self.pressed_keys.insert(key);
    }
    
    /// Simulate a virtual key release (for virtual keyboard UI)
    pub fn simulate_release(&mut self, key: Pc1500Key) {
        self.pressed_keys.remove(&key);
    }
}
