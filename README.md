# load-config

A flexible configuration loader for Rust that supports loading typed configs from files and environment variables, with support for special inline directives like `$path` and `$env`.

`load-config` is designed for configuration composition. You can keep your main config small and reference external files or environment variables directly inside the config tree.

## Features

- Load strongly typed configs with `serde`
- Load config from:
  - files
  - environment variables
- Resolve inline special directives:
  - `$path`
  - `$env`
  - `$text`
- Recursive config composition
- Relative `$path` resolution based on the current file location
- Supports multiple input formats through feature flags

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
load-config = "0.2"
serde = { version = "1", features = ["derive"] }
```

If you want to control enabled formats, you can disable default features and opt in explicitly:

```toml
[dependencies]
load-config = { version = "0.2", default-features = false, features = ["json", "toml", "yaml"] }
serde = { version = "1", features = ["derive"] }
```

## Quick Start

### Load from a file

```rust
use load_config::Loader;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    name: String,
}

fn main() -> Result<(), load_config::Error> {
    let config = Loader::load_path::<Config>("examples/config.json")?;
    println!("{config:?}");
    Ok(())
}
```

If `examples/config.json` contains:

```json
{
  "name": "John Doe"
}
```

then it will deserialize into:

```rust
Config {
    name: "John Doe".into(),
}
```

### Load from an environment variable

```rust
use load_config::Loader;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    name: String,
}

fn main() -> Result<(), load_config::Error> {
    unsafe {
        std::env::set_var("APP_CONFIG", "{\"name\": \"John Doe\"}");
    }

    let config = Loader::load_env::<Config>("APP_CONFIG")?;
    println!("{config:?}");
    Ok(())
}
```

## Special Directives

### `$path`

Use `$path` to replace the current node with the contents of another config file.

Input:

```json
{
  "config": {
    "$path": "config.toml"
  }
}
```

If `config.toml` contains:

```toml
name = "John Doe"
```

the final resolved structure becomes:

```json
{
  "config": {
    "name": "John Doe"
  }
}
```

### `$env`

Use `$env` to replace the current node with the contents of an environment variable.

Input:

```json
{
  "port": {
    "$env": "APP_PORT"
  }
}
```

If:

```text
APP_PORT=3306
```

then the resolved result can deserialize as:

```json
{
  "port": 3306
}
```

### `$text`

Use `$text` to replace the current node with the contents of a UTF-8 text file as a string. The file extension does not matter, so it works for `.pem`, `.crt`, `.key`, or files with no extension.

Input:

```json
{
  "tls": {
    "cert": { "$text": "certs/server.pem" },
    "key": { "$text": "certs/server.key" }
  }
}
```

`cert` and `key` can then deserialize into `String` fields.

- Relative paths are resolved the same way as `$path`
- The content is kept exactly as it is, including trailing newlines and line endings
- The content is not parsed: a file containing `3306` stays the string `"3306"`, and directives inside it are not resolved
- Files that are not valid UTF-8 return an error

### Nested Composition

Special directives can be nested and are resolved recursively.

For example:

```toml
[database]
"$path" = "database.json"
```

If `database.json` contains:

```json
{
  "host": "127.0.0.1",
  "port": 3306,
  "username": "mysql",
  "password": "mysql_password",
  "database": "mysql"
}
```

then the `database` field will be replaced by the contents of `database.json`.

The same also works in YAML:

```yaml
database:
  $path: "database.json"
```

Supported file extensions:

- `.yaml`
- `.yml`

Example:

```yaml
database:
  host: 127.0.0.1
  port: 3306
  enabled: true
```

Notes about YAML support:

- YAML mappings are converted into the crate's internal `Value::Dict`
- YAML sequences are converted into `Value::List`
- YAML integers, booleans, strings, and floats are supported
- Relative `$path` references inside YAML files are resolved relative to the YAML file location
- Complex YAML mapping keys are not supported
- YAML aliases are not supported

## Public API

The main entry point is `Loader`.

### Typed loading

```rust
let config = Loader::load_path::<MyConfig>("config.toml")?;
let config = Loader::load_env::<MyConfig>("APP_CONFIG")?;
```

### Load into an intermediate value tree

```rust
let value = Loader::load_value_from_path("config.toml")?;
let value = Loader::load_value_from_env("APP_CONFIG")?;
```

### Load a plain text file

```rust
let pem: String = Loader::load_text("certs/server.pem")?;
```

### Builder-style entry

```rust
let loader = Loader::new();

let config = loader.path("config.toml").load::<MyConfig>()?;
let config = loader.env("APP_CONFIG").load::<MyConfig>()?;
let pem = loader.text("certs/server.pem").load::<String>()?;
```

## Supported Formats

The crate currently exposes feature flags for:

- `json`
- `json5`
- `toml`
- `yaml`
- `preserve_order`

Default features enable all of the above format-related flags.

Format status:

- `json`: implemented
- `json5`: implemented
- `toml`: implemented
- `yaml`: implemented

## Error Handling

Most APIs return:

```rust
Result<T, load_config::Error>
```

Common errors include:

- file not found
- unsupported extension
- parse failures
- invalid special directive shape
- missing environment variables
- type conversion failures during deserialization
- unsupported YAML constructs such as complex mapping keys or aliases

## Relative Path Resolution

When using `$path` inside a loaded file, relative paths are resolved relative to the file that contains the directive, not the process working directory.

For example:

- `tests/config.toml` contains `$path = "database.json"`
- it resolves to `tests/database.json`

The same rule applies to YAML files.

This makes configuration composition predictable and portable.

## Example

Given:

`config.toml`

```toml
[database]
"$path" = "database.json"
```

`database.json`

```json
{
  "host": "127.0.0.1",
  "port": 3306,
  "username": "mysql",
  "password": "mysql_password",
  "database": "mysql"
}
```

and:

```rust
use load_config::Loader;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    database: Database,
}

#[derive(Debug, Deserialize)]
struct Database {
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
}

fn main() -> Result<(), load_config::Error> {
    let config = Loader::load_path::<Config>("config.toml")?;
    println!("{config:?}");
    Ok(())
}
```

the `database` field will be loaded from `database.json`.

A YAML version of the same config also works:

```yaml
database:
  $path: "database.json"
```

## Feature Notes

This crate is still evolving. Before publishing broadly, you should verify that the feature flags match the current implementation in your release:

- `json` is implemented
- `json5` is implemented
- `toml` is implemented
- `yaml` is implemented

If you are preparing a crates.io release, make sure the README and crate metadata stay in sync with actual functionality.

## License

[MIT](LICENSE)
