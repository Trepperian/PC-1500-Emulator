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
    pub fn int(&self) -> bool {
        self.int
    }

    pub fn new_opc(&self) -> bool {
        self.new_opc
    }

    pub fn set_new_opc(&mut self, new_opc: bool) {
        self.new_opc = new_opc;
    }

    // TODO: serial peripherals
    // pub fn set_irq(&mut self, irq: bool) {
    //     self.irq = irq;
    // }

    fn lh5810_pb7(&self) -> bool {
        (self.r_opb & 0x80) != 0
    }

    fn reset_divider(&mut self, timer_state: usize) {
        self.clock_rate_state = timer_state;
    }

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
            Reg::DDA => {
                self.r_dda = data;
                // println!("set_reg DDA: {:02X}", self.r_dda);
            }
            Reg::DDB => self.r_ddb = data,
            Reg::OPA => {
                self.r_opa = (self.r_opa & !self.r_dda) | (data & self.r_dda);
                // println!("set_reg OPA: {:02X}", self.r_opa);
            }
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

    pub fn new() -> Self {
        Self {
            new_g: true,
            new_f: true,
            rol_reg: 0xffff,
            ..Default::default()
        }
    }

    pub fn start_serial_transmit(&mut self, timer_state: usize) {
        self.rol_reg = (0xff00 | self.r_l as u16) << 1;
        self.bit_count = 0;
        self.last_pulse_state = timer_state;
        self.clock_rate_state = timer_state;
    }

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
