`ObjectMap` is no longer a type alias for `BTreeMap<KeyString, Value>`. It is now a dedicated type with
an opaque internal representation, so that representation can change for efficiency in the future.
Iteration remains sorted by key, and `ObjectMap` provides the commonly used map methods (including an
entry API and dedicated iterator types) along with conversion from `BTreeMap<KeyString, Value>`. Code that
relied on `ObjectMap` being a `BTreeMap` (for example by calling `BTreeMap`-specific methods or naming
`btree_map` iterator types) needs to be updated.

authors: lukesteensen
