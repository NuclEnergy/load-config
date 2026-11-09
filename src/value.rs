use std::path::Path;

use serde::de::DeserializeOwned;
use serde_core::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::{Error, map::Map, special::Special};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    #[default]
    None,
    Bool(bool),
    I64(i64),
    I128(i128),
    U64(u64),
    U128(u128),
    Float(f64),
    String(String),
    Dict(Dict),
    List(List),
}

pub(crate) type List = Vec<Value>;
pub(crate) type Dict = Map<String, Value>;

macro_rules! impl_from {
    ($variant:ident => $($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for Value {
                fn from(value: $ty) -> Self {
                    Self::$variant(value.into())
                }
            }
        )+
    };
}

macro_rules! impl_try_from_via {
    ($target:ty, $via:ty) => {
        impl TryFrom<Value> for $target {
            type Error = Error;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                let intermediate: $via = value.try_into()?;
                intermediate.try_into().map_err(|_| {
                    Error::Message(format!(
                        "invalid type for {}: out of range",
                        stringify!($target)
                    ))
                })
            }
        }
    };
}

impl_from!(Bool => bool);
impl_from!(I64 => i8, i16, i32, i64);
impl_from!(I128 => i128);
impl_from!(U64 => u8, u16, u32, u64);
impl_from!(U128 => u128);

impl From<isize> for Value {
    fn from(value: isize) -> Self {
        Self::I128(value as i128)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Self::U128(value as u128)
    }
}
impl_from!(Float => f32, f64);
impl_from!(String => String);

impl<'a> From<&'a str> for Value {
    fn from(value: &'a str) -> Self {
        Self::String(value.into())
    }
}

impl<T> From<Option<T>> for Value
where
    T: Into<Self>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Self::None,
        }
    }
}

impl<T> From<Map<String, T>> for Value
where
    T: Into<Value>,
{
    fn from(values: Map<String, T>) -> Self {
        let values = values.into_iter().map(|(k, v)| (k, v.into())).collect();
        Self::Dict(values)
    }
}

impl<T> From<Vec<T>> for Value
where
    T: Into<Value>,
{
    fn from(values: Vec<T>) -> Self {
        Self::List(values.into_iter().map(T::into).collect())
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;

        match self {
            Self::None => write!(f, "None"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "{value}"),
            Self::I64(value) => write!(f, "{value}"),
            Self::U64(value) => write!(f, "{value}"),
            Self::I128(value) => write!(f, "{value}"),
            Self::U128(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
            Self::Dict(dict) => {
                let mut s = String::new();
                for (k, v) in dict.iter() {
                    write!(s, "{k}: {v}, ")?;
                }
                write!(f, "{{ {s} }}")
            }
            Self::List(list) => {
                let mut s = String::new();
                for e in list.iter() {
                    write!(s, "{e}, ")?;
                }
                write!(f, "[{s:?}]")
            }
        }
    }
}

impl Value {
    pub fn deserialize_into<T>(&self) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        T::deserialize(self.clone())
    }

    pub fn resolve_specials(self, base_dir: Option<&Path>) -> Result<Self, Error> {
        match self {
            Self::Dict(dict) => Self::resolve_dict_specials(dict, base_dir),
            Self::List(list) => {
                let mut resolved = Vec::with_capacity(list.len());
                for value in list {
                    resolved.push(value.resolve_specials(base_dir)?);
                }
                Ok(Self::List(resolved))
            }
            value => Ok(value),
        }
    }

    fn resolve_dict_specials(dict: Dict, base_dir: Option<&Path>) -> Result<Self, Error> {
        if dict.len() == 1 {
            let mut iter = dict.into_iter();
            let (key, value) = iter.next().expect("dict length checked above");

            if Special::is_special_key(&key) {
                let Value::String(raw) = value else {
                    return Err(Error::Message(format!(
                        "special member {key} expects a string value"
                    )));
                };

                return match key.as_str() {
                    "$path" => Special::new_path(&raw).load_value_with_base(base_dir),
                    "$env" => Special::new_env(&raw).load_value(),
                    "$text" => Special::new_text(&raw).load_value_with_base(base_dir),
                    _ => unreachable!("checked by is_special_key"),
                };
            }

            let mut resolved = Map::default();
            resolved.insert(key, value.resolve_specials(base_dir)?);
            return Ok(Self::Dict(resolved));
        }

        let mut resolved = Map::default();
        for (key, value) in dict {
            resolved.insert(key, value.resolve_specials(base_dir)?);
        }

        Ok(Self::Dict(resolved))
    }
}

