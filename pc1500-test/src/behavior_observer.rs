/// Enhanced PC-1500 ROM Behavior Observer
/// This tool extends our ROM test to specifically observe calculator behavior

use ceres_core::pc1500::Pc1500;
use ceres_core::AudioCallback;

struct NoAudio;
impl AudioCallback for NoAudio {
    fn audio_sample(&self, _left: i16, _right: i16) {}
}

/// Memory write tracker for observing ROM behavior
#[derive(Debug, Clone)]
pub struct MemoryWrite {
    pub address: u16,
    pub value: u8,
    pub instruction_count: usize,
}

/// Enhanced behavior observer
pub struct Pc1500BehaviorObserver {
    emulator: Pc1500<NoAudio>,
    memory_writes: Vec<MemoryWrite>,
    display_writes: Vec<MemoryWrite>, 
    instruction_count: usize,
    previous_memory_state: Vec<u8>,
}

impl Pc1500BehaviorObserver {
    pub fn new() -> Self {
        let emulator = Pc1500::new(
            ceres_core::Model::Pc1500,
            NoAudio
        );

        // Initialize previous memory state snapshot
        let mut previous_memory_state = vec![0u8; 65536];
        for addr in 0..=65535u16 {
            previous_memory_state[addr as usize] = emulator.read_memory(addr);
        }

        Self {
            emulator,
            memory_writes: Vec::new(),
            display_writes: Vec::new(),
            instruction_count: 0,
            previous_memory_state,
        }
    }

    /// Execute one instruction and track all memory changes
    pub fn step_and_observe(&mut self) {
        // Store state before execution
        let _pc_before = self.emulator.cpu_state().pc;
        
        // Execute one instruction
        self.emulator.run_cpu();
        self.instruction_count += 1;
        
        // Check for memory changes
        self.detect_memory_changes();
        
        // Check for display activity specifically
        self.check_display_changes();
    }

    /// Detect any memory writes that occurred
    fn detect_memory_changes(&mut self) {
        // Check all memory addresses for changes
        for addr in 0..=65535u16 {
            let current_value = self.emulator.read_memory(addr);
            let previous_value = self.previous_memory_state[addr as usize];
            
            if current_value != previous_value {
                let write = MemoryWrite {
                    address: addr,
                    value: current_value,
                    instruction_count: self.instruction_count,
                };
                
                // Track all writes
                self.memory_writes.push(write.clone());
                
                // Track display writes specifically
                if (addr >= 0x7600 && addr <= 0x764F) || (addr >= 0x7700 && addr <= 0x774F) {
                    self.display_writes.push(write);
                    println!("🖥️  DISPLAY WRITE detected! Addr: 0x{:04X}, Value: 0x{:02X}, Instruction: #{}", 
                        addr, current_value, self.instruction_count);
                }
                
                // Update our memory snapshot
                self.previous_memory_state[addr as usize] = current_value;
            }
        }
    }
    
    /// Check for display-specific changes
    fn check_display_changes(&self) {
        // Check if display memory contains non-zero data
        let mut display_data = Vec::new();
        
        // Display Bank 0 (0x7600-0x764F)
        for addr in 0x7600..=0x764F {
            let value = self.emulator.read_memory(addr);
            if value != 0 {
                display_data.push((addr, value));
            }
        }
        
        // Display Bank 1 (0x7700-0x774F) 
        for addr in 0x7700..=0x774F {
            let value = self.emulator.read_memory(addr);
            if value != 0 {
                display_data.push((addr, value));
            }
        }
        
        if !display_data.is_empty() && self.instruction_count % 50 == 0 {
            println!("📺 Display contains {} non-zero bytes at instruction #{}", 
                display_data.len(), self.instruction_count);
        }
    }

    /// Run extended observation to catch calculator initialization
    pub fn observe_calculator_startup(&mut self, max_instructions: usize) {
        println!("🔍 OBSERVING PC-1500 CALCULATOR STARTUP BEHAVIOR");
        println!("================================================");
        println!("Tracking memory writes, display changes, and system behavior...\n");

        let mut last_pc = 0;
        let mut pc_same_count = 0;

        for i in 0..max_instructions {
            let cpu_state = self.emulator.cpu_state();
            
            // Check for PC loops (system waiting)
            if cpu_state.pc == last_pc {
                pc_same_count += 1;
                if pc_same_count == 10 {
                    println!("⏳ System appears to be in waiting loop at PC: 0x{:04X}", cpu_state.pc);
                    println!("   This likely indicates the calculator is waiting for keyboard input!");
                    break;
                }
            } else {
                pc_same_count = 0;
                last_pc = cpu_state.pc;
            }

            // Execute and observe
            self.step_and_observe();
            
            // Report progress periodically
            if i % 100 == 0 && i > 0 {
                self.report_progress(i);
            }
            
            // Check if we've detected calculator interface
            if !self.display_writes.is_empty() {
                println!("\n🎉 CALCULATOR INTERFACE DETECTED!");
                self.analyze_display_content();
                break;
            }
        }
        
        self.final_analysis();
    }
    
