use bitfield_struct::bitfield;
use num_derive::FromPrimitive;

pub use crate::{Error, Result};

#[bitfield(u16)]
pub struct Header {
    #[bits(5)]
    pub message_type: u8,
    pub data_rate: bool,
    #[bits(2)]
    pub spec_revision: u8,
    pub port_power_role: bool,
    #[bits(3)]
    pub message_id: u8,
    #[bits(3)]
    pub num_data_objects: usize,
    pub extended: bool,
}

#[derive(FromPrimitive)]
#[repr(u8)]
pub enum ControlMessageType {
    GoodCrc = 0b00001,
    GotoMin = 0b00010,
    Accept = 0b00011,
    Reject = 0b00100,
    Ping = 0b00101,
    PsRdy = 0b00110,
    GetSrouceCap = 0b00111,
    GetSinkCap = 0b01000,
    DrSawp = 0b01001,
    PrSawp = 0b01010,
    VconnSawp = 0b01011,
    Wait = 0b01100,
    DataReset = 0b01110,
    DataResetComplete = 0b01111,
    NotSupported = 0b10000,
    GetSourceCapExtended = 0b10001,
    GetStatus = 0b10010,
    FrSwap = 0b10011,
    GetPpsStatus = 0b10100,
    GetCountryCodes = 0b10101,
    GetSinkCapExtended = 0b10110,
    GetSourceInfo = 0b10111,
    GetRevision = 0b11000,
}

#[derive(FromPrimitive)]
#[repr(u8)]
pub enum DataMessageType {
    SourceCapabilities = 0b00001,
    Request = 0b00010,
    Bist = 0b00011,
    SinkCapabilities = 0b00100,
    BatteryStatus = 0b00101,
    Alert = 0b00110,
    GetCountryInfo = 0b00111,
    EnterUsb = 0b01000,
    EprRequest = 0b01001,
    EprMode = 0b010010,
    SourceInfo = 0b01011,
    Revision = 0b01100,
    VendorDefined = 0b01111,
}

#[bitfield(u32)]
pub struct FixedSupplyPdo {
    #[bits(10)]
    pub raw_max_current: u16,
    #[bits(10)]
    pub raw_voltage: u16,
    #[bits(2)]
    pub peak_current: u8,
    _reserved: bool,
    pub epr_mode_capable: bool,
    pub unchunked_extended_messages_supported: bool,
    pub dual_role_data: bool,
    pub usb_communications_capable: bool,
    pub unconstrainted_power: bool,
    pub usb_suspend_supporrted: bool,
    pub dual_role_power: bool,
    #[bits(2)]
    pub pdo_type: u8,
}

#[bitfield(u32)]
pub struct FixedVariableSupplyRequest {
    #[bits(10)]
    pub raw_min_operating_current: u16,
    #[bits(10)]
    pub raw_operating_current: u16,
    #[bits(2)]
    _reserved: u8,
    pub erp_mode_capable: bool,
    pub unchuncked_extended_messages_supported: bool,
    pub no_usb_suspend: bool,
    pub usb_communications_capable: bool,
    pub capability_mismatch: bool,
    pub give_back: bool,
    #[bits(4)]
    pub object_position: u8,
}

impl FixedVariableSupplyRequest {
    pub fn with_operating_current(self, val: u32) -> Self {
        self.with_raw_operating_current((val / 10) as u16)
    }

    pub fn with_min_operating_current(self, val: u32) -> Self {
        self.with_raw_min_operating_current((val / 10) as u16)
    }
}

impl FixedSupplyPdo {
    pub fn max_current(&self) -> u32 {
        self.raw_max_current() as u32 * 10
    }

    pub fn voltage(&self) -> u32 {
        self.raw_voltage() as u32 * 50
    }

    pub fn power(&self) -> u32 {
        self.voltage() * self.max_current() / 1000
    }
}
