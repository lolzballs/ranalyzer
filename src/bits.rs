use std::io::Read;

pub struct BitstreamReader<'a, T> {
    reader: &'a mut T,

    buf: u32,
    remaining: u8, // remaining bits
    eof: bool,
}

impl<'a, T: Read> BitstreamReader<'a, T> {
    pub fn new(reader: &'a mut T) -> std::io::Result<Self> {
        let mut empty = Self {
            reader,
            buf: 0,
            remaining: 0,
            eof: false,
        };

        empty.refill()?;
        Ok(empty)
    }

    /// f(n) - 4.10.2
    pub fn f(&mut self, n: u8) -> std::io::Result<u32> {
        self.get_bits(n)
    }

    /// Special helper for f(1) - 4.10.2
    pub fn f1(&mut self) -> std::io::Result<bool> {
        Ok(self.get_bits(1)? == 1)
    }

    /// uvlc() - 4.10.3
    pub fn uvlc(&mut self) -> std::io::Result<u32> {
        let mut leading_zeros = 0;
        while !self.f1()? {
            leading_zeros += 1;
        }

        if leading_zeros >= 32 {
            Ok(std::u32::MAX)
        } else {
            Ok(self.f(leading_zeros)? + (1 << leading_zeros) - 1)
        }
    }

    /// le(n) - 4.10.4
    pub fn le(&mut self, n: u8) -> std::io::Result<u32> {
        self.get_bits(n * 4)
    }

    /// leb128() - 4.10.5
    pub fn leb128(&mut self) -> std::io::Result<u32> {
        let mut value = 0;
        for i in 0..8 {
            let byte = self.f(8)?;
            value |= (byte & 0x7f) << (i * 7);

            if byte & 0x80 != 0 {
                break;
            }
        }
        Ok(value)
    }

    /// su(n) - 4.10.6
    pub fn su(&mut self, n: u8) -> std::io::Result<i32> {
        let value = self.f(n)?;
        let sign_mask = 1 << (n - 1);

        if (value & sign_mask) != 0 {
            Ok((value as i32)
                .overflowing_sub_unsigned(sign_mask)
                .0
                .overflowing_sub_unsigned(sign_mask)
                .0)
        } else {
            Ok(value as i32)
        }
    }

    /// ns(n) - 4.10.7
    pub fn ns(&mut self, n: u8) -> std::io::Result<u32> {
        let w = n.ilog2() as u8 + 1;
        let m = (1 << w) - (n as u32);
        let v = self.f(w - 1)?;

        if v < m {
            Ok(v)
        } else {
            Ok((v << 1) - m + self.f(1)?)
        }
    }

    fn get_bits(&mut self, n: u8) -> std::io::Result<u32> {
        assert!(n <= 32);

        let (res, taken) = if self.remaining >= n {
            (self.buf >> (32 - n), n)
        } else if self.eof {
            return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        } else {
            let initial_bits = self.remaining;
            let remainder = n - initial_bits;
            let initial = self.buf >> (32 - initial_bits - remainder);

            self.refill()?;

            (initial | (self.buf >> (32 - remainder)), remainder)
        };

        self.remaining -= taken;
        self.buf = if taken == 32 { 0 } else { self.buf << taken };

        Ok(res as u32)
    }

    fn refill(&mut self) -> std::io::Result<()> {
        self.remaining = 0;
        while self.remaining < 32 {
            match self.read_u8() {
                Ok(byte) => {
                    self.buf = (self.buf << 8) | (byte as u32);
                    self.remaining += 8;
                }
                Err(err) => {
                    match err.kind() {
                        std::io::ErrorKind::UnexpectedEof => {
                            // We aren't reading any more bits - shift the buffer to the end
                            if self.remaining > 0 {
                                self.buf <<= 32 - self.remaining;
                            }
                            self.eof = true;
                            break;
                        }
                        _ => return Err(err),
                    }
                }
            }
        }

        Ok(())
    }

