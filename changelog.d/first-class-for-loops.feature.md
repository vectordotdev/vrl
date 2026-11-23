Add first-class `for` loops supporting single-item (`for item in array`) and key-value (`for key, value in collection`) iteration over arrays and objects, native `break` and `continue` control flow, lexically scoped variable shadowing with in-place slot reuse, compile-time collection and fixed-point type validation (`E633`–`E635`), and cooperative execution step limits via `Runtime::resolve_with_limit` and `StepBudget`. The `in` keyword is parsed contextually to preserve backwards compatibility with existing identifiers named `in`.

authors: jimmystewpot
