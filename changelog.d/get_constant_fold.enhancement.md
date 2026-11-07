The `get` function now resolves a literal object or array value argument once at compile time
instead of re-resolving it for every event. VRL programs that use inline lookup tables, such as
`get({"emerg": 21, "info": 9}, [to_string(.severity)])`, avoid rebuilding the table on every event.

authors: yackushevas
