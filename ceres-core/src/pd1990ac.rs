// #ifndef PD1990AC_H
// #define PD1990AC_H

// #include <QDateTime>
// #include "pobject.h"

// class CPD1990AC:public QObject{

// 	struct pd1990ac_s
// {
// 	int seconds;
// 	int minutes;
// 	int hours;
// 	int days;
// 	int weekday;
// 	int month;
// };

// public:
//     const char*	GetClassName(){ return("CPD1990AC");}

// 	bool	init(void);						//initialize
// 	bool	exit(void);						//end
// 	void	Reset(void);
// 	bool	step(void);

// 	void	increment_day(void);
// 	void	increment_month(void);

// 	void	Load_Internal(FILE *ffile);
// 	void	save_internal(FILE *file);

// 	void	Regs_Info(UINT8 Type);

// 	bool	Get_data(void);
// 	bool	Get_tp(void);

// 	void	Set_c0(bool);
// 	void	Set_c1(bool);
// 	void	Set_c2(bool);
// 	void	Set_stb(bool);
// 	void	Set_cs(bool);
// 	void	Set_data(bool);
// 	void	Set_gnd(bool);
// 	void	Set_clk(bool);
//     void	Set_out_enable(bool);
// 	void	Set_n_xtal(bool);
// 	void	Set_xtal(bool);
// 	void	Set_vdd(bool);

// 	struct	pd1990ac_s pd1990ac;
// 	bool	c0,c1,c2,stb,cs,data_in,gnd,clk,data_out,tp,out_enable,n_xtal,xtal,vdd;

// 	CPD1990AC(CPObject *parent);
//     virtual ~CPD1990AC();

// 	void	addretrace (void);
//     //UINT	TP_FREQUENCY;

// private:

// 	char	Regs_String[255];

// 	UINT	nTimerId;
// 	UINT	nTPTimerId;

// 	UINT8	mode;
// 	UINT	bitno,Current_Bit;
// 	bool	New_Mode;
// 	bool	New_clk;
// 	UINT8	prev_mode;
// 	UINT8	prev_clk;
// 	bool	flip_clk;
//     UINT	TP_FREQUENCY;

// 	QDateTime	lastDateTime;
// 	CPObject *pPC;

//     quint64 previous_state;
//     quint64 previous_state_tp;
// };

// #endif

// Serial IO Calendar and clock

use chrono::{Datelike, Timelike};

pub const FREQUENCY: usize = 2600000 / 2;

pub struct Pd1990ac {
    seconds: u16,
    minutes: u16,
    hours: u16,
    days: u16,
    weekday: u16,
    month: u16,

    // Signals
    c0: bool,
    c1: bool,
    c2: bool,
    stb: bool,
    cs: bool,
    data_in: bool,
    gnd: bool,
    clk: bool,
    data_out: bool,
    tp: bool,
    out_enable: bool,
    n_xtal: bool,
    xtal: bool,
    vdd: bool,

    // Internal
    mode: u8,
    bitno: u8,
    current_bit: u8,
    new_mode: bool,
    new_clk: bool,
    prev_mode: u8,
    prev_clk: bool,
    flip_clk: bool,
    tp_frequency: usize,

    previous_state: usize,
    previous_state_tp: usize,
}

// INLINE WORD HEX2BCD(BYTE d)
// {
// 	BYTE	a,b,c;
// 	a=d/100;
// 	b=d-(a*100);
// 	c=b/10;
// 	return((a<<8)+(c<<4)+b-(c*10));
// }
fn hex2bcd(d: u32) -> u16 {
    let a = d / 100;
    let b = d - (a * 100);
    let c = b / 10;
    ((a as u16) << 8) + ((c as u16) << 4) + (b as u16) - ((c as u16) * 10)
}

// #define READ_BIT(b,p)	( ((b)>>(p)) & 0x01 ? 1 :0 )
// #define SET_BIT(b,p)	((b) |= (0x01<<(p)))
// #define UNSET_BIT(b,p)	((b) &= ~(0x01<<(p)))
// //#define PUT_BIT(b,p,v)	if (v) SET_BIT(b,p); else UNSET_BIT(b,p);
// #define PUT_BIT(b,p,v)	(v) ? SET_BIT(b,p) : UNSET_BIT(b,p);
fn read_bit(value: u16, position: u8) -> bool {
    ((value >> position) & 0x01) != 0
}

fn set_bit(value: &mut u16, position: u8) {
    *value |= 0x01 << position;
}

fn unset_bit(value: &mut u16, position: u8) {
    *value &= !(0x01 << position);
}

fn put_bit(value: &mut u16, position: u8, bit: bool) {
    if bit {
        set_bit(value, position);
    } else {
        unset_bit(value, position);
    }
}

