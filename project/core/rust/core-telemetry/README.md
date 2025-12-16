# TCL (Telemetry Client Library)

## Overview

**TCL** is a cross-platform telemetry library that:

- Collects telemetry events (metrics with dimensions/values), documentation [here](https://gitlab.protontech.ch/data/engineering/telemetry/-/tree/master/definitions)
- Stores them locally in a database
- Syncs them to Proton's backend

## Platform Support

The library targets **4 different platforms**:

- **Native** (Linux/Mac/Windows)
- **WebAssembly** (Web browsers)
- **Android**/**iOS** (via uniffi)

## Features

### Cargo Features

The library uses conditional compilation via cargo features:

#### `sqlite`

- Enables local SQLite database storage
- Includes Sqlite database implementation
- **Used on**: Native platforms (Android, iOS, Desktop)
- **NOT used on**: WASM

#### `http`

- Enables built-in HTTP client (`reqwest`)
- Sends telemetry to Proton's backend
- **Used on**: All platforms

#### `uniffi`

- Enables FFI bindings for Android and iOS
- **Used on**: Android and iOS

## Usage Examples

### Rust (Native) - Built-in HTTP and SQLite

```toml
[dependencies]
tcl = { git = "git+ssh://git@gitlab.protontech.ch/proton/clients/monorepo.git", features = ["http", "sqlite"] }
```

```rust
// Uses built-in HTTP Client + Db
let config = TelemetryConfig {
    storage_path: "/tmp/telemetry.db".to_string(),
    max_storage_size: 1024,
};
let tcl = Tcl::init(config)?;

tcl.log(vec![event]).await?;
tcl.sync(100).await?;
```

### Rust (Native) - Bring-your-own HTTP Client

```toml
[dependencies]
tcl = { git = "git+ssh://git@gitlab.protontech.ch/proton/clients/monorepo.git", features = ["sqlite"] }
```

```rust
// Provide your own HTTP client implementation
struct MyHttpClient { /* ... */ }
impl TelemetryHttpClientEx for MyHttpClient { /* ... */ }

let tcl = Tcl::init(config, Arc::new(MyHttpClient::new()))?;
```

### WASM - Bring-your-own Database

```toml
[dependencies]
tcl = { git = "git+ssh://git@gitlab.protontech.ch/proton/clients/monorepo.git", features = ["http"] }
```

```rust
// Provide IndexedDB wrapper (WASM has no filesystem)
struct IndexedDbWrapper { /* ... */ }
impl TelemetryDbEx for IndexedDbWrapper { /* ... */ }

let tcl = Tcl::init(config, Arc::new(IndexedDbWrapper::new()))?;

tcl.log(vec![event]).await?;
tcl.sync(100).await?;
```

**Note**: WASM has unique constraints:

- Single-threaded (JavaScript event loop)
- No filesystem (can't use SQLite)
- async functions can't be `Send`

### Android/iOS - via UniFFI

```toml
[dependencies]
tcl = { git = "git+ssh://git@gitlab.protontech.ch/proton/clients/monorepo.git", features = ["http", "sqlite", "uniffi"] }
```

```kotlin
val config = TelemetryConfig(
    storagePath = "/data/app/telemetry.db",
    maxStorageSize = 1024u
)
val tcl = Tcl.init(config)
tcl.log(listOf(event))
tcl.sync(10u)
```

## Build Commands Reference

```bash
# Native (desktop) - full features
cargo build --features "sqlite,http"

# WASM (web) - HTTP only, custom DB
cargo build --target wasm32-unknown-unknown --features "http"

# Android/iOS - everything + uniffi
cargo build --features "sqlite,http,uniffi"

# Run tests (native with all features)
cargo test --features "http,sqlite" -- --nocapture
```
