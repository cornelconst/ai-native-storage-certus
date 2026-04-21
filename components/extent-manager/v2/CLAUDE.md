# v2 Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-04-20

## Active Technologies
- Block device via IBlockDevice (channel-based SPDK), DMA-compatible buffers via DmaAllocFn (001-metadata-manager)

- Rust stable (MSRV 1.75) + component-framework, component-core, component-macros, interfaces (spdk feature), crc32fast (001-metadata-manager)

## Project Structure

```text
src/
tests/
```

## Commands

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
cargo doc --no-deps
```

## Code Style

Rust stable (MSRV 1.75): Follow standard conventions

## Recent Changes
- 001-metadata-manager: Added Rust stable (MSRV 1.75) + component-framework, component-core, component-macros, interfaces (spdk feature), crc32fas

- 001-metadata-manager: Added Rust stable (MSRV 1.75) + component-framework, component-core, component-macros, interfaces (spdk feature), crc32fast

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
