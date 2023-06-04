use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use esp32c3_hal::i2c::I2C;
use esp32c3_hal::peripherals::I2C0;
use esp32c3_hal::prelude::*;

use crate::Result;

pub async fn i2c_read_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    device: u8,
    reg: u8,
) -> Result<u8> {
    let mut buffer = [0u8];
    let mut i2c = i2c.lock().await;
    i2c.write_read(device, &[reg], &mut buffer)?;

    Ok(buffer[0])
}

pub async fn i2c_write_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    device: u8,
    reg: u8,
    data: u8,
) -> Result<()> {
    let mut i2c = i2c.lock().await;
    i2c.write(device, &[reg, data])?;
    Ok(())
}

pub async fn i2c_read_u16(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    device: u8,
    reg: u8,
) -> Result<u16> {
    let mut buffer = [0u8; 2];
    let mut i2c = i2c.lock().await;
    i2c.write_read(device, &[reg], &mut buffer)?;

    Ok(u16::from_le_bytes(buffer))
}
