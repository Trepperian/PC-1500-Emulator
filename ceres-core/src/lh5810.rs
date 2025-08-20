// IO Controller

use crate::pd1990ac::FREQUENCY;

pub enum Reg {
    RESET,
    U,
    L,
    G,
    MSK,
    IF,
    DDA,
    DDB,
    OPA,
    OPB,
    OPC,
    F,
}

#[derive(Default)]
pub struct Lh5810 {
    // Registers
    reset: u8,
    r_g: u8,
    r_u: u8,
    r_l: u8,
    r_msk: u8,
    r_if: u8,
    r_dda: u8,
    r_ddb: u8,
    r_opa: u8,
    r_opb: u8,
    r_opc: u8,
    r_f: u8,

    // Signals
    irq: bool,
    int: bool,
    fx: i32,
    fy: i32,
    sdo: bool,
    sdi: bool,
    cli: bool,
    clo: bool,

    new_l: bool,
    new_g: bool,
    new_f: bool,
    new_opc: bool,

    rol_reg: u16,
    bit_count: u8,
    bit: bool,
    modulation_send: bool,
    last_pulse_state: usize,
    clock_rate_state: usize,
    clock_rate: usize,
    clock_rate_wait: usize,
    clock_output: bool,
}

impl Lh5810 {
    pub fn new_opc(&self) -> bool {
        self.new_opc
    }

    pub fn set_new_opc(&mut self, new_opc: bool) {
        self.new_opc = new_opc;
    }

    pub fn set_irq(&mut self, irq: bool) {
        self.irq = irq;
    }

    // #define LH5810_PB7 ((lh5810.r_opb & 0x80) ? true : false)
    fn lh5810_pb7(&self) -> bool {
        (self.r_opb & 0x80) != 0
    }

    // void CLH5810::ResetDivider()
    // {
    //    clockRateState=pPC->pTIMER->state;
    // }
    fn reset_divider(&mut self, timer_state: usize) {
        self.clock_rate_state = timer_state;
    }

    //  UINT8 GetReg(LH5810_REGS reg)
    // {
    //     UINT8 t = 0;

    //     switch (reg)
    //     {
    //     case U:
    //         return (lh5810.r_u);
    //     case L:
    //         return (lh5810.r_l);
    //     case G:
    //         return (lh5810.r_g);
    //     case MSK:
    //         t = (lh5810.r_msk);
    //         if (IRQ)
    //             t |= 0x10;
    //         if (LH5810_PB7)
    //             t |= 0x20;
    //         if (SDI)
    //             t |= 0x40;
    //         if (CLI)
    //             t |= 0x80;
    //         return (t);
    //     case IF:
    //         t = (lh5810.r_if);
    //         //						if (IRQ)	t|=0x01;
    //         //						if (PB7)	t|=0x02;
    //         //                    qWarning()<<"return IF="<<t;
    //         return (t);
    //     case DDA:
    //         return (lh5810.r_dda);
    //     case DDB:
    //         return (lh5810.r_ddb);
    //     case OPA:
    //         return (lh5810.r_opa & (~lh5810.r_dda)); // OK
    //     case OPB:
    //         return (lh5810.r_opb & (~lh5810.r_ddb)); // OK
    //     case OPC:
    //         return (lh5810.r_opc);
    //     case F:
    //         return (lh5810.r_f);
    //     default:
    //         return (0);
    //     }
    //     return (0);
    // }
    pub fn get_reg(&self, reg: Reg) -> u8 {
        match reg {
            Reg::RESET => self.reset,
            Reg::U => self.r_u,
            Reg::L => self.r_l,
            Reg::G => self.r_g,
            Reg::MSK => {
                let mut t = self.r_msk;
                if self.irq {
                    t |= 0x10;
                }
                if self.lh5810_pb7() {
                    t |= 0x20;
                }
                if self.sdi {
                    t |= 0x40;
                }
                if self.cli {
                    t |= 0x80;
                }
                t
            }
            Reg::IF => self.r_if,
            Reg::DDA => self.r_dda,
            Reg::DDB => self.r_ddb,
            Reg::OPA => self.r_opa & !self.r_dda,
            Reg::OPB => self.r_opb & !self.r_ddb,
            Reg::OPC => self.r_opc,
            Reg::F => self.r_f,
        }
    }

