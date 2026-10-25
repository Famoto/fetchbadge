//! Minimal GT911 touch driver for the T5 4.7" S3 touch panel (I2C SDA=18 SCL=17, INT=47).
//! INT is on GPIO47, which is not an RTC pin, so touch cannot wake the chip from deep sleep.
use esp_hal::{delay::Delay, gpio::Flex, i2c::master::I2c, Blocking};

const STATUS: [u8; 2] = [0x81, 0x4E]; // bit7 = data ready, low nibble = touch count
const POINT1: [u8; 2] = [0x81, 0x50]; // x lo, x hi, y lo, y hi
const COMMAND: [u8; 2] = [0x80, 0x40];

pub struct Touch<'d> {
    i2c: I2c<'d, Blocking>,
    int: Flex<'d>,
    addr: u8,
}

impl<'d> Touch<'d> {
    /// Wakes the GT911 (INT high pulse) and finds it on 0x5D or 0x14. None = no touch panel.
    pub fn new(mut i2c: I2c<'d, Blocking>, mut int: Flex<'d>, delay: &Delay) -> Option<Self> {
        // It may still be booting after the panel rail came up, so retry for ~1 s.
        let found = (0..20).find_map(|attempt| {
            int.set_output_enable(true);
            int.set_high();
            delay.delay_millis(5);
            int.set_output_enable(false);
            delay.delay_millis(50);
            let addr = [0x5D, 0x14].into_iter().find(|&a| i2c.write(a, &STATUS).is_ok())?;
            esp_println::println!("touch: GT911 at {addr:#x}, attempt {attempt}");
            Some(addr)
        });
        let Some(addr) = found else {
            // ponytail: bring-up diagnostic; 0x51 is the PCF8563 on the same bus.
            let found: alloc::vec::Vec<u8> =
                (0x08..0x78).filter(|&a| i2c.read(a, &mut [0]).is_ok()).collect();
            esp_println::println!("touch: no GT911 on 0x5D/0x14; I2C answers: {found:02x?}");
            return None;
        };
        Some(Self { i2c, int, addr })
    }

    /// First touch point in display coordinates (960x540), if a finger is down.
    pub fn read(&mut self) -> Option<(i32, i32)> {
        let mut st = [0u8];
        self.i2c.write_read(self.addr, &STATUS, &mut st).ok()?;
        if st[0] & 0x80 == 0 {
            return None;
        }
        let mut p = [0u8; 4];
        let n = st[0] & 0x0F;
        if n > 0 {
            self.i2c.write_read(self.addr, &POINT1, &mut p).ok()?;
        }
        let _ = self.i2c.write(self.addr, &[STATUS[0], STATUS[1], 0]); // ack
        if n == 0 {
            return None;
        }
        let rx = u16::from_le_bytes([p[0], p[1]]) as i32;
        let ry = u16::from_le_bytes([p[2], p[3]]) as i32;
        // Swap XY + mirror Y, same as LilyGO's touch example. Taps landing mirrored? Change this line.
        Some((ry, 540 - rx))
    }

    /// GT911 sleep mode (it draws mA while idle-awake): INT low, then command 5.
    pub fn sleep(mut self, delay: &Delay) {
        self.int.set_output_enable(true);
        self.int.set_low();
        delay.delay_millis(5);
        let _ = self.i2c.write(self.addr, &[COMMAND[0], COMMAND[1], 0x05]);
        delay.delay_millis(5);
        self.int.set_output_enable(false);
    }
}