    /// Report current progress
    fn report_progress(&self, instructions: usize) {
        let cpu_state = self.emulator.cpu_state();
        println!("📊 Progress Report - Instruction #{}", instructions);
        println!("   PC: 0x{:04X}, A: 0x{:02X}, Memory Writes: {}, Display Writes: {}",
            cpu_state.pc, cpu_state.a, self.memory_writes.len(), self.display_writes.len());
    }
    
    /// Analyze display content when found
    fn analyze_display_content(&self) {
        println!("\n🖥️  DISPLAY CONTENT ANALYSIS:");
        println!("   Total display writes: {}", self.display_writes.len());
        
        for write in &self.display_writes {
            let row = if write.address >= 0x7700 { 1 } else { 0 };
            let col = (write.address - if row == 0 { 0x7600 } else { 0x7700 }) as u8;
            println!("   Bank {}, Pos {}: 0x{:02X} (Instruction #{})",
                row, col, write.value, write.instruction_count);
        }
    }
    
    /// Final behavior analysis
    fn final_analysis(&self) {
        println!("\n🎯 FINAL BEHAVIOR ANALYSIS:");
        println!("=============================");
        
        let cpu_state = self.emulator.cpu_state();
        println!("📊 Execution Statistics:");
        println!("   Instructions executed: {}", self.instruction_count);
        println!("   Final PC: 0x{:04X}", cpu_state.pc);
        println!("   Final A register: 0x{:02X}", cpu_state.a);
        println!("   Total memory writes: {}", self.memory_writes.len());
        println!("   Display writes: {}", self.display_writes.len());
        
        // Analyze memory write patterns
        if !self.memory_writes.is_empty() {
            println!("\n💾 Memory Write Analysis:");
            let mut ram_writes = 0;
            let mut display_writes = 0;
            let mut io_writes = 0;
            
            for write in &self.memory_writes {
                match write.address {
                    0x8000..=0x9FFF => ram_writes += 1,
                    0x7600..=0x764F | 0x7700..=0x774F => display_writes += 1,
                    0xFC00..=0xFFFF => io_writes += 1,
                    _ => {}
                }
            }
            
            println!("   RAM writes (0x8000-0x9FFF): {}", ram_writes);
            println!("   Display writes (0x7600-0x774F): {}", display_writes);
            println!("   I/O writes (0xFC00-0xFFFF): {}", io_writes);
        }
        
        // Assessment
        println!("\n🔮 CALCULATOR BEHAVIOR ASSESSMENT:");
        if !self.display_writes.is_empty() {
            println!("✅ SUCCESS: Calculator interface detected!");
            println!("   The ROM has initialized the display and is showing the calculator interface.");
            println!("   The emulator is successfully running authentic PC-1500 calculator firmware.");
        } else if self.memory_writes.len() > 10 {
            println!("🔄 PROGRESS: System is active but display not yet initialized");
            println!("   The ROM is executing and writing to memory, but hasn't reached display setup yet.");
            println!("   Try running for more instructions or check display initialization sequence.");
        } else {
            println!("⚠️  LIMITED ACTIVITY: ROM executing but minimal memory activity detected");
            println!("   The ROM is running but may need keyboard input or more time to initialize.");
        }
        
        println!("\n💡 Next steps:");
        println!("   1. If display detected: Implement visual rendering");
        println!("   2. If waiting loop: Implement keyboard input");
        println!("   3. If limited activity: Run for more instructions");
    }
}

fn main() -> anyhow::Result<()> {
    println!("🔬 PC-1500 CALCULATOR BEHAVIOR OBSERVATION");
    println!("=========================================\n");
    
    let mut observer = Pc1500BehaviorObserver::new();
    
    // Reset system to ensure clean start
    observer.emulator.reset();
    
    // Observe calculator startup behavior
    observer.observe_calculator_startup(5000); // Up to 5000 instructions
    
    println!("\n✅ Behavior observation complete!");
    
    Ok(())
}
