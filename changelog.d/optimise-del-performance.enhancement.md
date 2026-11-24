Optimised the `del` stdlib function and internal `crud::remove` leaf lookup to eliminate unnecessary deep cloning on local variables and owned expression targets, avoid redundant leaf `BTreeMap` lookups, and fold omitted or literal boolean `compact` flags at compile time.

authors: jimmystewpot
