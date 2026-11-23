Eliminated redundant right-hand-side `Value` clones when executing VRL assignments whose return values are discarded (such as non-final statements in blocks, groups, and `if` branches), delivering up to a ~2× runtime speedup on large-payload assignments such as `. = set!(., ...)`, `. = remove!(., ...)`, and `. = unnest(.events)`.

authors: jimmystewpot
