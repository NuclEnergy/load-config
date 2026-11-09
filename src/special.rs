use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, map::Map, value::Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Special<'a> {
    #[serde(rename = "$path")]
    Path(&'a Path),
    #[serde(rename = "$env")]
    Env(&'a str),
    #[serde(rename = "$text")]
    Text(&'a Path),
}

impl<'a> Special<'a> {
    pub(crate) fn new_path(path: &'a str) -> Self {
        Self::Path(Path::new(path))
    }

    pub(crate) fn new_text(path: &'a str) -> Self {
        Self::Text(Path::new(path))
    }

    pub(crate) fn new_env(env: &'a str) -> Self {
        Self::Env(env)
    }

    pub(crate) fn is_special_key(key: &str) -> bool {
        matches!(key, "$path" | "$env" | "$text")
    }

    pub(crate) fn load_value(&self) -> Result<Value, Error> {
        self.load_value_with_base(None)
    }

    pub(crate) fn load_value_with_base(&self, base_dir: Option<&Path>) -> Result<Value, Error> {
        match self {
            Self::Path(path) => load_value_from_path(path, base_dir),
            Self::Env(name) => load_value_from_env(name),
            Self::Text(path) => load_text_from_path(path, base_dir),
        }
    }
}

fn load_value_from_env(name: &str) -> Result<Value, Error> {
    let value = std::env::var(name).map_err(|err| {
        Error::Message(format!("failed to read environment variable {name}: {err}"))
    })?;

    parse_string_like_value(&value)?.resolve_specials(None)
}

/// Text content is a leaf: it is neither parsed nor scanned for directives.
fn load_text_from_path(path: &Path, base_dir: Option<&Path>) -> Result<Value, Error> {
    let (_, content) = read_file(path, base_dir)?;
    Ok(Value::String(content))
}

fn read_file(path: &Path, base_dir: Option<&Path>) -> Result<(PathBuf, String), Error> {
    let resolved = resolve_path(path, base_dir);
    let content = std::fs::read_to_string(&resolved)
        .map_err(|err| Error::Message(format!("failed to read {}: {err}", resolved.display())))?;

    Ok((resolved, content))
}

fn load_value_from_path(path: &Path, base_dir: Option<&Path>) -> Result<Value, Error> {
    let (resolved, content) = read_file(path, base_dir)?;

    let ext = resolved
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);

    let current_base_dir = resolved.parent();

    let value: Value = match ext.as_deref() {
        Some("json") => {
            #[cfg(feature = "json")]
            {
                serde_json::from_str::<Value>(&content).map_err(|err| {
                    Error::Message(format!(
                        "failed to parse JSON from {}: {err}",
                        resolved.display()
                    ))
                })?
            }
            #[cfg(not(feature = "json"))]
            {
                return Err(Error::Message(format!(
                    "JSON support is not enabled, cannot load {}",
                    resolved.display()
                )));
            }
        }
        Some("json5") => {
            #[cfg(feature = "json5")]
            {
                json5::from_str::<Value>(&content).map_err(|err| {
                    Error::Message(format!(
                        "failed to parse JSON5 from {}: {err}",
                        resolved.display()
                    ))
                })?
            }
            #[cfg(not(feature = "json5"))]
            {
                return Err(Error::Message(format!(
                    "JSON5 support is not enabled, cannot load {}",
                    resolved.display()
                )));
            }
        }
        Some("toml") => {
            #[cfg(feature = "toml")]
            {
                toml::from_str::<Value>(&content).map_err(|err| {
                    Error::Message(format!(
                        "failed to parse TOML from {}: {err}",
                        resolved.display()
                    ))
                })?
            }
            #[cfg(not(feature = "toml"))]
            {
                return Err(Error::Message(format!(
                    "TOML support is not enabled, cannot load {}",
                    resolved.display()
                )));
            }
        }
        Some("yaml") | Some("yml") => {
            #[cfg(feature = "yaml")]
            {
                load_yaml_value(&content, &resolved)?
            }
            #[cfg(not(feature = "yaml"))]
            {
                return Err(Error::Message(format!(
                    "YAML loading is not enabled, cannot load {}",
                    resolved.display()
                )));
            }
        }
        Some(ext) => {
            return Err(Error::Message(format!(
                "unsupported file extension .{ext} for {}; use $text to load it as plain text",
                resolved.display()
            )));
        }
        None => {
            return Err(Error::Message(format!(
                "cannot determine file format for {}; use $text to load it as plain text",
                resolved.display()
            )));
        }
    };

    value.resolve_specials(current_base_dir)
}

fn resolve_path(path: &Path, base_dir: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base_dir) = base_dir {
        base_dir.join(path)
    } else {
        path.to_path_buf()
    }
}