    // UINT8 SetReg(LH5810_REGS reg, UINT8 data)
    // {
    //     switch (reg)
    //     {
    //     case RESET:
    //         ResetDivider();
    //         return (lh5810.reset = data);
    //         break;
    //     case U:
    //         return (lh5810.r_u = data);
    //         break;
    //     case L:
    //         New_L = true;
    //         return (lh5810.r_l = data);
    //         break;
    //     case G:
    //         New_G = true;
    //         return (lh5810.r_g = data);
    //         break;
    //     case MSK:
    //         return (lh5810.r_msk = data & 0x0F);
    //         break;
    //     case IF:
    //         return (lh5810.r_if = ((lh5810.r_if & 0xFC) | (data & 0x03)));
    //         break;
    //     case DDA:
    //         return (lh5810.r_dda = data);
    //         break;
    //     case DDB:
    //         return (lh5810.r_ddb = data);
    //         break;
    //     case OPA:
    //         return (lh5810.r_opa = ((lh5810.r_opa & (~lh5810.r_dda)) | (data & (lh5810.r_dda))));
    //         break;
    //     case OPB:
    //         return (lh5810.r_opb = ((lh5810.r_opb & (~lh5810.r_ddb)) | (data & (lh5810.r_ddb))));
    //         break;
    //     case OPC:
    //         New_OPC = true;
    //         return (lh5810.r_opc = data);
    //         break;
    //     case F:
    //         New_F = true;
    //         return (lh5810.r_f = data);
    //         break;
    //     }

    //     return (0);
    // }

    pub fn set_reg(&mut self, reg: Reg, data: u8, timer_state: usize) {
        match reg {
            Reg::RESET => {
                self.reset_divider(timer_state);
                self.reset = data;
            }
            Reg::U => self.r_u = data,
            Reg::L => {
                self.new_l = true;
                self.r_l = data;
            }
            Reg::G => {
                self.new_g = true;
                self.r_g = data;
            }
            Reg::MSK => self.r_msk = data & 0x0F,
            Reg::IF => self.r_if = (self.r_if & 0xFC) | (data & 0x03),
            Reg::DDA => self.r_dda = data,
            Reg::DDB => self.r_ddb = data,
            Reg::OPA => self.r_opa = (self.r_opa & !self.r_dda) | (data & self.r_dda),
            Reg::OPB => self.r_opb = (self.r_opb & !self.r_ddb) | (data & self.r_ddb),
            Reg::OPC => {
                self.new_opc = true;
                self.r_opc = data;
            }
            Reg::F => {
                self.new_f = true;
                self.r_f = data;
            }
        }
    }

    //  UINT8 SetRegBit(LH5810_REGS reg, UINT8 bit, bool value)
    //     {
    //         if (value)
    //         {
    //             switch (reg)
    //             {
    //             case U:
    //                 return (lh5810.r_u |= (0x01 << bit));
    //                 break;
    //             case L:
    //                 return (lh5810.r_l |= (0x01 << bit));
    //                 break;
    //             case G:
    //                 return (lh5810.r_g |= (0x01 << bit));
    //                 break;
    //             case MSK:
    //                 return (lh5810.r_msk |= (0x01 << bit));
    //                 break;
    //             case IF:
    //                 return (lh5810.r_if |= (0x01 << bit));
    //                 break;
    //             case DDA:
    //                 return (lh5810.r_dda |= (0x01 << bit));
    //                 break;
    //             case DDB:
    //                 return (lh5810.r_ddb |= (0x01 << bit));
    //                 break;
    //             case OPA:
    //                 return (lh5810.r_opa |= (0x01 << bit));
    //                 break;
    //             case OPB:
    //                 return (lh5810.r_opb |= (0x01 << bit));
    //                 break;
    //             case OPC:
    //                 New_OPC = true;
    //                 return (lh5810.r_opc |= (0x01 << bit));
    //                 break;
    //             case F:
    //                 return (lh5810.r_f |= (0x01 << bit));
    //                 break;
    //             default:
    //                 break;
    //             }
    //         }
    //         else
    //         {
    //             switch (reg)
    //             {
    //             case U:
    //                 return (lh5810.r_u &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case L:
    //                 return (lh5810.r_l &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case G:
    //                 return (lh5810.r_g &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case MSK:
    //                 return (lh5810.r_msk &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case IF:
    //                 return (lh5810.r_if &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case DDA:
    //                 return (lh5810.r_dda &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case DDB:
    //                 return (lh5810.r_ddb &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case OPA:
    //                 return (lh5810.r_opa &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case OPB:
    //                 return (lh5810.r_opb &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case OPC:
    //                 return (lh5810.r_opc &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             case F:
    //                 return (lh5810.r_f &= ((0x01 << bit) ^ 0xff));
    //                 break;
    //             default:
    //                 break;
    //             }
    //         }
    //         return (0);
    //     }
    pub fn set_reg_bit(&mut self, reg: Reg, bit: u8, value: bool) {
        if value {
            match reg {
                Reg::U => self.r_u |= 0x01 << bit,
                Reg::L => self.r_l |= 0x01 << bit,
                Reg::G => self.r_g |= 0x01 << bit,
                Reg::MSK => self.r_msk |= 0x01 << bit,
                Reg::IF => self.r_if |= 0x01 << bit,
                Reg::DDA => self.r_dda |= 0x01 << bit,
                Reg::DDB => self.r_ddb |= 0x01 << bit,
                Reg::OPA => self.r_opa |= 0x01 << bit,
                Reg::OPB => self.r_opb |= 0x01 << bit,
                Reg::OPC => {
                    self.new_opc = true;
                    self.r_opc |= 0x01 << bit;
                }
                Reg::F => self.r_f |= 0x01 << bit,
                _ => {} // No action for other registers
            }
        } else {
            match reg {
                Reg::U => self.r_u &= !(0x01 << bit),
                Reg::L => self.r_l &= !(0x01 << bit),
                Reg::G => self.r_g &= !(0x01 << bit),
                Reg::MSK => self.r_msk &= !(0x01 << bit),
                Reg::IF => self.r_if &= !(0x01 << bit),
                Reg::DDA => self.r_dda &= !(0x01 << bit),
                Reg::DDB => self.r_ddb &= !(0x01 << bit),
                Reg::OPA => self.r_opa &= !(0x01 << bit),
                Reg::OPB => self.r_opb &= !(0x01 << bit),
                Reg::OPC => self.r_opc &= !(0x01 << bit),
                Reg::F => self.r_f &= !(0x01 << bit),
                _ => {} // No action for other registers
            }
        }
    }