    #[inline]
    fn read_u8(&mut self) -> std::io::Result<u8> {
        let mut buf = [0u8; 1];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const DATA_BUF: [u8; 34] = [
        0b10110011, 0b10001111, 0b00001111, 0b10000011, 0b11110000, 0b00111111, 0b10000000,
        0b11111111, 0b00000000, 0b11111111, 0b10000000, 0b00111111, 0b11110000, 0b00000011,
        0b11111111, 0b10000000, 0b00001111, 0b11111111, 0b00000000, 0b00001111, 0b11111111,
        0b10000000, 0b00000011, 0b11111111, 0b11110000, 0b00000000, 0b00111111, 0b11111111,
        0b10000000, 0b00000000, 0b11111111, 0b11111111, 0b00000000, 0b00000000,
    ];

    #[test]
    fn f_simple() {
        let mut bytes = Cursor::new(&DATA_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();

        for i in 1..=16 {
            assert_eq!(bs.f(i).unwrap(), ((1u32 << i) - 1));
            assert_eq!(bs.f(i).unwrap(), 0);
        }

        // Should get EOF now
        assert!(bs.f(1).is_err());
    }

    #[test]
    fn f_edge() {
        let mut bytes = Cursor::new(&DATA_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();

        let dword = ((DATA_BUF[0] as u32) << 24)
            | ((DATA_BUF[1] as u32) << 16)
            | ((DATA_BUF[2] as u32) << 8)
            | ((DATA_BUF[3] as u32) << 0);
        assert_eq!(bs.f(31).unwrap(), dword >> 1);
        assert_eq!(bs.f(2).unwrap(), ((dword & 1) << 1) | 1);
    }

    #[test]
    fn uvlc_32_leading_zeroes() {
        const TEST_BUF: [u8; 5] = [0x00, 0x00, 0x00, 0x00, 0x80];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.uvlc().unwrap(), std::u32::MAX);
    }

    #[test]
    fn uvlc_33_leading_zeroes() {
        const TEST_BUF: [u8; 5] = [0x00, 0x00, 0x00, 0x00, 0x40];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.uvlc().unwrap(), std::u32::MAX);
    }

    #[test]
    fn uvlc_9_leading_zeroes_value_2() {
        const TEST_BUF: [u8; 3] = [0x00, 0b01000000, 0b01000000];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.uvlc().unwrap(), 1 + (1 << 9));
    }

    #[test]
    fn uvlc_9_leading_zeroes_value_14() {
        const TEST_BUF: [u8; 3] = [0x00, 0b01000001, 0b11000000];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.uvlc().unwrap(), 13 + (1 << 9));
    }

    #[test]
    fn uvlc_8_leading_zeroes_value_1() {
        const TEST_BUF: [u8; 3] = [0x00, 0b10000000, 0b10000000];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.uvlc().unwrap(), 1 << 8);
    }

    #[test]
    fn uvlc_8_leading_zeroes_value_255() {
        const TEST_BUF: [u8; 3] = [0x00, 0b11111111, 0b10000000];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.uvlc().unwrap(), 254 + (1 << 8));
    }

    #[test]
    fn uvlc_5_leading_zeroes_value_8() {
        const TEST_BUF: [u8; 2] = [0b00000101, 0b00000000];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.uvlc().unwrap(), 7 + (1 << 5));
    }

    #[test]
    fn uvlc_5_leading_zeroes_value_11() {
        const TEST_BUF: [u8; 2] = [0b00000101, 0b01100000];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.uvlc().unwrap(), 10 + (1 << 5));
    }

    #[test]
    fn su_4() {
        const TEST_BUF: [u8; 2] = [0b00011111, 0b00101110];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.su(4).unwrap(), 1);
        assert_eq!(bs.su(4).unwrap(), -1);
        assert_eq!(bs.su(4).unwrap(), 2);
        assert_eq!(bs.su(4).unwrap(), -2);
    }

    #[test]
    fn su_7() {
        const TEST_BUF: [u8; 2] = [0b00000011, 0b11111100];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.su(7).unwrap(), 1);
        assert_eq!(bs.su(7).unwrap(), -1);
    }

    #[test]
    fn su_32() {
        const TEST_BUF: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.su(32).unwrap(), 0xffffff);
        assert_eq!(bs.su(32).unwrap(), -1);
    }

    #[test]
    fn ns_5() {
        const TEST_BUF: [u8; 2] = [0b00011011, 0b01110000];

        let mut bytes = Cursor::new(&TEST_BUF);
        let mut bs = BitstreamReader::new(&mut bytes).unwrap();
        assert_eq!(bs.ns(5).unwrap(), 0);
        assert_eq!(bs.ns(5).unwrap(), 1);
        assert_eq!(bs.ns(5).unwrap(), 2);
        assert_eq!(bs.ns(5).unwrap(), 3);
        assert_eq!(bs.ns(5).unwrap(), 4);
    }
}
