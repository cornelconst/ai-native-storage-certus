//! Error types for the SPDK environment component.

use std::fmt;

/// Error conditions reported by the SPDK environment component.
///
/// Each variant carries a descriptive message with actionable guidance
/// to help the user resolve the issue.
#[derive(Debug, Clone)]
pub enum SpdkEnvError {
    /// VFIO is not available: `/dev/vfio` not found or `vfio-pci` module not loaded.
    VfioNotAvailable(String),
    /// Insufficient permissions on a specific VFIO path.
    PermissionDenied(String),
    /// No hugepages configured for DPDK.
    HugepagesNotConfigured(String),
    /// Logging receptacle not connected before `init()`.
    LoggerNotConnected(String),
    /// Another SPDK environment instance is already active in this process.
    AlreadyInitialized(String),
    /// SPDK/DPDK environment initialization failed.
    InitFailed(String),
    /// PCI device enumeration failed after environment was initialized.
    DeviceProbeFailed(String),
}

impl fmt::Display for SpdkEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpdkEnvError::VfioNotAvailable(msg) => write!(f, "VFIO not available: {msg}"),
            SpdkEnvError::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            SpdkEnvError::HugepagesNotConfigured(msg) => {
                write!(f, "Hugepages not configured: {msg}")
            }
            SpdkEnvError::LoggerNotConnected(msg) => write!(f, "Logger not connected: {msg}"),
            SpdkEnvError::AlreadyInitialized(msg) => write!(f, "Already initialized: {msg}"),
            SpdkEnvError::InitFailed(msg) => write!(f, "SPDK init failed: {msg}"),
            SpdkEnvError::DeviceProbeFailed(msg) => write!(f, "Device probe failed: {msg}"),
        }
    }
}

impl std::error::Error for SpdkEnvError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_vfio_not_available() {
        let e = SpdkEnvError::VfioNotAvailable("/dev/vfio not found".into());
        assert_eq!(e.to_string(), "VFIO not available: /dev/vfio not found");
    }

    #[test]
    fn error_display_permission_denied() {
        let e = SpdkEnvError::PermissionDenied("/dev/vfio/vfio (need read+write)".into());
        assert_eq!(
            e.to_string(),
            "Permission denied: /dev/vfio/vfio (need read+write)"
        );
    }

    #[test]
    fn error_display_hugepages() {
        let e = SpdkEnvError::HugepagesNotConfigured("no hugepages allocated".into());
        assert_eq!(
            e.to_string(),
            "Hugepages not configured: no hugepages allocated"
        );
    }

    #[test]
    fn error_display_logger() {
        let e = SpdkEnvError::LoggerNotConnected("call comp.logger.connect() first".into());
        assert_eq!(
            e.to_string(),
            "Logger not connected: call comp.logger.connect() first"
        );
    }

    #[test]
    fn error_display_already_initialized() {
        let e = SpdkEnvError::AlreadyInitialized("singleton".into());
        assert_eq!(e.to_string(), "Already initialized: singleton");
    }

    #[test]
    fn error_display_init_failed() {
        let e = SpdkEnvError::InitFailed("spdk_env_init returned -1".into());
        assert_eq!(e.to_string(), "SPDK init failed: spdk_env_init returned -1");
    }

    #[test]
    fn error_display_probe_failed() {
        let e = SpdkEnvError::DeviceProbeFailed("enumeration error".into());
        assert_eq!(e.to_string(), "Device probe failed: enumeration error");
    }

    #[test]
    fn error_is_std_error() {
        let e: Box<dyn std::error::Error> = Box::new(SpdkEnvError::InitFailed("test".into()));
        assert!(e.to_string().contains("test"));
    }

    #[test]
    fn error_clone() {
        let e = SpdkEnvError::InitFailed("clone test".into());
        let e2 = e.clone();
        assert_eq!(e.to_string(), e2.to_string());
    }

    #[test]
    fn error_debug() {
        let e = SpdkEnvError::VfioNotAvailable("debug test".into());
        let dbg = format!("{:?}", e);
        assert!(dbg.contains("VfioNotAvailable"));
        assert!(dbg.contains("debug test"));
    }

    #[test]
    fn error_all_variants_are_std_error() {
        let variants: Vec<Box<dyn std::error::Error>> = vec![
            Box::new(SpdkEnvError::VfioNotAvailable("a".into())),
            Box::new(SpdkEnvError::PermissionDenied("b".into())),
            Box::new(SpdkEnvError::HugepagesNotConfigured("c".into())),
            Box::new(SpdkEnvError::LoggerNotConnected("d".into())),
            Box::new(SpdkEnvError::AlreadyInitialized("e".into())),
            Box::new(SpdkEnvError::InitFailed("f".into())),
            Box::new(SpdkEnvError::DeviceProbeFailed("g".into())),
        ];
        for e in &variants {
            assert!(!e.to_string().is_empty());
        }
    }

    #[test]
    fn error_display_preserves_message() {
        let msg = "detailed error context with special chars: /dev/vfio/42 (uid=1000)";
        let e = SpdkEnvError::PermissionDenied(msg.into());
        assert!(e.to_string().contains(msg));
    }
}