#[cfg(feature = "json")]
fn parse_string_like_value(value: &str) -> Result<Value, Error> {
    if let Ok(parsed) = serde_json::from_str::<Value>(value) {
        return Ok(parsed);
    }

    let quoted = format!("{value:?}");
    serde_json::from_str::<Value>(&quoted)
        .map_err(|err| Error::Message(format!("failed to deserialize special value: {err}")))
}

/// Without JSON support an environment variable can only be taken verbatim.
#[cfg(not(feature = "json"))]
fn parse_string_like_value(value: &str) -> Result<Value, Error> {
    Ok(Value::String(value.to_owned()))
}

#[cfg(feature = "yaml")]
fn load_yaml_value(content: &str, path: &Path) -> Result<Value, Error> {
    let docs = yaml_rust2::YamlLoader::load_from_str(content).map_err(|err| {
        Error::Message(format!(
            "failed to parse YAML from {}: {err}",
            path.display()
        ))
    })?;

    let Some(doc) = docs.into_iter().next() else {
        return Ok(Value::None);
    };

    yaml_to_value(doc).map_err(|err| {
        Error::Message(format!(
            "failed to convert YAML from {}: {err}",
            path.display()
        ))
    })
}

#[cfg(feature = "yaml")]
fn yaml_to_value(yaml: yaml_rust2::Yaml) -> Result<Value, String> {
    match yaml {
        yaml_rust2::Yaml::Null | yaml_rust2::Yaml::BadValue => Ok(Value::None),
        yaml_rust2::Yaml::Boolean(value) => Ok(Value::Bool(value)),
        yaml_rust2::Yaml::Integer(value) => {
            if value >= 0 {
                Ok(Value::U64(value as u64))
            } else {
                Ok(Value::I64(value))
            }
        }
        yaml_rust2::Yaml::Real(value) => {
            let parsed = value
                .parse::<f64>()
                .map_err(|err| format!("invalid YAML float {value:?}: {err}"))?;
            Ok(Value::Float(parsed))
        }
        yaml_rust2::Yaml::String(value) => Ok(Value::String(value)),
        yaml_rust2::Yaml::Array(values) => values
            .into_iter()
            .map(yaml_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        yaml_rust2::Yaml::Hash(map) => {
            let mut values = Map::default();

            for (key, value) in map {
                let key = yaml_key_to_string(key)?;
                let value = yaml_to_value(value)?;
                values.insert(key, value);
            }

            Ok(Value::Dict(values))
        }
        yaml_rust2::Yaml::Alias(alias) => Err(format!("YAML alias {alias} is not supported")),
    }
}

#[cfg(feature = "yaml")]
fn yaml_key_to_string(key: yaml_rust2::Yaml) -> Result<String, String> {
    match key {
        yaml_rust2::Yaml::String(value) => Ok(value),
        yaml_rust2::Yaml::Integer(value) => Ok(value.to_string()),
        yaml_rust2::Yaml::Real(value) => Ok(value),
        yaml_rust2::Yaml::Boolean(value) => Ok(value.to_string()),
        yaml_rust2::Yaml::Null => Ok("null".to_string()),
        yaml_rust2::Yaml::BadValue => Err("invalid YAML mapping key".to_string()),
        yaml_rust2::Yaml::Alias(alias) => Err(format!(
            "YAML alias {alias} is not supported as a mapping key"
        )),
        yaml_rust2::Yaml::Array(_) | yaml_rust2::Yaml::Hash(_) => {
            Err("complex YAML mapping keys are not supported".to_string())
        }
    }
}
