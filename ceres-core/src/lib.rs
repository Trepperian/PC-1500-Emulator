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
const TICKS_PER_FRAME: usize = 12000;

pub struct Pc1500 {
    lh5801: Lh5801,
    lh5810: Lh5810,
    pd1990ac: Pd1990ac,
    memory: MemoryBus,
    joypad: Keyboard,
    display: DisplayController,
}

impl Pc1500 {
    #[must_use]
    pub fn new() -> Self {
        Self {
            lh5801: Lh5801::new(),
            memory: MemoryBus::new(),
            joypad: Keyboard::new(),
            display: DisplayController::new(),
            lh5810: Lh5810::new(),
            pd1990ac: Pd1990ac::new(),
        }
    }

    fn run(&mut self) {
        self.step_cpu();
        self.step();
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
        self.joypad.press(key);
    }

    pub fn release(&mut self, key: Key) {
        self.joypad.release(key);
    }

    // bool CLH5810_PC1500::step()
    // {
    //     Cpc15XX *pc1500 = (Cpc15XX *)pPC;

    // 	////////////////////////////////////////////////////////////////////
    // 	//	INT FROM connector to IRQ
    // 	////////////////////////////////////////////////////////////////////
    //     IRQ= ((CbusPc1500*)pc1500->bus)->getINT();

    // 	////////////////////////////////////////////////////////////////////
    // 	//	Send Data to PD1990AC -- TIMER
    // 	////////////////////////////////////////////////////////////////////

    //     if (New_OPC) {
    //         UINT8 t = lh5810.r_opc;
    //         pPD1990AC->Set_data(READ_BIT(t,0));		// PC0
    //         pPD1990AC->Set_stb(READ_BIT(t,1));		// PC1
    //         pPD1990AC->Set_clk(READ_BIT(t,2));		// PC2
    //         pPD1990AC->Set_out_enable(READ_BIT(t,3));	// PC3
    //         pPD1990AC->Set_c0(READ_BIT(t,3));			// PC3
    //         pPD1990AC->Set_c1(READ_BIT(t,4));			// PC4
    //         pPD1990AC->Set_c2(READ_BIT(t,5));			// PC5

    //         pPD1990AC->step();
    //         New_OPC = false;
    //     }
    // 	// PB5 = TP
    // 	// PB6 = DATA
    //     SetRegBit(OPB,5,pPD1990AC->Get_tp());
    //     SetRegBit(OPB,6,pPD1990AC->Get_data());

    // 	////////////////////////////////////////////////////////////////////
    // 	//	ON/Break
    // 	////////////////////////////////////////////////////////////////////
    //     SetRegBit(OPB,7,pPC->pKEYB->Kon);

    // 	////////////////////////////////////////////////////////////////////
    // 	//	TAPE READER
    // 	////////////////////////////////////////////////////////////////////
    //     SetRegBit(OPB,2,((CbusPc1500*)pc1500->bus)->isCMTIN());
    //     CLI = CLO;

    //     SetRegBit(OPB,3,true);	// Export model vs domestic model
    //     SetRegBit(OPB,4,false);	// PB4 to GND

    // 	  //----------------------//
    // 	 // Standard LH5810 STEP //
    // 	//----------------------//
    // 	CLH5810::step();

    // 	return(1);
    // }
    fn step(&mut self) {
        if self.lh5810.new_opc() {
            let t = self.lh5810.get_reg(lh5810::Reg::OPC);
            self.pd1990ac.set_data(t & 0x01 != 0);
            self.pd1990ac.set_stb(t & 0x02 != 0);
            self.pd1990ac.set_clk(t & 0x04 != 0);
            self.pd1990ac.set_out_enable(t & 0x08 != 0);
            self.pd1990ac.set_c0(t & 0x10 != 0);
            self.pd1990ac.set_c1(t & 0x20 != 0);
            self.pd1990ac.set_c2(t & 0x40 != 0);

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
