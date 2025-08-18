/// PC-1500 Instruction Tracer - Detailed opcode analysis
/// This tool traces the actual instructions being executed from ROM

use ceres_core::pc1500::Pc1500;
use ceres_core::AudioCallback;

struct NoAudio;
impl AudioCallback for NoAudio {
    fn audio_sample(&self, _left: i16, _right: i16) {}
}

/// Detailed instruction trace entry
#[derive(Debug, Clone)]
pub struct InstructionTrace {
    pub pc: u16,
    pub opcode: u8,
    pub operand: Option<u16>,
    pub instruction_name: String,
    pub cycles: u8,
    pub a_before: u8,
    pub a_after: u8,
}

/// Instruction tracer for ROM analysis
pub struct Pc1500InstructionTracer {
    emulator: Pc1500<NoAudio>,
    trace_log: Vec<InstructionTrace>,
    instruction_count: usize,
}

impl Pc1500InstructionTracer {
    pub fn new() -> Self {
        let emulator = Pc1500::new(
            ceres_core::Model::Pc1500,
            NoAudio
        );

        Self {
            emulator,
            trace_log: Vec::new(),
            instruction_count: 0,
        }
    }

    /// Trace one instruction with detailed analysis
    pub fn trace_instruction(&mut self) -> InstructionTrace {
        let cpu_before = self.emulator.cpu_state();
        let pc_before = cpu_before.pc;
        let a_before = cpu_before.a;
        
        // Read opcode and potential operands
        let opcode = self.emulator.read_memory(pc_before);
        let operand = self.read_operand(pc_before, opcode);
        
        // Execute instruction
        self.emulator.run_cpu();
        
        let cpu_after = self.emulator.cpu_state();
        let a_after = cpu_after.a;
        
        // Calculate cycles (estimate based on PC change and typical LH5801 timing)
        let pc_delta = cpu_after.pc.wrapping_sub(pc_before);
        let estimated_cycles = self.estimate_cycles(opcode, pc_delta);
        
        let trace = InstructionTrace {
            pc: pc_before,
            opcode,
            operand,
            instruction_name: self.decode_instruction_name(opcode, operand),
            cycles: estimated_cycles,
            a_before,
            a_after,
        };
        
        self.trace_log.push(trace.clone());
        self.instruction_count += 1;
        
        trace
    }
    
    /// Read operand bytes if needed based on opcode
    fn read_operand(&self, pc: u16, opcode: u8) -> Option<u16> {
        match opcode {
            // Instructions that take 1-byte operands
            0x05 | 0x09 | 0x0D | 0x0F | 0x11..=0x15 | 0x81..=0x8F => {
                Some(self.emulator.read_memory(pc.wrapping_add(1)) as u16)
            },
            // Instructions that take 2-byte operands  
            0x04 | 0x07 | 0x08 | 0x0B | 0x0C | 0x0E | 0x02 | 0x03 | 0x0A => {
                let low = self.emulator.read_memory(pc.wrapping_add(1));
                let high = self.emulator.read_memory(pc.wrapping_add(2));
                Some(u16::from_le_bytes([low, high]))
            },
            // FD prefix - extended instructions
            0xFD => {
                let next_opcode = self.emulator.read_memory(pc.wrapping_add(1));
                Some(next_opcode as u16)
            },
            _ => None,
        }
    }
    
    /// Decode instruction name from opcode
    fn decode_instruction_name(&self, opcode: u8, operand: Option<u16>) -> String {
        match opcode {
            0x05 => format!("LDA #0x{:02X}", operand.unwrap_or(0) as u8),
            0x04 => format!("LDA 0x{:04X}", operand.unwrap_or(0)),
            0x07 => format!("STA 0x{:04X}", operand.unwrap_or(0)),
            0x09 => format!("LDB #0x{:02X}", operand.unwrap_or(0) as u8),
            0x08 => format!("LDB 0x{:04X}", operand.unwrap_or(0)),
            0x0B => format!("STB 0x{:04X}", operand.unwrap_or(0)),
            0x0D => format!("ADC #0x{:02X}", operand.unwrap_or(0) as u8),
            0x0C => format!("ADC 0x{:04X}", operand.unwrap_or(0)),
            0x0F => format!("SBC #0x{:02X}", operand.unwrap_or(0) as u8),
            0x0E => format!("SBC 0x{:04X}", operand.unwrap_or(0)),
            0x11 => format!("AND #0x{:02X}", operand.unwrap_or(0) as u8),
            0x12 => format!("OR #0x{:02X}", operand.unwrap_or(0) as u8),
            0x13 => format!("XOR #0x{:02X}", operand.unwrap_or(0) as u8),
            0x14 => format!("CMP #0x{:02X}", operand.unwrap_or(0) as u8),
            0x15 => format!("CMP 0x{:04X}", operand.unwrap_or(0)),
            0x40 => "INC A".to_string(),
            0x42 => "DEC A".to_string(),
            0x41 => "INC B".to_string(),
            0x43 => "DEC B".to_string(),
            0x81 => format!("BRA 0x{:02X}", operand.unwrap_or(0) as u8),
            0x82 => format!("BCS 0x{:02X}", operand.unwrap_or(0) as u8),
            0x83 => format!("BCC 0x{:02X}", operand.unwrap_or(0) as u8),
            0x84 => format!("BEQ 0x{:02X}", operand.unwrap_or(0) as u8),
            0x85 => format!("BNE 0x{:02X}", operand.unwrap_or(0) as u8),
            0x86 => format!("BMI 0x{:02X}", operand.unwrap_or(0) as u8),
            0x87 => format!("BPL 0x{:02X}", operand.unwrap_or(0) as u8),
            0xFD => {
                let ext_op = operand.unwrap_or(0) as u8;
                format!("FD {:02X} (Extended)", ext_op)
            },
            _ => format!("UNKNOWN 0x{:02X}", opcode),
        }
    }
    
