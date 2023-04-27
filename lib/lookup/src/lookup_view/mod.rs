use std::collections::VecDeque;

// use crate::{LookupBuf, SegmentBuf};

mod segment;
pub use segment::{Field, Segment};

/// `Lookup`s are pre-validated event, unowned lookup paths.
///
/// These are unowned, ordered sets of segments. `Segment`s represent parts of a path such as
/// `pies.banana.slices[0]`. The segments would be `["pies", "banana", "slices", 0]`. You can "walk"
/// a lookup with an `iter()` call.
///
/// # Building
///
/// You build `Lookup`s from `str`s and other str-like objects with a `from()` call.
/// **These do not parse the buffer.**
///
/// ```rust
/// use lookup::Lookup;
/// let mut lookup = Lookup::from("foo");
/// lookup.push_back(1);
/// lookup.push_back("bar");
///
/// let mut lookup = Lookup::from("foo.bar"); // This is **not** two segments.
/// lookup.push_back(1);
/// lookup.push_back("bar");
/// ```
///
/// From there, you can `push` and `pop` onto the `Lookup`.
///
/// # Parsing
///
/// To parse buffer into a `Lookup`, use the `std::str::FromStr` implementation. If you're working
/// something that's not able to be a `str`, you should consult `std::str::from_utf8` and handle the
/// possible error.
///
/// ```rust
/// use lookup::Lookup;
/// let mut lookup = Lookup::from_str("foo").unwrap();
/// lookup.push_back(1);
/// lookup.push_back("bar");
///
/// let mut lookup = Lookup::from_str("foo.bar").unwrap(); // This **is** two segments.
/// lookup.push_back(1);
/// lookup.push_back("bar");
/// ```
///
/// # Owned Variant
///
/// There exists an owned variant of this type appropriate for more flexible contexts or where you
/// have a string. (Say, most of the time).
///
/// To shed ownership use `lookup_buf.into_buf()`. To gain ownership of a `lookup`, use
/// `lookup.into()`.
///
/// ```rust
/// use lookup::Lookup;
/// let mut lookup = Lookup::from_str("foo.bar").unwrap();
/// let mut owned = lookup.clone().into_buf();
/// owned.push_back(1);
/// owned.push_back("bar");
/// lookup.push_back("baz"); // Does not impact the owned!
/// ```
///
/// # Warnings
///
/// * You **can not** deserialize lookups (that is, views, the buffers are fine) out of str slices
///   with escapes in serde_json. [serde_json does not allow it.](https://github.com/serde-rs/json/blob/master/src/read.rs#L424-L476)
///   You **must** use strings. This means it is **almost always not a good idea to deserialize a
///   string into a `Lookup`. **Use a `LookupBuf` instead.**
#[derive(Debug, PartialEq, Eq, Default, PartialOrd, Ord, Clone, Hash)]
pub struct Lookup<'a> {
    pub segments: VecDeque<Segment<'a>>,
}
