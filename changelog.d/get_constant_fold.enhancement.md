Improved `get` performance for inline objects and arrays containing only constant values such as `get({"emerg": 21, "info": 9}, [to_string!(.severity)]) ?? null`.

authors: yackushevas
