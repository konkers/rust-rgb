use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_hal_async::i2c::I2c;

use crate::{Error, Result};

pub async fn i2c_read_u8<I2C, E>(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C>,
    device: u8,
    reg: u8,
) -> Result<u8>
where
    I2C: I2c<Error = E>,
    Error: From<E>,
{
    let mut buffer = [0u8];
    let mut i2c = i2c.lock().await;
    i2c.write_read(device, &[reg], &mut buffer).await?;

    Ok(buffer[0])
}

pub async fn i2c_write_u8<I2C, E>(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C>,
    device: u8,
    reg: u8,
    data: u8,
) -> Result<()>
where
    I2C: I2c<Error = E>,
    Error: From<E>,
{
    let mut i2c = i2c.lock().await;
    i2c.write(device, &[reg, data]).await?;
    Ok(())
}

pub async fn i2c_read_u16<I2C, E>(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C>,
    device: u8,
    reg: u8,
) -> Result<u16>
where
    I2C: I2c<Error = E>,
    Error: From<E>,
{
    let mut buffer = [0u8; 2];
    let mut i2c = i2c.lock().await;
    i2c.write_read(device, &[reg], &mut buffer).await?;

    Ok(u16::from_le_bytes(buffer))
}
