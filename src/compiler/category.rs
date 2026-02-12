use serde::{Deserialize, Serialize};
use strum_macros::AsRefStr;

/// Standard VRL function categories.
///
/// These categories are used to organize VRL standard library functions
/// in documentation and tooling.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, AsRefStr,
)]
#[serde(rename_all = "PascalCase")]
pub enum Category {
    /// Array manipulation functions
    Array,
    /// Encoding and decoding functions
    Codec,
    /// Type coercion functions
    Coerce,
    /// Type conversion functions
    Convert,
    /// Debugging functions
    Debug,
    /// Enumeration and iteration functions
    Enumerate,
    /// Path manipulation functions
    Path,
    /// Cryptographic functions
    Cryptography,
    /// IP address functions
    #[serde(rename = "IP")]
    #[strum(serialize = "IP")]
    Ip,
    /// Mapping/distance related functions
    Map,
    /// Numeric functions
    Number,
    /// Object manipulation functions
    Object,
    /// Parsing functions
    Parse,
    /// Random value generation functions
    Random,
    /// String manipulation functions
    String,
    /// System functions
    System,
    /// Timestamp functions
    Timestamp,
    /// Type checking functions
    Type,
    /// Checksum functions
    Checksum,
}
