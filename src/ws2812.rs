const RESET_LEN: usize = 200;
pub struct Ws2812<'a, const BUF_SIZE: usize> {
    data: &'a mut [u8],
}

impl<'a, const BUF_SIZE: usize> Ws2812<'a, BUF_SIZE> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }

    pub fn into_buf(self) -> &'a mut [u8] {
        self.data
    }

    pub fn set_led(&mut self, index: usize, r: u8, g: u8, b: u8) {
        let buf = &mut self.data[RESET_LEN + index * 9..];

        let buf = Self::set_byte(buf, g);
        let buf = Self::set_byte(buf, r);
        Self::set_byte(buf, b);
    }

    fn set_byte(buf: &mut [u8], mut data: u8) -> &mut [u8] {
        let mut encoded = 0u32;
        for _ in 0..8 {
            encoded <<= 3;
            if (data & 0x80) == 0 {
                encoded |= 0b100;
            } else {
                encoded |= 0b110;
            }
            data <<= 1;
        }
        buf[0] = ((encoded >> 16) & 0xff) as u8;
        buf[1] = ((encoded >> 8) & 0xff) as u8;
        buf[2] = ((encoded) & 0xff) as u8;

        &mut buf[3..]
    }

    #[allow(dead_code)]
    pub fn num_leds() -> usize {
        (BUF_SIZE - RESET_LEN * 2) / 9
    }
}

pub const fn buffer_len(num_leds: usize) -> usize {
    RESET_LEN * 2 + num_leds * 9
}
