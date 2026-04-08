//! Example logger component for the Certus component framework.
//!
//! Provides [`ILogger`], a component interface for logging, and
//! [`ConsoleLogHandler`], an actor handler that writes log messages to stderr.
//!
//! # Quick start
//!
//! ```
//! use example_logger::{ConsoleLogHandler, ConsoleLogRequest, ILogger, LogLevel, LoggerComponent};
//! use component_framework::actor::Actor;
//! use component_framework::iunknown::query;
//!
//! // Create logger component and query the ILogger interface.
//! let comp = LoggerComponent::new();
//! let ilogger = query::<dyn ILogger + Send + Sync>(&*comp).unwrap();
//! assert_eq!(ilogger.name(), "console-logger");
//!
//! // Start a logger actor for async log delivery.
//! let actor = Actor::simple(ConsoleLogHandler::new());
//! let handle = actor.activate().unwrap();
//! handle.send(ConsoleLogRequest {
//!     level: LogLevel::Info,
//!     source: "app".into(),
//!     text: "hello".into(),
//! }).unwrap();
//! handle.deactivate().unwrap();
//! ```

// Note: #![deny(missing_docs)] cannot be used because define_interface!/define_component!
// macros generate items without doc comments. Documentation is provided via module-level docs.

use component_framework::actor::ActorHandler;
use component_framework::{define_component, define_interface};
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

pub use component_core::log::LogLevel;

define_interface! {
    pub ILogger {
        /// Returns the name of this logger.
        fn name(&self) -> &str;
    }
}

define_component! {
    pub LoggerComponent {
        version: "0.1.0",
        provides: [ILogger],
    }
}

impl ILogger for LoggerComponent {
    fn name(&self) -> &str {
        "console-logger"
    }
}

/// Log request message sent to a [`ConsoleLogHandler`] actor.
///
/// # Examples
///
/// ```
/// use example_logger::{ConsoleLogRequest, LogLevel};
///
/// let req = ConsoleLogRequest {
///     level: LogLevel::Info,
///     source: "myapp".into(),
///     text: "started".into(),
/// };
/// assert_eq!(req.source, "myapp");
/// ```
#[derive(Debug, Clone)]
pub struct ConsoleLogRequest {
    /// Severity level.
    pub level: LogLevel,
    /// Source component or subsystem name.
    pub source: String,
    /// Log message text.
    pub text: String,
}

/// Actor handler that writes [`ConsoleLogRequest`] messages to stderr.
///
/// Each line is formatted as:
/// ```text
/// 2026-04-07T12:00:00.000Z [INFO ] [source] message
/// ```
///
/// # Examples
///
/// ```
/// use example_logger::{ConsoleLogHandler, ConsoleLogRequest, LogLevel};
/// use component_framework::actor::Actor;
///
/// let actor = Actor::simple(ConsoleLogHandler::new());
/// let handle = actor.activate().unwrap();
/// handle.send(ConsoleLogRequest {
///     level: LogLevel::Info,
///     source: "test".into(),
///     text: "hello".into(),
/// }).unwrap();
/// handle.deactivate().unwrap();
/// ```
pub struct ConsoleLogHandler;

impl ConsoleLogHandler {
    /// Create a new console log handler.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsoleLogHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorHandler<ConsoleLogRequest> for ConsoleLogHandler {
    fn handle(&mut self, msg: ConsoleLogRequest) {
        let timestamp = format_timestamp(SystemTime::now());
        let _ = writeln!(
            io::stderr(),
            "{} [{}] [{}] {}",
            timestamp,
            msg.level,
            msg.source,
            msg.text
        );
    }
}

/// Format a `SystemTime` as `YYYY-MM-DDTHH:MM:SS.mmmZ`.
fn format_timestamp(time: SystemTime) -> String {
    let dur = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = dur.as_secs();
    let millis = dur.subsec_millis();

    let days = total_secs / 86400;
    let day_secs = total_secs % 86400;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y, m, d, hours, minutes, seconds, millis
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use component_framework::iunknown::{query, IUnknown};

    #[test]
    fn logger_component_provides_ilogger() {
        let comp = LoggerComponent::new();
        let ilogger = query::<dyn ILogger + Send + Sync>(&*comp).unwrap();
        assert_eq!(ilogger.name(), "console-logger");
    }

    #[test]
    fn logger_component_version() {
        let comp = LoggerComponent::new();
        assert_eq!(comp.version(), "0.1.0");
    }

    #[test]
    fn console_log_request_debug() {
        let req = ConsoleLogRequest {
            level: LogLevel::Info,
            source: "test".into(),
            text: "hello".into(),
        };
        let dbg = format!("{:?}", req);
        assert!(dbg.contains("Info"));
        assert!(dbg.contains("test"));
    }
}
