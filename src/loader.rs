use std::path::Path;

use serde::de::DeserializeOwned;

use crate::{Error, special::Special, value::Value};

#[derive(Debug, Default, Clone, Copy)]
pub struct Loader;

impl Loader {
    pub const fn new() -> Self {
        Self
    }

    pub fn load_path<T>(path: impl AsRef<Path>) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        Self::new().path(&path).load()
    }

    pub fn load_env<T>(name: impl AsRef<str>) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        Self::new().env(&name).load()
    }

    pub fn load_text(path: impl AsRef<Path>) -> Result<String, Error> {
        Self::new().text(&path).load()
    }

    pub fn load_value_from_path(path: impl AsRef<Path>) -> Result<Value, Error> {
        Self::new().path(&path).load_value()
    }

    pub fn load_value_from_env(name: impl AsRef<str>) -> Result<Value, Error> {
        Self::new().env(&name).load_value()
    }

    pub fn path<'a>(&self, path: &'a impl AsRef<Path>) -> Source<'a> {
        Source {
            special: Special::Path(path.as_ref()),
        }
    }

    pub fn env<'a>(&self, name: &'a impl AsRef<str>) -> Source<'a> {
        Source {
            special: Special::new_env(name.as_ref()),
        }
    }

    pub fn text<'a>(&self, path: &'a impl AsRef<Path>) -> Source<'a> {
        Source {
            special: Special::Text(path.as_ref()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source<'a> {
    special: Special<'a>,
}

impl<'a> Source<'a> {
    pub fn load<T>(&self) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        let value = self.load_value()?;
        value.deserialize_into()
    }

    pub fn load_value(&self) -> Result<Value, Error> {
        self.special.load_value()
    }
}
