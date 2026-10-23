`ObjectMap` now stores its entries in a single vector sorted by key instead of a `BTreeMap`. Typical
event objects are small, so lookups, construction, cloning, iteration, and dropping are faster and use
less memory. Iteration order is unchanged (sorted by key).

authors: lukesteensen
