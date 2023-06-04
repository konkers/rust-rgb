use bitfield_struct::bitfield;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use embedded_hal_async::digital::Wait;
use esp32c3_hal::gpio::{
    Bank0GpioRegisterAccess, Floating, Gpio7Signals, GpioPin, Input, InputOutputPinType,
    SingleCoreInteruptStatusRegisterAccessBank0,
};
use esp32c3_hal::i2c::I2C;
use esp32c3_hal::peripherals::I2C0;
use esp_println::println;
use num_traits::{FromPrimitive, ToPrimitive};

use super::i2c::{i2c_read_u16, i2c_read_u8, i2c_write_u8};
use crate::{Error, Result};

const ADDR: u8 = 0x6b;

#[repr(u8)]
pub enum Register {
    ChargeCurrentLimit = 0x02,
    ChargeCurrentVoltageLimit = 0x04,
    InputCurrentLimit = 0x06,
    InputVoltageLimit = 0x08,
    IotgRegulation = 0x0a,
    BotgRegulation = 0x0c,
    MinimalSystemVoltage = 0x0e,
    PreChargeCurrent = 0x10,
    TerminationControl = 0x12,
    ChargeControl0 = 0x14,
    ChargeTimerControl = 0x15,
    ChargerControl1 = 0x16,
    ChargerControl2 = 0x17,
    ChargerControl3 = 0x18,
    ChargerControl4 = 0x19,
    NtcControl0 = 0x1a,
    NtcControl1 = 0x1b,
    NtcControl2 = 0x1c,
    ChargerStatus0 = 0x1d,
    ChargerStatus1 = 0x13,
    FaultStatus0 = 0x1f,
    ChargerFlag0 = 0x20,
    ChargerFlag1 = 0x21,
    FaultFlag0 = 0x22,
    ChargerMask0 = 0x23,
    ChargerMask = 0x24,
    FaultMask0 = 0x25,
    AdcControl = 0x26,
    AdcFunctionDisable = 0x27,
    IbusAdc = 0x28,
    IbatAdc = 0x2a,
    VbusAdc = 0x2c,
    VpmidAdc = 0x2e,
    VbatAdc = 0x30,
    VsysAdc = 0x32,
    TsAdc = 0x34,
    TdieAdc = 0x36,
    PartInformation = 0x38,
}

#[bitfield(u8)]
pub struct AdcControl {
    #[bits(2)]
    _res: u8,
    adc_avg_int: bool,
    adc_avg: bool,
    #[bits(2)]
    adc_sample: u8,
    adc_rate: bool,
    adc_en: bool,
}

#[bitfield(u16)]
pub struct AdcVoltage {
    #[bits(2)]
    _res0: u8,
    #[bits(13)]
    raw_voltage: u16,
    #[bits(1)]
    _res15: u8,
}

impl AdcVoltage {
    pub fn microvolts(&self) -> u32 {
        self.raw_voltage() as u32 * 3970
    }
}

type VbusAdc = AdcVoltage;
type VsysAdc = AdcVoltage;

#[bitfield(u8)]
pub struct PartInformation {
    #[bits(3)]
    dev_rev: u8,
    #[bits(3)]
    pn: u8,
    #[bits(2)]
    _res: u8,
}

pub struct Bq25620 {
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'static, I2C0>>,
}

#[macro_export]
macro_rules! bq25620_read_reg8 {
    ($i2c:expr, $reg:ident) => {
        async || -> Result<crate::pd::bq25620::$reg> {
            crate::pd::bq25620::read_u8($i2c, crate::pd::bq25620::Register::$reg)
                .await
                .map(|val| crate::pd::bq25620::$reg::from(val))
        }()
    };
}

#[macro_export]
macro_rules! bq25620_write_reg8 {
    ($i2c:expr, $reg:ident, $data:expr) => {
        crate::pd::bq25620::write_u8($i2c, crate::pd::bq25620::Register::$reg, $data.into())
    };
}

#[macro_export]
macro_rules! bq25620_read_reg16 {
    ($i2c:expr, $reg:ident) => {
        async || -> Result<crate::pd::bq25620::$reg> {
            crate::pd::bq25620::read_u16($i2c, crate::pd::bq25620::Register::$reg)
                .await
                .map(|val| crate::pd::bq25620::$reg::from(val))
        }()
    };
}

impl Bq25620 {
    pub fn new(i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'static, I2C0>>) -> Self {
        Self { i2c }
    }

    pub async fn init(&mut self) -> Result<()> {
        let part_info = bq25620_read_reg8!(self.i2c, PartInformation).await?;
        println!("bq part_info: {:?}", part_info);

        Ok(())
    }

    pub async fn tick(&mut self) -> Result<()> {
        bq25620_write_reg8!(self.i2c, AdcControl, AdcControl::new().with_adc_en(true)).await?;

        let val = bq25620_read_reg16!(self.i2c, VsysAdc).await?;
        println!("bq sys: {} uV", val.microvolts());
        let val = bq25620_read_reg16!(self.i2c, VbusAdc).await?;
        println!("bq bus: {} uV", val.microvolts());

        Ok(())
    }
}

pub(crate) async fn read_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    register: Register,
) -> Result<u8> {
    i2c_read_u8(i2c, ADDR, register as u8).await
}

pub(crate) async fn read_u16(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    register: Register,
) -> Result<u16> {
    i2c_read_u16(i2c, ADDR, register as u8).await
}

pub(crate) async fn write_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    register: Register,
    data: u8,
) -> Result<()> {
    i2c_write_u8(i2c, ADDR, register as u8, data).await
}