impl<'de> Deserialize<'de> for Value {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("any valid configuration value")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(Value::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(Value::I64(value))
            }

            fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E> {
                Ok(Value::I128(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(Value::U64(value))
            }

            fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E> {
                Ok(Value::U128(value))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
                Ok(Value::Float(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde_core::de::Error,
            {
                Ok(Value::String(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(Value::String(value))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Value::None)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Value::None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                Deserialize::deserialize(deserializer)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();

                while let Some(value) = seq.next_element::<Value>()? {
                    values.push(value);
                }

                Ok(Value::List(values))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = Map::default();

                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    values.insert(key, value);
                }

                Ok(Value::Dict(values))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

impl TryFrom<Value> for bool {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(bool) => Ok(bool),
            Value::I64(i64) => Ok(i64 != 0),
            Value::I128(i128) => Ok(i128 != 0),
            Value::U64(u64) => Ok(u64 != 0),
            Value::U128(u128) => Ok(u128 != 0),
            Value::Float(f64) => Ok(f64 != 0.0),
            Value::String(string) => {
                Err(Error::Message(format!("invalid type for bool: {string}")))
            }
            Value::None => Err(Error::NotFound("bool".to_string())),
            Value::Dict(_) => Err(Error::Message("invalid type for bool: dict".to_string())),
            Value::List(_) => Err(Error::Message("invalid type for bool: list".to_string())),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(string) => Ok(string),
            Value::Bool(bool) => Ok(bool.to_string()),
            Value::I64(i64) => Ok(i64.to_string()),
            Value::I128(i128) => Ok(i128.to_string()),
            Value::U64(u64) => Ok(u64.to_string()),
            Value::U128(u128) => Ok(u128.to_string()),
            Value::Float(f64) => Ok(f64.to_string()),
            Value::None => Err(Error::NotFound("string".to_string())),
            Value::Dict(_) => Err(Error::Message("invalid type for string: map".to_string())),
            Value::List(_) => Err(Error::Message(
                "invalid type for string: sequence".to_string(),
            )),
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(bool) => Ok(if bool { 1.0 } else { 0.0 }),
            Value::I64(i64) => Ok(i64 as f64),
            Value::I128(i128) => Ok(i128 as f64),
            Value::U64(u64) => Ok(u64 as f64),
            Value::U128(u128) => Ok(u128 as f64),
            Value::Float(f64) => Ok(f64),
            Value::String(string) => Err(Error::Message(format!("invalid type for f64: {string}"))),
            Value::None => Err(Error::NotFound("f64".to_string())),
            Value::Dict(_) => Err(Error::Message("invalid type for f64: map".to_string())),
            Value::List(_) => Err(Error::Message("invalid type for f64: sequence".to_string())),
        }
    }
}

impl TryFrom<Value> for i64 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(bool) => Ok(i64::from(bool)),
            Value::I64(i64) => Ok(i64),
            Value::I128(i128) => i128.try_into().map_err(|_| {
                Error::Message("invalid type for i64: a signed 64 bit or less integer".to_string())
            }),
            Value::U64(u64) => u64.try_into().map_err(|_| {
                Error::Message("invalid type for i64: a signed 64 bit or less integer".to_string())
            }),
            Value::U128(u128) => u128.try_into().map_err(|_| {
                Error::Message("invalid type for i64: a signed 64 bit or less integer".to_string())
            }),
            Value::Float(f64) => {
                if !f64.is_finite() {
                    return Err(Error::Message(
                        "invalid type for i64: non-finite float".to_string(),
                    ));
                }
                let rounded = f64.round();
                if rounded < i64::MIN as f64 || rounded > i64::MAX as f64 {
                    return Err(Error::Message(
                        "invalid type for i64: out of range".to_string(),
                    ));
                }
                Ok(rounded as i64)
            }
            Value::String(string) => Err(Error::Message(format!("invalid type for i64: {string}"))),
            Value::None => Err(Error::NotFound("i64".to_string())),
            Value::Dict(_) => Err(Error::Message("invalid type for i64: map".to_string())),
            Value::List(_) => Err(Error::Message("invalid type for i64: sequence".to_string())),
        }
    }
}

impl TryFrom<Value> for i128 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(bool) => Ok(i128::from(bool)),
            Value::I64(i64) => Ok(i64.into()),
            Value::I128(i128) => Ok(i128),
            Value::U64(u64) => Ok(u64.into()),
            Value::U128(u128) => u128.try_into().map_err(|_| {
                Error::Message("invalid type for i128: an unsigned 128 bit integer".to_string())
            }),
            Value::Float(f64) => {
                if !f64.is_finite() {
                    return Err(Error::Message(
                        "invalid type for i128: non-finite float".to_string(),
                    ));
                }
                Ok(f64.round() as i128)
            }
            Value::String(string) => {
                Err(Error::Message(format!("invalid type for i128: {string}")))
            }
            Value::None => Err(Error::NotFound("i128".to_string())),
            Value::Dict(_) => Err(Error::Message("invalid type for i128: map".to_string())),
            Value::List(_) => Err(Error::Message(
                "invalid type for i128: sequence".to_string(),
            )),
        }
    }
}

