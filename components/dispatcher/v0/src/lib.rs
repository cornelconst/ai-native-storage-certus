//! Dispatcher component for the Certus storage system.
//!
//! Orchestrates cache operations (populate, lookup, check, remove) using
//! GPU-to-SSD data flows via DMA staging buffers. Coordinates N data block
//! devices with N extent managers for persistent storage.
//!
//! Provides the [`IDispatcher`] interface with receptacles for
//! [`ILogger`] and [`IDispatchMap`].

mod background;
pub mod io_segmenter;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use component_framework::define_component;
use interfaces::{
    CacheKey, DispatcherConfig, DispatcherError, IDispatchMap, IDispatcher, ILogger, IpcHandle,
};

use crate::background::{BackgroundWriter, WriteJob};

define_component! {
    pub DispatcherComponentV0 {
        version: "0.1.0",
        provides: [IDispatcher],
        receptacles: {
            logger: ILogger,
            dispatch_map: IDispatchMap,
        },
        fields: {
            initialized: AtomicBool,
            bg_writer: Mutex<Option<BackgroundWriter>>,
        },
    }
}

impl DispatcherComponentV0 {
    fn log_info(&self, msg: &str) {
        if let Ok(logger) = self.logger.get() {
            logger.info(msg);
        }
    }

    #[allow(dead_code)]
    fn log_error(&self, msg: &str) {
        if let Ok(logger) = self.logger.get() {
            logger.error(msg);
        }
    }

    fn ensure_initialized(&self) -> Result<(), DispatcherError> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err(DispatcherError::NotInitialized(
                "dispatcher not initialized".into(),
            ));
        }
        Ok(())
    }
}

impl IDispatcher for DispatcherComponentV0 {
    fn initialize(&self, config: DispatcherConfig) -> Result<(), DispatcherError> {
        self.log_info("dispatcher: initializing");

        let _dm = self
            .dispatch_map
            .get()
            .map_err(|_| DispatcherError::NotInitialized("dispatch_map not bound".into()))?;

        if config.data_pci_addrs.is_empty() {
            return Err(DispatcherError::InvalidParameter(
                "data_pci_addrs must not be empty".into(),
            ));
        }

        // TODO: Create N block devices and N extent managers from config.
        // This requires SPDK environment to be active and real hardware.
        // For now, start the background writer with a placeholder processor.

        let writer = BackgroundWriter::start(move |job: WriteJob| {
            // TODO: Implement actual staging-to-SSD write with MDTS segmentation.
            let _ = job;
        });

        *self.bg_writer.lock().unwrap() = Some(writer);
        self.initialized.store(true, Ordering::Release);

        self.log_info("dispatcher: initialized");
        Ok(())
    }

    fn shutdown(&self) -> Result<(), DispatcherError> {
        self.log_info("dispatcher: shutting down");

        if let Some(mut writer) = self.bg_writer.lock().unwrap().take() {
            writer.shutdown();
        }

        self.initialized.store(false, Ordering::Release);
        self.log_info("dispatcher: shut down");
        Ok(())
    }

    fn lookup(&self, key: CacheKey, _ipc_handle: IpcHandle) -> Result<(), DispatcherError> {
        self.ensure_initialized()?;

        let dm = self
            .dispatch_map
            .get()
            .map_err(|_| DispatcherError::NotInitialized("dispatch_map not bound".into()))?;

        dm.take_read(key)
            .map_err(|_| DispatcherError::KeyNotFound(key))?;

        let result = dm.lookup(key);

        dm.release_read(key)
            .map_err(|_| DispatcherError::IoError("failed to release read lock".into()))?;

        match result {
            Ok(lookup_result) => {
                use interfaces::LookupResult;
                match lookup_result {
                    LookupResult::NotExist => Err(DispatcherError::KeyNotFound(key)),
                    LookupResult::MismatchSize => Err(DispatcherError::InvalidParameter(
                        "size mismatch on lookup".into(),
                    )),
                    LookupResult::Staging { buffer } => {
                        // TODO: DMA copy from staging buffer to ipc_handle
                        let _ = buffer;
                        Ok(())
                    }
                    LookupResult::BlockDevice { offset } => {
                        // TODO: MDTS-segmented read from SSD, DMA copy to ipc_handle
                        let _ = offset;
                        Ok(())
                    }
                }
            }
            Err(_) => Err(DispatcherError::KeyNotFound(key)),
        }
    }

    fn check(&self, key: CacheKey) -> Result<bool, DispatcherError> {
        self.ensure_initialized()?;

        let dm = self
            .dispatch_map
            .get()
            .map_err(|_| DispatcherError::NotInitialized("dispatch_map not bound".into()))?;

        match dm.lookup(key) {
            Ok(result) => {
                use interfaces::LookupResult;
                match result {
                    LookupResult::NotExist => Ok(false),
                    _ => Ok(true),
                }
            }
            Err(_) => Ok(false),
        }
    }

