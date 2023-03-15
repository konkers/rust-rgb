use crate::hal;

pub(crate) enum Error {
    I2cError(hal::i2c::Error),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2cError(arg0) => f.debug_tuple("I2cError").field(arg0).finish(),
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

pub(crate) type Result<T> = core::result::Result<T, Error>;