impl Pd1990ac {
    // bool	CPD1990AC::init(void)
    // {

    // 	lastDateTime = QDateTime::currentDateTime();
    // 	pd1990ac.seconds	= HEX2BCD(lastDateTime.time().second());	/* seconds BCD */
    // 	pd1990ac.minutes	= HEX2BCD(lastDateTime.time().minute());	/* minutes BCD */
    // 	pd1990ac.hours		= HEX2BCD(lastDateTime.time().hour());		/* hours   BCD */
    // 	pd1990ac.days		= HEX2BCD(lastDateTime.date().day());		/* days    BCD */
    // 	pd1990ac.weekday	= 0;										/* weekday BCD */
    // 	pd1990ac.month		= lastDateTime.date().month();				/* month   Hexadecimal form */
    // 	bitno = 0;
    // 	prev_mode = 0x10;
    // 	tp = 0;
    //     TP_FREQUENCY=1;
    // 	previous_state = 0;
    // 	previous_state_tp = 0;

    // 	return(1);
    // }
    pub fn new() -> Self {
        let now = std::time::SystemTime::now();
        let datetime: chrono::DateTime<chrono::Utc> = now.into();
        Self {
            seconds: hex2bcd(datetime.second()),
            minutes: hex2bcd(datetime.minute()),
            hours: hex2bcd(datetime.hour()),
            days: hex2bcd(datetime.day()),
            weekday: hex2bcd(datetime.weekday().num_days_from_sunday()),
            month: datetime.month() as u16,

            // Signals
            c0: false,
            c1: false,
            c2: false,
            stb: false,
            cs: false,
            data_in: false,
            gnd: false,
            clk: false,
            data_out: false,
            tp: false,
            out_enable: false,
            n_xtal: false,
            xtal: false,
            vdd: false,

            // Internal
            mode: 0,
            bitno: 0,
            current_bit: 0,
            new_mode: false,
            new_clk: false,
            prev_mode: 0x10,
            prev_clk: false,
            flip_clk: false,
            tp_frequency: 1,

            previous_state: 0,
            previous_state_tp: 0,
        }
    }

    // bool CPD1990AC::step(void)
    // {
    // //	Mode :
    // //			0	-	Register Hold		DATA OUT = 1 Hz
    // //			1	-	Register Shift		DATA OUT = [LSB] = 0 or 1
    // //			2	-	Time Set			DATA OUT = [LSB] = 0 or 1
    // //			3	-	Time Read			DATA OUT = 1 Hz

    // 	if (previous_state == 0) previous_state = pPC->pTIMER->state;

    //     while ( (pPC->pTIMER->state - previous_state) >= pPC->getfrequency() )
    // 	{
    // 		addretrace();
    //         previous_state += pPC->getfrequency();
    // 	};

    // 	if (stb)
    // 	{
    // 		// Mode can change
    // 		mode = c0+(c1<<1)+(c2<<2);
    //         AddLog(LOG_TIME,tr("Mode:%1 m=%2 clk=%3").arg(mode).arg(pd1990ac.minutes).arg(clk));
    //         if (mode !=prev_mode) { New_Mode = true; prev_mode=mode; }
    //         else					New_Mode = false;
    // 	}

    //     if (clk != prev_clk) {	flip_clk=true; prev_clk=clk;	}
    //     else					flip_clk=false;

    //     if (mode == 4)	{
    // //        if (( (CpcXXXX *)pPC)->pCPU->fp_log) fprintf(( (CpcXXXX *)pPC)->pCPU->fp_log,"TP64\n");
    //         TP_FREQUENCY=64;
    //     }
    //     if (mode == 0)	{ clk = true; flip_clk=true; bitno=0; }

    //     if (clk && flip_clk)
    // 	{

