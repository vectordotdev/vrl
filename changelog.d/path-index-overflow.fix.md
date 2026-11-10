In release builds, reject field paths with array indices outside the `isize` range
instead of silently wrapping to a different index.

authors: pront
