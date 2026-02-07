pub mod auth;
pub mod responses;
pub mod subs;
pub mod wallets;
pub mod websockets;

use serde::{Deserialize, Deserializer, Serialize};

/// Defines our update semantics. While this derives [Serialize](serde::Serialize), it should never
/// actually be used. Only derives it to provide OpenAPI schema
///
/// When using as part of a struct, the value must be marked with the `#[serde(default)]`
/// attribute.
///
/// See: <https://itsfoxstudio.substack.com/p/rust-patterns-patch-type>
/// Adapted from: <https://github.com/itsfoxstudio/value-extra>
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Eq, Default)]
pub enum Patch<T> {
    /// A new value to update the field to
    Some(T),
    #[default]
    /// No value was passed, do not update field
    None,
    /// Value was explicitly `null`, clear/reset the field
    Null,
}

impl<T> Patch<T> {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl<T> From<Option<T>> for Patch<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Self::Some(v),
            None => Self::None,
        }
    }
}

impl<T> From<Patch<T>> for Option<T> {
    fn from(patch: Patch<T>) -> Self {
        match patch {
            Patch::Some(v) => Some(v),
            _ => None,
        }
    }
}

impl<'de, T> Deserialize<'de> for Patch<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Map like this rather than just using .into() because I don't want the default behavior
        // for Option::None to convert to Patch::Null
        Option::deserialize(deserializer).map(|opt| match opt {
            Some(v) => Self::Some(v),
            None => Self::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use crate::models::kromer::Patch;

    #[derive(Debug, Clone, Deserialize)]
    struct PatchWrapper {
        #[serde(default)]
        val: Patch<String>,
    }

    #[test]
    fn deserialize_some() {
        let s = json!({ "val": "foo" });

        let parsed: PatchWrapper = serde_json::from_value(s).expect("Failed to deserialize");

        assert_eq!(
            Patch::Some("foo".to_string()),
            parsed.val,
            "Failed to parse into Some"
        );
    }

    #[test]
    fn deserialize_none() {
        let s = json!({});

        let parsed: PatchWrapper = serde_json::from_value(s).expect("Failed to deserialize");

        assert_eq!(Patch::None, parsed.val, "Failed to parse into None")
    }

    #[test]
    fn deserialize_null() {
        let s = json!({ "val": null });

        let parsed: PatchWrapper = serde_json::from_value(s).expect("Failed to deserialize");

        assert_eq!(Patch::Null, parsed.val, "Failed to parse into Null");
    }
}
