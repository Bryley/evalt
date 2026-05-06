use std::fmt::Display;

use regex::Regex;
use schemars::JsonSchema;
use serde::Deserialize;

use super::deserialize_regex;

/// Numeric comparison operators for integer-like run values.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "op", content = "right")]
pub enum NumericOp<T> {
    /// Equal to.
    #[serde(rename = "==")]
    Eq(T),

    /// Not equal to.
    #[serde(rename = "!=")]
    Ne(T),

    /// Less than.
    #[serde(rename = "<")]
    Lt(T),

    /// Less than or equal to.
    #[serde(rename = "<=")]
    Lte(T),

    /// Greater than.
    #[serde(rename = ">")]
    Gt(T),

    /// Greater than or equal to.
    #[serde(rename = ">=")]
    Gte(T),
}

impl<T> NumericOp<T> {
    pub fn symbol(&self) -> &'static str {
        match self {
            NumericOp::Eq(_) => "==",
            NumericOp::Ne(_) => "!=",
            NumericOp::Lt(_) => "<",
            NumericOp::Lte(_) => "<=",
            NumericOp::Gt(_) => ">",
            NumericOp::Gte(_) => ">=",
        }
    }

    pub fn value_display(&self) -> String
    where
        T: Display,
    {
        match self {
            NumericOp::Eq(x)
            | NumericOp::Ne(x)
            | NumericOp::Lt(x)
            | NumericOp::Lte(x)
            | NumericOp::Gt(x)
            | NumericOp::Gte(x) => x.to_string(),
        }
    }

    pub fn compare<K>(&self, value: K) -> bool
    where
        K: PartialOrd<T>,
    {
        match self {
            NumericOp::Eq(v) => value == *v,
            NumericOp::Ne(v) => value != *v,
            NumericOp::Lt(v) => value < *v,
            NumericOp::Lte(v) => value <= *v,
            NumericOp::Gt(v) => value > *v,
            NumericOp::Gte(v) => value >= *v,
        }
    }
}

/// String comparison operators for text values.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", tag = "op")]
pub enum StringOp {
    /// Exactly equal to.
    #[serde(rename = "==")]
    Eq { right: String },

    /// Not equal to.
    #[serde(rename = "!=")]
    Ne { right: String },

    /// Contains the expected substring.
    Contains {
        right: String,
        #[serde(default)]
        case_sensitive: bool,
    },

    /// Does not contain the expected substring.
    NotContains {
        right: String,
        #[serde(default)]
        case_sensitive: bool,
    },

    /// Matches the expected regular expression.
    MatchesRegex {
        #[serde(deserialize_with = "deserialize_regex")]
        #[schemars(with = "String")]
        right: Regex,
    },

    /// Starts with the expected prefix.
    StartsWith {
        right: String,
        #[serde(default)]
        case_sensitive: bool,
    },

    /// Ends with the expected suffix.
    EndsWith {
        right: String,
        #[serde(default)]
        case_sensitive: bool,
    },
}

impl StringOp {
    pub fn symbol(&self) -> &'static str {
        match self {
            StringOp::Eq { .. } => "==",
            StringOp::Ne { .. } => "!=",
            StringOp::Contains { .. } => "contains",
            StringOp::NotContains { .. } => "not_contains",
            StringOp::MatchesRegex { .. } => "matches_regex",
            StringOp::StartsWith { .. } => "starts_with",
            StringOp::EndsWith { .. } => "ends_with",
        }
    }

    pub fn value_display(&self) -> String {
        match self {
            StringOp::Eq { right }
            | StringOp::Ne { right }
            | StringOp::Contains { right, .. }
            | StringOp::NotContains { right, .. }
            | StringOp::StartsWith { right, .. }
            | StringOp::EndsWith { right, .. } => right.clone(),
            StringOp::MatchesRegex { right } => right.to_string(),
        }
    }

    pub fn compare(&self, value: &str) -> bool {
        match self {
            StringOp::Eq { right } => right == value,
            StringOp::Ne { right } => right != value,
            StringOp::Contains {
                right,
                case_sensitive,
            } => compare_string(value, right, *case_sensitive, |value, right| {
                value.contains(right)
            }),
            StringOp::NotContains {
                right,
                case_sensitive,
            } => !compare_string(value, right, *case_sensitive, |value, right| {
                value.contains(right)
            }),
            StringOp::MatchesRegex { right } => right.is_match(value),
            StringOp::StartsWith {
                right,
                case_sensitive,
            } => compare_string(value, right, *case_sensitive, |value, right| {
                value.starts_with(right)
            }),
            StringOp::EndsWith {
                right,
                case_sensitive,
            } => compare_string(value, right, *case_sensitive, |value, right| {
                value.ends_with(right)
            }),
        }
    }
}

fn compare_string(
    value: &str,
    right: &str,
    case_sensitive: bool,
    compare: impl Fn(&str, &str) -> bool,
) -> bool {
    if case_sensitive {
        compare(value, right)
    } else {
        compare(&value.to_lowercase(), &right.to_lowercase())
    }
}

/// Boolean comparison operators.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "op", content = "right")]
pub enum BoolOp<T> {
    /// Equal to.
    #[serde(rename = "==")]
    Eq(T),

    /// Not equal to.
    #[serde(rename = "!=")]
    Ne(T),
}

impl Default for BoolOp<bool> {
    fn default() -> Self {
        BoolOp::Eq(true)
    }
}

impl<T> BoolOp<T> {
    pub fn symbol(&self) -> &'static str {
        match self {
            BoolOp::Eq(_) => "==",
            BoolOp::Ne(_) => "!=",
        }
    }

    pub fn value_display(&self) -> String
    where
        T: Display,
    {
        match self {
            BoolOp::Eq(x) | BoolOp::Ne(x) => x.to_string(),
        }
    }

    pub fn compare<K>(&self, value: K) -> bool
    where
        K: PartialEq<T>,
    {
        match self {
            BoolOp::Eq(v) => value == *v,
            BoolOp::Ne(v) => value != *v,
        }
    }
}
