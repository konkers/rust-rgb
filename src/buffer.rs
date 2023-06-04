use core::marker::PhantomData;

use byteorder::ByteOrder;

#[derive(Debug)]
pub enum Error {
    Eof,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Eof => write!(f, "End of buffer reached"),
        }
    }
}

impl core::error::Error for Error {}

type Result<T> = core::result::Result<T, Error>;

#[allow(dead_code)] // TODO: konkers - remember what I was doing here...
pub struct Buffer<T> {
    inner: T,
    pos: usize,
}

impl<T> Buffer<T> {
    #[allow(dead_code)]
    pub const fn new(inner: T) -> Buffer<T> {
        Buffer { pos: 0, inner }
    }

    #[allow(dead_code)]
    pub fn into_inner(self) -> T {
        self.inner
    }

    #[allow(dead_code)]
    pub const fn position(&self) -> usize {
        self.pos
    }
}

impl<T> Buffer<T>
where
    T: AsRef<[u8]>,
{
    #[allow(dead_code)]
    fn ensure_space(&self, n_bytes: usize) -> Result<()> {
        if (self.pos + n_bytes) > self.inner.as_ref().len() as usize {
            Err(Error::Eof)
        } else {
            Ok(())
        }
    }

    #[allow(dead_code)]
    pub fn take(&mut self, n_bytes: usize) -> Result<&[u8]> {
        self.ensure_space(n_bytes)?;
        let data = &self.inner.as_ref()[self.pos..(self.pos + n_bytes)];
        self.pos += n_bytes;
        Ok(data)
    }
}

pub struct OldBuffer<'a, ENDIAN: ByteOrder> {
    data: &'a [u8],
    pos: usize,
    phantom: PhantomData<ENDIAN>,
}

impl<'a, ENDIAN: ByteOrder> OldBuffer<'a, ENDIAN> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            phantom: PhantomData,
        }
    }

    #[allow(dead_code)]
    pub fn pos(&self) -> usize {
        self.pos
    }

    fn ensure_space(&self, n_bytes: usize) -> Result<()> {
        if (self.pos + n_bytes) > self.data.len() {
            Err(Error::Eof)
        } else {
            Ok(())
        }
    }

    pub fn take(&mut self, n_bytes: usize) -> Result<&'a [u8]> {
        self.ensure_space(n_bytes)?;
        let data = &self.data[self.pos..(self.pos + n_bytes)];
        self.pos += n_bytes;
        Ok(data)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let data = self.take(1)?;
        Ok(data[0])
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let data = self.take(2)?;
        Ok(ENDIAN::read_u16(data))
    }

    #[allow(dead_code)]
    pub fn read_u32(&mut self) -> Result<u32> {
        let data = self.take(4)?;
        Ok(ENDIAN::read_u32(data))
    }

    #[allow(dead_code)]
    pub fn read_u64(&mut self) -> Result<u64> {
        let data = self.take(8)?;
        Ok(ENDIAN::read_u64(data))
    }

    #[allow(dead_code)]
    pub fn read_u128(&mut self) -> Result<u128> {
        let data = self.take(16)?;
        Ok(ENDIAN::read_u128(data))
    }

    pub fn read_buf<'b>(&mut self, buf: &'b mut [u8]) -> Result<()> {
        let data = self.take(buf.len())?;
        buf.copy_from_slice(data);
        Ok(())
    }

    pub fn read<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut buf = [0u8; N];
        let data = self.take(buf.len())?;
        buf.copy_from_slice(data);
        Ok(buf)
    }
}

pub struct MutBuffer<'a, ENDIAN: ByteOrder> {
    data: &'a mut [u8],
    pos: usize,
    phantom: PhantomData<ENDIAN>,
}

impl<'a, ENDIAN: ByteOrder> MutBuffer<'a, ENDIAN> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self {
            data,
            pos: 0,
            phantom: PhantomData,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    fn ensure_space(&self, n_bytes: usize) -> Result<()> {
        if (self.pos + n_bytes) > self.data.len() {
            Err(Error::Eof)
        } else {
            Ok(())
        }
    }

    #[allow(dead_code)]
    pub fn take(&mut self, n_bytes: usize) -> Result<&[u8]> {
        self.ensure_space(n_bytes)?;
        let data = &(self.data[self.pos..(self.pos + n_bytes)]);
        self.pos += n_bytes;
        Ok(data)
    }

    pub fn take_mut(&mut self, n_bytes: usize) -> Result<&mut [u8]> {
        self.ensure_space(n_bytes)?;
        let data = &mut self.data[self.pos..(self.pos + n_bytes)];
        self.pos += n_bytes;
        Ok(data)
    }

    #[allow(dead_code)]
    pub fn read_u8(&mut self) -> Result<u8> {
        let data = self.take(1)?;
        Ok(data[0])
    }

    #[allow(dead_code)]
    pub fn read_u16(&mut self) -> Result<u16> {
        let data = self.take(2)?;
        Ok(ENDIAN::read_u16(data))
    }

    #[allow(dead_code)]
    pub fn read_u32(&mut self) -> Result<u32> {
        let data = self.take(4)?;
        Ok(ENDIAN::read_u32(data))
    }

    #[allow(dead_code)]
    pub fn read_u64(&mut self) -> Result<u64> {
        let data = self.take(8)?;
        Ok(ENDIAN::read_u64(data))
    }

    #[allow(dead_code)]
    pub fn read_u128(&mut self) -> Result<u128> {
        let data = self.take(16)?;
        Ok(ENDIAN::read_u128(data))
    }

    pub fn write_u8(&mut self, val: u8) -> Result<()> {
        let data = self.take_mut(1)?;
        data[0] = val;
        Ok(())
    }

    pub fn write_u16(&mut self, val: u16) -> Result<()> {
        let data = self.take_mut(2)?;
        ENDIAN::write_u16(data, val);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn write_u32(&mut self, val: u32) -> Result<()> {
        let data = self.take_mut(4)?;
        ENDIAN::write_u32(data, val);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn write_u64(&mut self, val: u64) -> Result<()> {
        let data = self.take_mut(8)?;
        ENDIAN::write_u64(data, val);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn write_u128(&mut self, val: u128) -> Result<()> {
        let data = self.take_mut(16)?;
        ENDIAN::write_u128(data, val);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn read_buf(&mut self, buf: &mut [u8]) -> Result<()> {
        let data = self.take_mut(buf.len())?;
        buf.copy_from_slice(data);
        Ok(())
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<()> {
        let data = self.take_mut(buf.len())?;
        data.copy_from_slice(buf);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn read<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut buf = [0u8; N];
        let data = self.take(buf.len())?;
        buf.copy_from_slice(data);
        Ok(buf)
    }
}