    // 		switch (mode)
    // 		{
    // 		case 0x00:	/* Start afresh with shifting */
    // 		case 0x01:	/* Load Register */
    // 			switch(bitno)
    // 			{
    // 			case 0x00: case 0x01: case 0x02: case 0x03:
    // 				data_out=READ_BIT(pd1990ac.seconds , bitno);			// Read seconds 1
    // 				break;
    // 			case 0x04: case 0x05: case 0x06: case 0x07:
    // 				data_out=READ_BIT(pd1990ac.seconds , bitno);			// Read seconds 10
    // 				break;
    // 			case 0x08: case 0x09: case 0x0a: case 0x0b:
    // 				data_out=READ_BIT(pd1990ac.minutes , (bitno-0x08));		// Read minutes 1
    // 				break;
    // 			case 0x0c: case 0x0d: case 0x0e: case 0x0f:
    // 				data_out=READ_BIT(pd1990ac.minutes , (bitno-0x08));		// Read minutes 10
    // 				break;
    // 			case 0x10: case 0x11: case 0x12: case 0x13:
    // 				data_out=READ_BIT(pd1990ac.hours , (bitno-0x10));		// Read hours 1
    // 				break;
    // 			case 0x14: case 0x15: case 0x16: case 0x17:
    // 				data_out=READ_BIT(pd1990ac.hours , (bitno-0x10));		// Read hours 10
    // 				break;
    // 			case 0x18: case 0x19: case 0x1a: case 0x1b:
    // 				data_out=READ_BIT(pd1990ac.days , (bitno-0x18));		// Read day 1
    // 				break;
    // 			case 0x1c: case 0x1d: case 0x1e: case 0x1f:
    // 				data_out=READ_BIT(pd1990ac.days , (bitno-0x18));		// Read day 10
    // 				break;
    // 			case 0x20: case 0x21: case 0x22: case 0x23:
    // 				data_out=READ_BIT(pd1990ac.weekday , (bitno-0x20));		// Read weekday
    // 				break;
    // 			case 0x24: case 0x25: case 0x26: case 0x27:
    // 				data_out=READ_BIT(pd1990ac.month , (bitno-0x24));		// Read month
    // 				break;
    // 			}
    // 			bitno++;
    // 			break;

    // 		case 0x02:	/* Set Register */
    //             AddLog(LOG_TIME,"SET TIME");
    // 			switch(bitno)
    // 			{
    // 			case 0x00: case 0x01: case 0x02: case 0x03:
    // 			case 0x04: case 0x05: case 0x06: case 0x07:
    // 				PUT_BIT(pd1990ac.seconds, bitno, data_in);
    // 				break;
    // 			case 0x08: case 0x09: case 0x0a: case 0x0b:
    // 			case 0x0c: case 0x0d: case 0x0e: case 0x0f:
    // 				PUT_BIT(pd1990ac.minutes, bitno-0x08, data_in);
    // 				break;
    // 			case 0x10: case 0x11: case 0x12: case 0x13:
    // 			case 0x14: case 0x15: case 0x16: case 0x17:
    // 				PUT_BIT(pd1990ac.hours, bitno-0x10, data_in);
    // 				break;
    // 			case 0x18: case 0x19: case 0x1a: case 0x1b:
    // 			case 0x1c: case 0x1d: case 0x1e: case 0x1f:
    // 				PUT_BIT(pd1990ac.days, bitno-0x18,data_in);
    // 				break;
    // 			case 0x20: case 0x21: case 0x22: case 0x23:
    // 				PUT_BIT(pd1990ac.weekday, bitno-0x20,data_in);
    // 				break;
    // 			case 0x24: case 0x25: case 0x26: case 0x27:
    // 				PUT_BIT(pd1990ac.month, bitno-0x24, data_in);
    // 				break;
    // 			}
    // 			bitno++;
    // 			break;

    // 		default:	/* Unhandled value */
    //             //AddLog(LOG_TIME,"MODE %02X (Unhandled) - bitno=%02X, D_in=%s",mode,bitno,data_in?"1":"0");
    // 			break;
    // 		}
    // 	}

