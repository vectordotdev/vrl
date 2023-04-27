use std::collections::VecDeque;

mod segmentbuf;
pub use segmentbuf::{FieldBuf, SegmentBuf};

/// `LookupBuf`s are pre-validated, owned event lookup paths.
///
/// These are owned, ordered sets of `SegmentBuf`s. `SegmentBuf`s represent parts of a path such as
/// `pies.banana.slices[0]`. The segments would be `["pies", "banana", "slices", 0]`. You can "walk"
/// a `LookupBuf` with an `iter()` call.
///
/// # Building
///
/// You build `LookupBuf`s from `String`s and other string-like objects with a `from()` or `try_from()`
/// call. **These do not parse the buffer.**
///
/// From there, you can `push` and `pop` onto the `LookupBuf`.
///
/// ```rust
/// use lookup::LookupBuf;
/// let mut lookup = LookupBuf::from("foo");
/// lookup.push_back(1);
/// lookup.push_back("bar");
///
/// let mut lookup = LookupBuf::from("foo.bar"); // This is **not** two segments.
/// lookup.push_back(1);
/// lookup.push_back("bar");
/// ```
///
/// # Parsing
///
/// to parse buffer into a `LookupBuf`, use the `std::str::FromStr` implementation. If you're working
/// something that's not able to be a `str`, you should consult `std::str::from_utf8` and handle the
/// possible error.
///
/// ```rust
/// use lookup::LookupBuf;
/// let mut lookup = LookupBuf::from_str("foo").unwrap();
/// lookup.push_back(1);
/// lookup.push_back("bar");
///
/// let mut lookup = LookupBuf::from_str("foo.bar").unwrap(); // This **is** two segments.
/// lookup.push_back(1);
/// lookup.push_back("bar");
/// ```
///
/// # Unowned Variant
///
/// There exists an unowned variant of this type appropriate for static contexts or where you only
/// have a view into a long lived string. (Say, deserialization of configs).
///
/// To shed ownership use `lookup_buf.to_lookup()`. To gain ownership of a `lookup`, use
/// `lookup.into()`.
///
/// ```rust
/// use lookup::LookupBuf;
/// let mut lookup = LookupBuf::from_str("foo.bar").unwrap();
/// let mut unowned_view = lookup.to_lookup();
/// unowned_view.push_back(1);
/// unowned_view.push_back("bar");
/// lookup.push_back("baz"); // Does not impact the view!
/// ```
///
/// For more, investigate `Lookup`.
#[derive(Debug, PartialEq, Default, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct LookupBuf {
    pub segments: VecDeque<SegmentBuf>,
}
