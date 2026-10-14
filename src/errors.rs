use core::error::Error;
use core::fmt;
#[cfg(feature = "std")]
use std::io;

/// Errors returned by this crate.
#[derive(Debug)]
pub enum FcsdError {
    /// The bucket size is zero.
    BucketSizeZero,
    /// The bucket size is not a power of two.
    BucketSizeNotPowerOfTwo,
    /// An input key contains [`crate::END_MARKER`].
    KeyContainsEndMarker,
    /// Input keys are not sorted and unique.
    KeysNotSortedAndUnique,
    /// Serialized data is invalid.
    InvalidData(&'static str),
    /// An I/O operation failed.
    #[cfg(feature = "std")]
    Io(io::Error),
}

impl fmt::Display for FcsdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BucketSizeZero => write!(f, "bucket_size must not be zero"),
            Self::BucketSizeNotPowerOfTwo => {
                write!(f, "bucket_size must be a power of two")
            }
            Self::KeyContainsEndMarker => write!(
                f,
                "the input key must not contain END_MARKER (={})",
                crate::END_MARKER
            ),
            Self::KeysNotSortedAndUnique => {
                write!(f, "input keys must be sorted and unique")
            }
            Self::InvalidData(message) => f.write_str(message),
            #[cfg(feature = "std")]
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl Error for FcsdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            #[cfg(feature = "std")]
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "std")]
impl From<io::Error> for FcsdError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A specialized [`Result`](core::result::Result) for this crate.
pub type Result<T> = core::result::Result<T, FcsdError>;