    // 	return(1);
    // }
    pub fn step(&mut self, timer_state: usize) -> bool {
        // Mode:
        // 0 - Register Hold DATA OUT = 1 Hz
        // 1 - Register Shift DATA OUT = [LSB] = 0 or 1
        // 2 - Time Set DATA OUT = [LSB] = 0 or 1
        // 3 - Time Read DATA OUT = 1 Hz

        if self.previous_state == 0 {
            self.previous_state = timer_state;
        }

        while (timer_state - self.previous_state) >= FREQUENCY {
            self.previous_state += FREQUENCY;
        }

        if self.stb {
            // Mode can change
            self.mode = (self.c0 as u8) + ((self.c1 as u8) << 1) + ((self.c2 as u8) << 2);
            if self.mode != self.prev_mode {
                self.new_mode = true;
                self.prev_mode = self.mode;
            } else {
                self.new_mode = false;
            }
        }

        if self.clk != self.prev_clk {
            self.flip_clk = true;
            self.prev_clk = self.clk;
        } else {
            self.flip_clk = false;
        }

        if self.mode == 4 {
            self.tp_frequency = 64;
        }
        if self.mode == 0 {
            self.clk = true;
            self.flip_clk = true;
            self.bitno = 0;
        }

        if self.clk && self.flip_clk {
            match self.mode {
                0x00 | 0x01 => {
                    // Start afresh with shifting or Load Register
                    match self.bitno {
                        0x00..=0x03 => {
                            // Seconds (1)
                            self.data_out = read_bit(self.seconds, self.bitno);
                        }
                        0x04..=0x07 => {
                            // Seconds (10)
                            self.data_out = read_bit(self.seconds, self.bitno);
                        }
                        0x08..=0x0B => {
                            // Minutes (1)
                            self.data_out = read_bit(self.minutes, self.bitno - 0x08);
                        }
                        0x0C..=0x0F => {
                            // Minutes (10)
                            self.data_out = read_bit(self.minutes, self.bitno - 0x08);
                        }
                        0x10..=0x13 => {
                            // Hours (1)
                            self.data_out = read_bit(self.hours, self.bitno - 0x10);
                        }
                        0x14..=0x17 => {
                            // Hours (10)
                            self.data_out = read_bit(self.hours, self.bitno - 0x10);
                        }
                        0x18..=0x1B => {
                            // Days (1)
                            self.data_out = read_bit(self.days, self.bitno - 0x18);
                        }
                        0x1C..=0x1F => {
                            // Days (10)
                            self.data_out = read_bit(self.days, self.bitno - 0x18);
                        }
                        0x20..=0x23 => {
                            // Weekday
                            self.data_out = read_bit(self.weekday, self.bitno - 0x20);
                        }
                        0x24..=0x27 => {
                            // Month
                            self.data_out = read_bit(self.month, self.bitno - 0x24);
                        }
                        _ => {}
                    }

                    self.bitno = self.bitno.wrapping_add(1);
                }
                0x02 => {
                    match self.bitno {
                        0x00..=0x07 => {
                            put_bit(&mut self.seconds, self.bitno, self.data_in);
                        }
                        0x08..=0x0F => {
                            put_bit(&mut self.minutes, self.bitno - 0x08, self.data_in);
                        }
                        0x10..=0x17 => {
                            put_bit(&mut self.hours, self.bitno - 0x10, self.data_in);
                        }
                        0x18..=0x1F => {
                            put_bit(&mut self.days, self.bitno - 0x18, self.data_in);
                        }
                        0x20..=0x23 => {
                            put_bit(&mut self.weekday, self.bitno - 0x20, self.data_in);
                        }
                        0x24..=0x27 => {
                            put_bit(&mut self.month, self.bitno - 0x24, self.data_in);
                        }
                        _ => {}
                    }
                    self.bitno = self.bitno.wrapping_add(1);
                }
                _ => {}
            }
        }

        true
    }

    // bool CPD1990AC::Get_data(void)
    // {
    // 	return (data_out);
    // }
    pub fn get_data(&self) -> bool {
        self.data_out
    }

    // bool CPD1990AC::Get_tp(void)
    // {
    // // generate tp signal. Used by tape functionnality
    // #define TP_STATE (pPC->getfrequency() / TP_FREQUENCY)
    // 	qint64 delta_state;

    // 	if (previous_state_tp == 0)
    // 		previous_state_tp = pPC->pTIMER->state;
    // 	while (((pPC->pTIMER->state - previous_state_tp)) >= (TP_STATE / 2))
    // 	{
    // 		tp ^= 1;
    // 		previous_state_tp += (TP_STATE / 2);
    // 	};

    // 	return (tp);
    // }
    pub fn get_tp(&mut self, timer_state: usize) -> bool {
        // generate tp signal. Used by tape functionality
        let tp_state = FREQUENCY / self.tp_frequency;

        if self.previous_state_tp == 0 {
            self.previous_state_tp = timer_state;
        }

        while (timer_state - self.previous_state_tp) >= (tp_state / 2) {
            self.tp = !self.tp;
            self.previous_state_tp += tp_state / 2;
        }

        self.tp
    }

    pub fn set_c0(&mut self, value: bool) {
        self.c0 = value;
    }

    pub fn set_c1(&mut self, value: bool) {
        self.c1 = value;
    }

    pub fn set_c2(&mut self, value: bool) {
        self.c2 = value;
    }

    pub fn set_stb(&mut self, value: bool) {
        self.stb = value;
    }

    pub fn set_cs(&mut self, value: bool) {
        self.cs = value;
    }

    pub fn set_data(&mut self, value: bool) {
        self.data_in = value;
    }

    pub fn set_gnd(&mut self, value: bool) {
        self.gnd = value;
    }

    pub fn set_clk(&mut self, value: bool) {
        self.clk = value;
    }

    pub fn set_out_enable(&mut self, value: bool) {
        self.out_enable = value;
    }

    pub fn set_n_xtal(&mut self, value: bool) {
        self.n_xtal = value;
    }

    pub fn set_xtal(&mut self, value: bool) {
        self.xtal = value;
    }

    pub fn set_vdd(&mut self, value: bool) {
        self.vdd = value;
    }
}