impl TryFrom<Value> for u64 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(bool) => Ok(bool.into()),
            Value::I64(i64) => i64.try_into().map_err(|_| {
                Error::Message("invalid type for u64: negative signed integer".to_string())
            }),
            Value::I128(i128) => i128.try_into().map_err(|_| {
                Error::Message(
                    "invalid type for u64: negative or too large signed integer".to_string(),
                )
            }),
            Value::U64(u64) => Ok(u64),
            Value::U128(u128) => u128
                .try_into()
                .map_err(|_| Error::Message("invalid type for u64: u128".to_string())),
            Value::Float(f64) => {
                if !f64.is_finite() {
                    return Err(Error::Message(
                        "invalid type for u64: non-finite float".to_string(),
                    ));
                }
                let rounded = f64.round();
                if rounded < 0.0 || rounded > u64::MAX as f64 {
                    return Err(Error::Message(
                        "invalid type for u64: out of range".to_string(),
                    ));
                }
                Ok(rounded as u64)
            }
            Value::String(string) => Err(Error::Message(format!("invalid type for u64: {string}"))),
            Value::None => Err(Error::NotFound("u64".to_string())),
            Value::Dict(_) => Err(Error::Message("invalid type for u64: map".to_string())),
            Value::List(_) => Err(Error::Message("invalid type for u64: sequence".to_string())),
        }
    }
}

impl TryFrom<Value> for u128 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(bool) => Ok(u128::from(bool)),
            Value::I64(i64) => i64.try_into().map_err(|_| {
                Error::Message("invalid type for u128: negative signed integer".to_string())
            }),
            Value::I128(i128) => i128.try_into().map_err(|_| {
                Error::Message("invalid type for u128: negative signed integer".to_string())
            }),
            Value::U64(u64) => Ok(u64.into()),
            Value::U128(u128) => Ok(u128),
            Value::Float(f64) => {
                if !f64.is_finite() {
                    return Err(Error::Message(
                        "invalid type for u128: non-finite float".to_string(),
                    ));
                }
                let rounded = f64.round();
                if rounded < 0.0 || rounded > u128::MAX as f64 {
                    return Err(Error::Message(
                        "invalid type for u128: out of range".to_string(),
                    ));
                }
                Ok(rounded as u128)
            }
            Value::String(string) => {
                Err(Error::Message(format!("invalid type for u128: {string}")))
            }
            Value::None => Err(Error::NotFound("u128".to_string())),
            Value::Dict(_) => Err(Error::Message("invalid type for u128: map".to_string())),
            Value::List(_) => Err(Error::Message(
                "invalid type for u128: sequence".to_string(),
            )),
        }
    }
}

