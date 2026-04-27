//! DispatchMap component for the Certus storage system.
//!
//! Provides the `IDispatchMap` interface with receptacles for `ILogger`
//! and `IExtentManager`.
//!
//! # Quick start
//!
//! ```
//! use dispatch_map::DispatchMapComponentV0;
//! use interfaces::IDispatchMap;
//! use component_core::query_interface;
//!
//! let component = DispatchMapComponentV0::new();
//! let dm = query_interface!(component, IDispatchMap);
//! assert!(dm.is_some());
//! ```

use component_framework::define_component;
use interfaces::IDispatchMap;
use interfaces::IExtentManager;
use interfaces::ILogger;

define_component! {
    pub DispatchMapComponentV0 {
        version: "0.1.0",
        provides: [IDispatchMap],
        receptacles: {
            logger: ILogger,
            extent_manager: IExtentManager,
        },
    }
}

impl IDispatchMap for DispatchMapComponentV0 {
    fn dispatch(&self, _key: &str) -> Result<(), String> {
        todo!("DispatchMap::dispatch not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use component_core::query_interface;

    #[test]
    fn test_query_idispatch_map() {
        let component = DispatchMapComponentV0::new();
        let iface = query_interface!(component, IDispatchMap);
        assert!(iface.is_some());
    }
}
