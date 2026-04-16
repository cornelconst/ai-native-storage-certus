//! Interface for the extent-manager component and shared error type.
//
// This file defines `ExtentManagerError` (used across the workspace) and
// the `IExtentManager` trait exposed to callers.

use component_macros::define_interface;
use std::fmt;

/// Errors returned by `IExtentManager` operations.
#[derive(Debug, Clone)]
pub enum ExtentManagerError {
    CorruptMetadata(String),
    DuplicateKey(u64),
    InvalidSizeClass(u32),
    IoError(String),
    KeyNotFound(u64),
    NotInitialized(String),
    OutOfSpace { size_class: u32 },
}

impl fmt::Display for ExtentManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CorruptMetadata(msg) => write!(f, "corrupt metadata: {msg}"),
            Self::DuplicateKey(k) => write!(f, "duplicate key: {k}"),
            Self::InvalidSizeClass(c) => write!(f, "invalid size class: {c}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::KeyNotFound(k) => write!(f, "key not found: {k}"),
            Self::NotInitialized(msg) => write!(f, "not initialized: {msg}"),
            Self::OutOfSpace { size_class } => {
                write!(f, "out of space for size class: {size_class}")
            }
        }
    }
}

impl std::error::Error for ExtentManagerError {}

#[cfg(feature = "spdk")]
impl From<crate::iblock_device::NvmeBlockError> for ExtentManagerError {
    fn from(e: crate::iblock_device::NvmeBlockError) -> Self {
        Self::IoError(e.to_string())
    }
}

// The IExtentManager interface exposed to users of the extent manager
define_interface! {
    pub IExtentManager {
        fn create_extent(
            &self,
            key: u64,
            size_class: u32,
            filename: &str,
            data_crc: u32,
            has_crc: bool,
        ) -> Result<Vec<u8>, ExtentManagerError>;

        fn remove_extent(&self, key: u64) -> Result<(), ExtentManagerError>;

        fn lookup_extent(&self, key: u64) -> Result<Vec<u8>, ExtentManagerError>;

        fn extent_count(&self) -> u64;
    }
}
