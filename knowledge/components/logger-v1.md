# logger (v1)

**Crate**: `logger`
**Path**: `components/logger/v1/`
**Version**: 0.1.0

## Description

Production logger component. Writes timestamped, level-filtered log lines to stderr (default) or a file. Timestamps are ISO 8601 UTC with milliseconds. Log level is controlled by the `RUST_LOG` environment variable. Color output is auto-detected via `libc::isatty`.

## Component Definition

```
LoggerComponentV1 {
    version: "0.1.0",
    provides: [ILogger],
    fields: { state: LoggerState },
}
```

## Interfaces Provided

| Interface | Methods |
|-----------|---------|
| `ILogger` | `error(&self, msg: &str)` -- log at ERROR level |
|           | `warn(&self, msg: &str)` -- log at WARN level |
|           | `info(&self, msg: &str)` -- log at INFO level |
|           | `debug(&self, msg: &str)` -- log at DEBUG level |

## Receptacles

None.

## Constructors

- `new_default()` -- stderr output, auto-color detection, level from `RUST_LOG` env
- `new_with_file(path)` -- file output, no color, level from `RUST_LOG` env
- `new_with_writer(writer, level, color)` -- custom writer, explicit level and color

## Log Levels

`Error` (0) > `Warn` (1) > `Info` (2) > `Debug` (3). Parsed from `RUST_LOG` env var.
