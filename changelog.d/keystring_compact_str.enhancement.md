`KeyString` (the object key type) is now backed by a small-string-optimized type, so keys of up to 24
bytes are stored inline without a heap allocation. Creating, cloning, and dropping objects is cheaper.

authors: lukesteensen
