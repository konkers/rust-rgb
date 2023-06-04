use core::convert::Infallible;

use crate::hal;

pub enum Error {
    I2cError(hal::i2c::Error),
    SoftResetFailure,
    InvalidDeviceId,
    NoCcDetected,
    Index,
    Infallible,
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2cError(arg0) => f.debug_tuple("I2cError").field(arg0).finish(),
            Self::SoftResetFailure => write!(f, "Soft reset failure"),
            Self::InvalidDeviceId => write!(f, "InvalidDeviceId"),
            Self::NoCcDetected => write!(f, "No CC line detected"),
            Self::Index => write!(f, "Index error"),
            Self::Infallible => write!(f, "Infalible"),
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl core::error::Error for Error {}

impl From<hal::i2c::Error> for Error {
    fn from(e: hal::i2c::Error) -> Self {
        Self::I2cError(e)
    }
}

impl From<Infallible> for Error {
    fn from(_e: Infallible) -> Self {
        Self::Infallible
    }
}

pub type Result<T> = core::result::Result<T, Error>;
