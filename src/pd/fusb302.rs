use bitfield_struct::bitfield;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use esp32c3_hal::i2c::I2C;
use esp32c3_hal::peripherals::I2C0;
use esp32c3_hal::prelude::*;
use num_derive::{FromPrimitive, ToPrimitive};

use super::i2c::{i2c_read_u8, i2c_write_u8};
use crate::{Error, Result};

const FUSB302_ADDR: u8 = 0x22;

#[repr(u8)]
#[allow(unused)]
pub enum Fusb302Register {
    DeviceId = 0x01,
    Switches0 = 0x02,
    Switches1 = 0x03,
    Measure = 0x04,
    Slice = 0x05,
    Control0 = 0x06,
    Control1 = 0x07,
    Control2 = 0x08,
    Control3 = 0x09,
    Mask1 = 0x0a,
    Power = 0x0b,
    Reset = 0x0c,
    OCPreg = 0x0d,
    MaskA = 0x0e,
    MaskB = 0x0f,
    Control4 = 0x10,
    Status0A = 0x3c,
    Status1A = 0x3d,
    InterruptA = 0x3e,
    InterruptB = 0x3f,
    Status0 = 0x40,
    Status1 = 0x41,
    Interrupt = 0x42,
    Fifos = 0x43,
}

#[bitfield(u8)]
pub struct DeviceId {
    #[bits(2)]
    pub revision: u8,
    #[bits(2)]
    pub product: u8,
    #[bits(4)]
    pub version: u8,
}

#[bitfield(u8)]
pub struct Switches0 {
    pub pdwn1: bool,
    pub pdwn2: bool,
    pub meas_cc1: bool,
    pub meas_cc2: bool,
    pub vconn_cc1: bool,
    pub vconn_cc2: bool,
    pub pu_en1: bool,
    pub pu_en2: bool,
}

#[bitfield(u8)]
pub struct Switches1 {
    pub txcc1: bool,
    pub txcc2: bool,
    pub auto_crc: bool,
    _reserved: bool,
    pub data_role: bool,
    #[bits(2)]
    pub spec_rev: u8,
    pub power_role: bool,
}

#[bitfield(u8)]
pub struct Measure {
    #[bits(6)]
    pub mdac: u8,
    pub meas_vbus: bool,
    _reserved: bool,
}

#[bitfield(u8)]
pub struct Slice {
    pub sdac0: bool,
    pub sdac1: bool,
    pub sdac2: bool,
    pub sdac3: bool,
    pub sdac4: bool,
    pub sdac5: bool,
    pub sdac_hys2: bool,
    pub sdac_hys1: bool,
}

#[bitfield(u8)]
pub struct Control0 {
    pub tx_start: bool,
    pub auto_pre: bool,
    #[bits(2)]
    pub host_cur: u8,
    _reserved0: bool,
    pub int_mask: bool,
    pub tx_flush: bool,
    _reserved1: bool,
}

#[bitfield(u8)]
pub struct Control1 {
    pub ensop1: bool,
    pub ensop2: bool,
    pub rx_flush: bool,
    _reserved0: bool,
    pub bist_mode2: bool,
    pub ensop1db: bool,
    pub ensop2db: bool,
    _reserved1: bool,
}

#[bitfield(u8)]
pub struct Control2 {
    pub toggle: bool,
    #[bits(2)]
    pub mode: u8,
    pub wake_en: bool,
    _reserved0: bool,
    pub tog_rd_only: bool,
    pub tog_save_pwr1: bool,
    pub tog_save_pwr2: bool,
}

#[bitfield(u8)]
pub struct Control3 {
    pub auto_retry: bool,
    #[bits(2)]
    pub n_retries: u8,
    pub auto_hardreset: bool,
    pub auto_softreset: bool,
    pub bist_t_mode: bool,
    pub send_hard_reset: bool,
    _reserved: bool,
}

#[bitfield(u8)]
pub struct Mask1 {
    pub m_bc_lvl: bool,
    pub m_collision: bool,
    pub m_wake: bool,
    pub m_alert: bool,
    pub m_crc_chk: bool,
    pub m_comp_chng: bool,
    pub m_activity: bool,
    pub m_vbusok: bool,
}

#[bitfield(u8)]
pub struct Power {
    pub pwr0: bool,
    pub pwr1: bool,
    pub pwr2: bool,
    pub pwr3: bool,
    #[bits(4)]
    _reserved: u8,
}

#[bitfield(u8)]
pub struct Reset {
    pub sw_res: bool,
    pub pd_reset: bool,
    #[bits(6)]
    _reserved: u8,
}

