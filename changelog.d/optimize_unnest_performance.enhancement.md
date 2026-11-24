Optimized the `unnest` stdlib function to avoid redundant full-root clones on error, empty-array, root-path, and owned-target paths, reuse the base object for the final element, and overwrite pure-field paths in place instead of pruning and reinserting on each output element.

authors: jimmystewpot
