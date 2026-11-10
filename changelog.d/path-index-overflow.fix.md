Reject field paths with array indices outside the `isize` range instead of
panicking in debug builds or silently wrapping to a different index in release builds.

authors: pront