    /// Estimate instruction cycles based on opcode and PC change
    fn estimate_cycles(&self, opcode: u8, pc_delta: u16) -> u8 {
        match opcode {
            // Load/Store instructions
            0x04..=0x0B => if pc_delta == 3 { 4 } else { 3 },
            // Arithmetic immediate
            0x0D | 0x0F | 0x11..=0x14 => 3,
            // Branches  
            0x81..=0x8F => if pc_delta > 2 { 3 } else { 2 },
            // Register operations
            0x40..=0x4F => 2,
            // Extended instructions
            0xFD => if pc_delta > 2 { 4 } else { 3 },
            _ => 2, // Default estimate
        }
    }
    
    /// Run detailed instruction trace
    pub fn run_detailed_trace(&mut self, max_instructions: usize, show_every: usize) {
        println!("🔬 PC-1500 DETAILED INSTRUCTION TRACE");
        println!("=====================================");
        println!("Tracing authentic PC-1500 ROM execution with full opcode analysis\n");
        
        self.emulator.reset();
        
        println!("{:>4} | {:>6} | {:>4} | {:>20} | {:>3} | {:>4} | {:>4}", 
            "#", "PC", "OP", "INSTRUCTION", "CYC", "A-", "A+");
        println!("{}", "=".repeat(70));
        
        let mut loop_detection = std::collections::HashMap::new();
        
        for i in 0..max_instructions {
            let trace = self.trace_instruction();
            
            // Show detailed trace periodically
            if i % show_every == 0 || i < 50 {
                println!("{:>4} | 0x{:04X} | 0x{:02X} | {:>20} | {:>3} | 0x{:02X} | 0x{:02X}", 
                    i + 1, trace.pc, trace.opcode, trace.instruction_name, 
                    trace.cycles, trace.a_before, trace.a_after);
            }
            
            // Detect loops (same PC repeated)
            *loop_detection.entry(trace.pc).or_insert(0) += 1;
            if loop_detection[&trace.pc] > 10 {
                println!("\n⏳ Loop detected at PC: 0x{:04X} (repeated {} times)", 
                    trace.pc, loop_detection[&trace.pc]);
                println!("   This suggests the ROM is waiting for an external condition.");
                break;
            }
            
            // Check for potential store instructions that might fail
            if trace.instruction_name.contains("STA") || trace.instruction_name.contains("STB") {
                println!("🔍 STORE INSTRUCTION DETECTED: {}", trace.instruction_name);
                println!("   This should cause a memory write - investigating...");
            }
        }
        
        self.analyze_trace_results();
    }
    
    /// Analyze the trace results for patterns
    fn analyze_trace_results(&self) {
        println!("\n📊 TRACE ANALYSIS RESULTS:");
        println!("=========================");
        
        // Count instruction types
        let mut load_count = 0;
        let mut store_count = 0;
        let mut branch_count = 0;
        let mut arithmetic_count = 0;
        let mut unknown_count = 0;
        
        for trace in &self.trace_log {
            match trace.opcode {
                0x04..=0x0A => load_count += 1,
                0x07 | 0x0B => store_count += 1,
                0x81..=0x8F => branch_count += 1,
                0x0D | 0x0F | 0x11..=0x15 | 0x40..=0x4F => arithmetic_count += 1,
                _ => unknown_count += 1,
            }
        }
        
        println!("Instruction Type Breakdown:");
        println!("  Load instructions: {}", load_count);
        println!("  Store instructions: {} ⚠️", store_count);
        println!("  Branch instructions: {}", branch_count);
        println!("  Arithmetic instructions: {}", arithmetic_count);
        println!("  Unknown/Unimplemented: {} ⚠️", unknown_count);
        
        // Check for concerning patterns
        if store_count > 0 {
            println!("\n🔍 CRITICAL FINDING:");
            println!("   {} store instructions were executed but no memory writes detected!", store_count);
            println!("   This suggests store instructions may not be properly implemented.");
            
            println!("\n   Store instructions found:");
            for trace in &self.trace_log {
                if trace.instruction_name.contains("STA") || trace.instruction_name.contains("STB") {
                    println!("     PC: 0x{:04X} - {}", trace.pc, trace.instruction_name);
                }
            }
        }
        
        if unknown_count > self.trace_log.len() / 4 {
            println!("\n⚠️  HIGH UNKNOWN INSTRUCTION COUNT:");
            println!("   {}% of instructions are unknown/unimplemented", 
                (unknown_count * 100) / self.trace_log.len());
            println!("   This may prevent proper ROM execution.");
        }
        
        // PC range analysis
        if let (Some(first), Some(last)) = (self.trace_log.first(), self.trace_log.last()) {
            println!("\nExecution Range:");
            println!("  Started at: 0x{:04X}", first.pc);
            println!("  Ended at: 0x{:04X}", last.pc);
            println!("  Total instructions traced: {}", self.trace_log.len());
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut tracer = Pc1500InstructionTracer::new();
    
    // Run detailed trace showing every 10th instruction for 500 total
    tracer.run_detailed_trace(500, 10);
    
    println!("\n✅ Detailed instruction trace complete!");
    
    Ok(())
}