    //     CLH5810::CLH5810(CPObject *parent) : CPObject(parent) //[constructor]
    // {
    //     lh5810.r_g = lh5810.r_msk = lh5810.r_dda = lh5810.r_ddb = lh5810.r_opa = lh5810.r_opb = lh5810.r_opc = lh5810.r_f = 0;
    //     lh5810.r_if = 0;
    //     IRQ = INT = false;
    //     SDO = SDI = CLI = false;
    //     modulationSend = false;
    //     New_L = false;
    //     New_G = New_F = true;
    //     RolReg = 0xffff;
    //     clockOutput = false;
    //     //	OPA=OPB=0;
    // }
    pub fn new() -> Self {
        Self {
            new_g: true,
            new_f: true,
            rol_reg: 0xffff,
            ..Default::default()
        }
    }

    // void CLH5810::start_serial_transmit()
    // {
    //     RolReg = (0xff00 | lh5810.r_l) << 1;
    //     //    bit = 0;
    //     //    serialSend= true;
    //     bitCount = 0;
    //     lastPulseState = clockRateState = pPC->pTIMER->state;
    //     //    qWarning()<<"New_L:"<<QString("%1").arg(lh5810.r_l,2,16,QChar('0'));
    // }
    pub fn start_serial_transmit(&mut self, timer_state: usize) {
        self.rol_reg = (0xff00 | self.r_l as u16) << 1;
        self.bit_count = 0;
        self.last_pulse_state = timer_state;
        self.clock_rate_state = timer_state;
    }

    // bool CLH5810::step()
    // {
    //     INT = false;

    //     if (New_F)
    //     {
    //         FX = lh5810.r_f & 0x07;
    //         FY = (lh5810.r_f >> 3) & 0x07;
    //         qWarning() << "Fx=" << FX << "   FY=" << FY << "  val=" << lh5810.r_f;
    //         New_F = false;

    //         modulationSend = (lh5810.r_f & 0x40);
    //         bit = false;
    //     }

    //     if (modulationSend)
    //     {
    //         //        SetRegBit(IF,3,false);
    //         bit = RolReg & 0x01;

    //         quint64 waitState = (bit ? (0x40 << FX) : (0x40 << FY)) / 2;
    //         if ((pPC->pTIMER->state - lastPulseState) >= waitState)
    //         {
    //             SDO = !SDO;
    //             //            qWarning()<<"flip="<<(pPC->pTIMER->state - lastPulseState);
    //             while ((pPC->pTIMER->state - lastPulseState) >= waitState)
    //                 lastPulseState += waitState;
    //         }
    //     }

    //     if (clockOutput)
    //     {
    //         if ((pPC->pTIMER->state - clockRateState) >= (clockRate / 1))
    //         {
    //             CLO = true;
    //             bitCount++;
    //             if (bitCount == 9)
    //                 SetRegBit(IF, 3, true);

    //             RolReg >>= 1;
    //             RolReg |= 0x8000;

    //             //            qWarning()<<"bit:"<<(RolReg & 0x01)<<"  delta="<<pPC->pTIMER->state - clockRateState;
    //             while ((pPC->pTIMER->state - clockRateState) >= (clockRate / 1))
    //                 clockRateState += clockRate / 1;
    //         }
    //         if (CLO &&
    //             ((pPC->pTIMER->state - clockRateState) > (clockRate / 10)))
    //         {
    //             CLO = false;
    //         }
    //     }

