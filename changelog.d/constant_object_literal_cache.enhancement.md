VRL programs that use object literals whose values are all literals (or nested
object/array literals) now run faster: the object value is assembled once at
compile time and cloned on evaluation, instead of being re-assembled entry by
entry (resolving each value, cloning each key, and re-inserting into a
`BTreeMap`) on every run. This benefits scripts that define constant lookup
tables, for example syslog severity/facility mappings evaluated per event.

The cache is intentionally restricted to structurally side-effect-free
expressions; variables, queries, function calls, and blocks never qualify
(see vectordotdev/vrl#1799 for why compile-time state is not a sound source
of runtime values).

authors: anton-yakushev