impl TryFrom<Value> for f32 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let float: f64 = value.try_into()?;
        if !float.is_finite() {
            return Err(Error::Message(
                "invalid type for f32: non-finite float".to_string(),
            ));
        }
        if float < f32::MIN as f64 || float > f32::MAX as f64 {
            return Err(Error::Message(
                "invalid type for f32: out of range".to_string(),
            ));
        }
        Ok(float as f32)
    }
}

impl TryFrom<Value> for isize {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let int: i128 = value.try_into()?;
        int.try_into()
            .map_err(|_| Error::Message("invalid type for isize: out of range".to_string()))
    }
}

impl TryFrom<Value> for usize {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let int: u128 = value.try_into()?;
        int.try_into()
            .map_err(|_| Error::Message("invalid type for usize: out of range".to_string()))
    }
}

impl_try_from_via!(i8, i64);
impl_try_from_via!(i16, i64);
impl_try_from_via!(i32, i64);
impl_try_from_via!(u8, u64);
impl_try_from_via!(u16, u64);
impl_try_from_via!(u32, u64);

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn deserialize_value_keeps_plain_map_before_resolution() {
        let json = json!({
            "config": {
                "$path": "tests/database.json"
            }
        });

        let value: Value = serde_json::from_value(json).unwrap();

        let expected = Value::Dict(Map::from([(
            "config".to_string(),
            Value::Dict(Map::from([(
                "$path".to_string(),
                Value::String("tests/database.json".into()),
            )])),
        )]));

        assert_eq!(value, expected);
    }

    #[test]
    fn resolve_specials_rejects_non_string_special_value() {
        let value = Value::Dict(Map::from([(
            "config".to_string(),
            Value::Dict(Map::from([("$path".to_string(), Value::U64(123))])),
        )]));

        let err = value.resolve_specials(None).unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("special member $path expects a string value"));
    }

    #[test]
    fn resolve_specials_recursively_resolves_special_path() {
        let value = Value::Dict(Map::from([(
            "config".to_string(),
            Value::Dict(Map::from([(
                "$path".to_string(),
                Value::String("tests/database.json".into()),
            )])),
        )]));

        let resolved = value.resolve_specials(None).unwrap();

        let expected = Value::Dict(Map::from([(
            "config".to_string(),
            Value::Dict(Map::from([
                ("host".to_string(), Value::String("127.0.0.1".into())),
                ("port".to_string(), Value::U64(3306)),
                ("username".to_string(), Value::String("mysql".into())),
                (
                    "password".to_string(),
                    Value::String("mysql_password".into()),
                ),
                ("database".to_string(), Value::String("mysql".into())),
            ])),
        )]));

        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_specials_propagates_special_resolution_error() {
        let value = Value::Dict(Map::from([(
            "config".to_string(),
            Value::Dict(Map::from([(
                "$path".to_string(),
                Value::String("tests/not-found.json".into()),
            )])),
        )]));

        let err = value.resolve_specials(None).unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("failed to read tests/not-found.json"));
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct Database {
        host: String,
        port: u16,
        username: String,
        password: String,
        database: String,
    }

    #[test]
    fn deserialize_value_into_typed_config() {
        let value = Value::Dict(Map::from([
            ("host".to_string(), Value::String("127.0.0.1".to_string())),
            ("port".to_string(), Value::U64(3306)),
            ("username".to_string(), Value::String("mysql".to_string())),
            (
                "password".to_string(),
                Value::String("mysql_password".to_string()),
            ),
            ("database".to_string(), Value::String("mysql".to_string())),
        ]));

        let database: Database = value.deserialize_into().unwrap();

        assert_eq!(
            database,
            Database {
                host: "127.0.0.1".into(),
                port: 3306,
                username: "mysql".into(),
                password: "mysql_password".into(),
                database: "mysql".into(),
            }
        );
    }
}