#[bitfield(u8)]
pub struct OCPreg {
    pub ocp_cur0: bool,
    pub ocp_cur1: bool,
    pub ocp_cur2: bool,
    pub ocp_cur3: bool,
    #[bits(4)]
    _reserved: u8,
}

#[bitfield(u8)]
pub struct MaskA {
    pub m_hardrst: bool,
    pub m_softrst: bool,
    pub m_txsent: bool,
    pub m_hardsent: bool,
    pub m_retryfail: bool,
    pub m_softfail: bool,
    pub m_togdone: bool,
    pub m_ocp_temp: bool,
}

#[bitfield(u8)]
pub struct MaskB {
    pub m_gcrcsent: bool,
    #[bits(7)]
    _reserved: u8,
}

#[bitfield(u8)]
pub struct Control4 {
    pub tog_exit_aud: bool,
    #[bits(7)]
    _reserved: u8,
}

#[bitfield(u8)]
pub struct Status0A {
    pub hardrst: bool,
    pub softrst: bool,
    pub power2: bool,
    pub power3: bool,
    pub retryfail: bool,
    pub softfail: bool,
    #[bits(2)]
    _reserved: u8,
}

#[bitfield(u8)]
pub struct Status1A {
    pub rxsop: bool,
    pub rxsop1db: bool,
    pub rssop2db: bool,
    pub togss1: bool,
    pub togss2: bool,
    pub togss3: bool,
    #[bits(2)]
    _reserved: u8,
}

#[bitfield(u8)]
pub struct InterruptA {
    pub i_hardrst: bool,
    pub i_softrst: bool,
    pub i_txsent: bool,
    pub i_hardsent: bool,
    pub i_retryfail: bool,
    pub i_softfail: bool,
    pub i_togdone: bool,
    pub i_ocp_temp: bool,
}

#[bitfield(u8)]
pub struct InterruptB {
    pub i_gcrcsent: bool,
    #[bits(7)]
    _reserved: u8,
}

#[bitfield(u8)]
pub struct Status0 {
    #[bits(2)]
    pub bc_lvl: u8,
    pub wake: bool,
    pub alert: bool,
    pub crc_chk: bool,
    pub comp: bool,
    pub activity: bool,
    pub vbusok: bool,
}

#[bitfield(u8)]
pub struct Status1 {
    pub ocp: bool,
    pub overtemp: bool,
    pub tx_full: bool,
    pub tx_empty: bool,
    pub rx_full: bool,
    pub rx_empty: bool,
    pub rxsop1: bool,
    pub rxsop2: bool,
}

#[bitfield(u8)]
pub struct Interrupt {
    pub i_bc_lvl: bool,
    pub i_collision: bool,
    pub i_wake: bool,
    pub i_alert: bool,
    pub i_crc_chk: bool,
    pub i_comp_chng: bool,
    pub i_activity: bool,
    pub i_vbusok: bool,
}

#[derive(Debug)]
pub struct Status {
    pub status_0a: Status0A,
    pub status_1a: Status1A,
    pub interrupt_a: InterruptA,
    pub interrupt_b: InterruptB,
    pub status_0: Status0,
    pub status_1: Status1,
    pub interrupt: Interrupt,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            status_0a: Status0A::new(),
            status_1a: Status1A::new(),
            interrupt_a: InterruptA::new(),
            interrupt_b: InterruptB::new(),
            status_0: Status0::new(),
            status_1: Status1::new(),
            interrupt: Interrupt::new(),
        }
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RxTokenType {
    Reserved0 = 0b000,
    Reserved1 = 0b001,
    Reserved2 = 0b010,
    Sop2Db = 0b011,
    Sop1Db = 0b100,
    Sop2 = 0b101,
    Sop1 = 0b110,
    Sop = 0b111,
}

impl From<u8> for RxTokenType {
    fn from(value: u8) -> Self {
        // RxTokenType is valid for all values from 0b000 to 0b111
        unsafe { core::mem::transmute(value & 0b111) }
    }
}

impl From<RxTokenType> for u8 {
    fn from(value: RxTokenType) -> u8 {
        value as u8
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TxToken {
    Sop1 = 0x12,
    Sop2 = 0x13,
    Eop = 0x14,
    Reset1 = 0x15,
    Reset2 = 0x16,
    Sop3 = 0x1b,
    PackSym = 0x80,
    TxOn = 0xa1,
    TxOff = 0xfe,
    JamCrc = 0xff,
}

#[bitfield(u8)]
pub struct RxToken {
    #[bits(5)]
    _reserved: u8,
    #[bits(3)]
    pub token: RxTokenType,
}

pub(crate) async fn fusb302_read(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    register: Fusb302Register,
    data: &mut [u8],
) -> Result<()> {
    let mut i2c = i2c.lock().await;
    i2c.write_read(FUSB302_ADDR, &[register as u8], data)?;
    Ok(())
}

pub(crate) async fn fusb302_read_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    register: Fusb302Register,
) -> Result<u8> {
    i2c_read_u8(i2c, FUSB302_ADDR, register as u8).await
}

pub(crate) async fn fusb302_write_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    register: Fusb302Register,
    data: u8,
) -> Result<()> {
    i2c_write_u8(i2c, FUSB302_ADDR, register as u8, data).await
}