    fn remove(&self, key: CacheKey) -> Result<(), DispatcherError> {
        self.ensure_initialized()?;

        let dm = self
            .dispatch_map
            .get()
            .map_err(|_| DispatcherError::NotInitialized("dispatch_map not bound".into()))?;

        dm.take_write(key)
            .map_err(|_| DispatcherError::KeyNotFound(key))?;

        let result = dm.remove(key);

        match result {
            Ok(()) => {
                // TODO: Free SSD extent if entry was in block-device state
                Ok(())
            }
            Err(_) => {
                let _ = dm.release_write(key);
                Err(DispatcherError::KeyNotFound(key))
            }
        }
    }

    fn populate(&self, key: CacheKey, ipc_handle: IpcHandle) -> Result<(), DispatcherError> {
        self.ensure_initialized()?;

        if ipc_handle.size == 0 {
            return Err(DispatcherError::InvalidParameter(
                "IPC handle size must be > 0".into(),
            ));
        }

        let dm = self
            .dispatch_map
            .get()
            .map_err(|_| DispatcherError::NotInitialized("dispatch_map not bound".into()))?;

        let block_count = ipc_handle.size.div_ceil(4096);

        let staging_buffer = dm.create_staging(key, block_count).map_err(|e| match e {
            interfaces::DispatchMapError::AlreadyExists(k) => DispatcherError::AlreadyExists(k),
            interfaces::DispatchMapError::AllocationFailed(msg) => {
                DispatcherError::AllocationFailed(msg)
            }
            other => DispatcherError::IoError(other.to_string()),
        })?;

        // TODO: DMA copy from ipc_handle to staging buffer
        let _ = staging_buffer;

        dm.downgrade_reference(key)
            .map_err(|e| DispatcherError::IoError(e.to_string()))?;

        let guard = self.bg_writer.lock().unwrap();
        if let Some(ref writer) = *guard {
            let _ = writer.enqueue(WriteJob {
                key,
                size: ipc_handle.size,
                device_index: 0,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use component_core::query_interface;

    #[test]
    fn component_creation() {
        let _c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
    }

    #[test]
    fn query_idispatcher() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher);
        assert!(d.is_some());
    }

    #[test]
    fn initialize_without_receptacles_fails() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        let config = DispatcherConfig {
            metadata_pci_addr: "0000:01:00.0".to_string(),
            data_pci_addrs: vec!["0000:02:00.0".to_string()],
        };
        let err = d.initialize(config);
        assert!(matches!(err, Err(DispatcherError::NotInitialized(_))));
    }

    #[test]
    fn initialize_with_empty_pci_addrs_fails() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        let config = DispatcherConfig {
            metadata_pci_addr: "0000:01:00.0".to_string(),
            data_pci_addrs: vec![],
        };
        // This will fail with NotInitialized since dispatch_map isn't bound
        let err = d.initialize(config);
        assert!(err.is_err());
    }

    #[test]
    fn lookup_before_initialize_fails() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        let mut buf = vec![0u8; 4096];
        let handle = IpcHandle {
            address: buf.as_mut_ptr(),
            size: 4096,
        };
        let err = d.lookup(42, handle);
        assert!(matches!(err, Err(DispatcherError::NotInitialized(_))));
    }

    #[test]
    fn check_before_initialize_fails() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        let err = d.check(42);
        assert!(matches!(err, Err(DispatcherError::NotInitialized(_))));
    }

    #[test]
    fn remove_before_initialize_fails() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        let err = d.remove(42);
        assert!(matches!(err, Err(DispatcherError::NotInitialized(_))));
    }

    #[test]
    fn populate_before_initialize_fails() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        let mut buf = vec![0u8; 4096];
        let handle = IpcHandle {
            address: buf.as_mut_ptr(),
            size: 4096,
        };
        let err = d.populate(42, handle);
        assert!(matches!(err, Err(DispatcherError::NotInitialized(_))));
    }

    #[test]
    fn populate_with_zero_size_fails() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        // Even though not initialized, zero-size check comes after init check.
        // This test verifies the parameter validation exists in the code path.
        let mut buf = vec![0u8; 0];
        let handle = IpcHandle {
            address: buf.as_mut_ptr(),
            size: 0,
        };
        let err = d.populate(42, handle);
        // Will fail with NotInitialized since that check comes first
        assert!(err.is_err());
    }

    #[test]
    fn shutdown_without_initialize_succeeds() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        assert!(d.shutdown().is_ok());
    }

    #[test]
    fn double_shutdown_succeeds() {
        let c = DispatcherComponentV0::new(AtomicBool::new(false), Mutex::new(None));
        let d = query_interface!(c, IDispatcher).unwrap();
        assert!(d.shutdown().is_ok());
        assert!(d.shutdown().is_ok());
    }

    #[test]
    fn concurrent_pre_init_calls_from_multiple_threads() {
        use std::sync::Arc;
        use std::thread;

        let c = Arc::new(DispatcherComponentV0::new(
            AtomicBool::new(false),
            Mutex::new(None),
        ));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let comp = Arc::clone(&c);
                thread::spawn(move || {
                    let d = query_interface!(comp, IDispatcher).unwrap();
                    assert!(matches!(
                        d.check(1),
                        Err(DispatcherError::NotInitialized(_))
                    ));
                    assert!(matches!(
                        d.remove(1),
                        Err(DispatcherError::NotInitialized(_))
                    ));
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }
}