    //     if (New_G)
    //     {
    //         switch (lh5810.r_g & 0x07)
    //         {
    //         case 0x00:
    //             clockRate = 1;
    //             break;
    //         case 0x01:
    //             clockRate = 2;
    //             break;
    //         case 0x02:
    //             clockRate = 128;
    //             break;
    //         case 0x03:
    //             clockRate = 256;
    //             break;
    //         case 0x04:
    //             clockRate = 512;
    //             break;
    //         case 0x05:
    //             clockRate = 1024;
    //             break;
    //         case 0x06:
    //             clockRate = 2048;
    //             break;
    //         case 0x07:
    //             clockRate = 4096;
    //             break;
    //         }
    //         clockRateWait = pPC->getfrequency() / clockRate;
    //         //        qWarning()<<"G= "<<lh5810.r_g<<"   ClockRate set to :"<<clockRateWait;

    //         clockOutput = lh5810.r_g & 0X10;
    //         New_G = false;
    //     }
    //     // If the L register change, then TD flag of the IF register down
    //     if (New_L)
    //     {
    //         // AddLog(LOG_TAPE,tr("L register change -> %1X").arg(lh5810.r_l,4,16,QChar('0')));
    //         SetRegBit(IF, 3, false);
    //         New_L = false;
    //         if (modulationSend)
    //             qWarning() << "Serial transmission in progress!!!";
    //         start_serial_transmit();
    //     }
    //     if (IRQ)
    //         lh5810.r_if |= 0x01;
    //     if (LH5810_PB7)
    //         lh5810.r_if |= 0x02;

    //     if (
    //         ((lh5810.r_msk & 0x01) && IRQ) ||
    //         ((lh5810.r_msk & 0x02) && LH5810_PB7))
    //     {
    //         INT = true;
    //         //        if (pPC->pCPU->fp_log) fprintf(pPC->pCPU->fp_log,"INT\n");
    //     }

    //     return (1);
    // }
    pub fn step(&mut self, timer_state: usize) {
        self.int = false;

        if self.new_f {
            self.fx = (self.r_f & 0x07) as i32;
            self.fy = ((self.r_f >> 3) & 0x07) as i32;
            self.new_f = false;

            self.modulation_send = (self.r_f & 0x40) != 0;
            self.bit = false;
        }

        if self.modulation_send {
            self.bit = (self.rol_reg & 0x01) != 0;

            let wait_state = if self.bit {
                0x40 << self.fx
            } else {
                0x40 << self.fy
            } / 2;

            if (timer_state - self.last_pulse_state) >= wait_state as usize {
                self.sdo = !self.sdo;
                while (timer_state - self.last_pulse_state) >= wait_state as usize {
                    self.last_pulse_state += wait_state as usize;
                }
            }
        }

        if self.clock_output {
            if (timer_state - self.clock_rate_state) >= (self.clock_rate / 1) as usize {
                self.clo = true;
                self.bit_count = self.bit_count.wrapping_add(1);
                if self.bit_count == 9 {
                    self.set_reg_bit(Reg::IF, 3, true);
                }

                self.rol_reg >>= 1;
                self.rol_reg |= 0x8000;

                while (timer_state - self.clock_rate_state) >= (self.clock_rate / 1) as usize {
                    self.clock_rate_state += self.clock_rate / 1;
                }
            }
            if self.clo && ((timer_state - self.clock_rate_state) > (self.clock_rate / 10) as usize)
            {
                self.clo = false;
            }
        }

        if self.new_g {
            self.clock_rate = match self.r_g & 0x07 {
                0x00 => 1,
                0x01 => 2,
                0x02 => 128,
                0x03 => 256,
                0x04 => 512,
                0x05 => 1024,
                0x06 => 2048,
                0x07 => 4096,
                _ => unreachable!(),
            };
            self.clock_rate_wait = FREQUENCY / self.clock_rate;
            self.clock_output = (self.r_g & 0x10) != 0;
            self.new_g = false;
        }

        // If the L register change, then TD flag of the IF register down
        if self.new_l {
            self.set_reg_bit(Reg::IF, 3, false);
            self.new_l = false;
            self.start_serial_transmit(timer_state);
        }

        if self.irq {
            self.r_if |= 0x01;
        }

        if self.lh5810_pb7() {
            self.r_if |= 0x02;
        }

        if ((self.r_msk & 0x01 != 0) && self.irq) || ((self.r_msk & 0x02 != 0) && self.lh5810_pb7())
        {
            self.int = true;
        }
    }
}