pub(crate) async fn fusb302_read_status(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
) -> Result<Status> {
    let mut data = [0u8; 7];
    fusb302_read(i2c, Fusb302Register::Status0A, &mut data).await?;
    Ok(Status {
        status_0a: Status0A::from(data[0]),
        status_1a: Status1A::from(data[1]),
        interrupt_a: InterruptA::from(data[2]),
        interrupt_b: InterruptB::from(data[3]),
        status_0: Status0::from(data[4]),
        status_1: Status1::from(data[5]),
        interrupt: Interrupt::from(data[6]),
    })
}

pub(crate) async fn fusb302_read_fifo(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    buffer: &mut [u8],
) -> Result<()> {
    fusb302_read(i2c, Fusb302Register::Fifos, buffer).await?;
    Ok(())
}

pub(crate) async fn fusb302_read_fifo_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
) -> Result<u8> {
    let mut buffer = [0u8];
    fusb302_read(i2c, Fusb302Register::Fifos, &mut buffer).await?;
    Ok(buffer[0])
}

pub(crate) async fn fusb302_read_fifo_u16(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
) -> Result<u16> {
    let mut buffer = [0u8; 2];
    fusb302_read(i2c, Fusb302Register::Fifos, &mut buffer).await?;
    Ok(u16::from_le_bytes(buffer))
}

pub(crate) async fn fusb302_read_fifo_u32(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
) -> Result<u32> {
    let mut buffer = [0u8; 4];
    fusb302_read(i2c, Fusb302Register::Fifos, &mut buffer).await?;
    Ok(u32::from_le_bytes(buffer))
}

const MESSAGE_BUFFER_SIZE: usize = 1 /* register addr */ + 2 /* header */ + 7 * 4 /* maximum number of data objects */;

#[derive(Debug)]
pub(crate) struct Fusb302MessageBuffer {
    buffer: [u8; MESSAGE_BUFFER_SIZE],
    len: usize,
}

impl Fusb302MessageBuffer {
    pub fn new() -> Self {
        Self {
            buffer: [
                Fusb302Register::Fifos as u8,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
            len: 3, // register address + 2 bytes header
        }
    }

    pub fn write_header(&mut self, header: u16) {
        let bytes = header.to_le_bytes();
        self.buffer[1] = bytes[0];
        self.buffer[2] = bytes[1];
    }

    pub fn write_data(&mut self, object: u32) -> Result<()> {
        if self.len + 4 > MESSAGE_BUFFER_SIZE {
            return Err(Error::Index);
        }
        let bytes = object.to_le_bytes();
        for i in 0..4 {
            self.buffer[self.len + i] = bytes[i];
        }
        self.len += 4;

        Ok(())
    }

    pub async fn send(
        &self,
        i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    ) -> Result<()> {
        let mut i2c = i2c.lock().await;
        let sop_seq = [
            Fusb302Register::Fifos as u8,
            TxToken::Sop1 as u8,
            TxToken::Sop1 as u8,
            TxToken::Sop1 as u8,
            TxToken::Sop2 as u8,
            TxToken::PackSym as u8 + (self.len - 1) as u8,
        ];
        i2c.write(FUSB302_ADDR, &sop_seq)?;
        i2c.write(FUSB302_ADDR, &self.buffer[..self.len])?;
        let eop_seq = [
            Fusb302Register::Fifos as u8,
            TxToken::JamCrc as u8,
            TxToken::Eop as u8,
            TxToken::TxOff as u8,
            TxToken::TxOn as u8,
        ];
        i2c.write(FUSB302_ADDR, &eop_seq)?;

        Ok(())
    }
}

#[macro_export]
macro_rules! fusb302_read_reg {
    ($i2c:expr, $reg:ident) => {
        crate::pd::fusb302::fusb302_read_u8($i2c, crate::pd::fusb302::Fusb302Register::$reg)
            .await
            .map(|val| $reg::from(val))
    };
}

#[macro_export]
macro_rules! fusb302_write_reg {
    ($i2c:expr, $reg:ident, $data:expr) => {
        crate::pd::fusb302::fusb302_write_u8(
            $i2c,
            crate::pd::fusb302::Fusb302Register::$reg,
            $data.into(),
        )
        .await
    };
}
