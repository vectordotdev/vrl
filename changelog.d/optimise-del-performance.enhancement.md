Optimised the `del` stdlib function and internal `crud::remove` leaf lookup to eliminate unnecessary deep cloning on local variables and expression targets, avoid per-call `OwnedTargetPath` allocations, and fold omitted or literal boolean `compact` flags at compile time.

authors: jimmystewpot
