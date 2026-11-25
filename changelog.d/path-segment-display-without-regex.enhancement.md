Formatting a path field no longer runs a regex to decide whether the field needs quotes. This removes regex cache-pool contention when paths are formatted from many threads at once.

authors: gwenaskell
