Optimised the `get` stdlib function to borrow external targets and local variables directly instead of deep-cloning the payload, move values out of owned temporaries in O(1), pre-compile constant path arrays, and traverse dynamic paths without per-call heap allocations.

authors: jimmystewpot
