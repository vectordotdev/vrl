# Diagnostics

This directory contains tests that validate the output of our diagnostic
messages.

The name of the file should preferably be similar to the title of the diagnostic
message.

A diagnostic message is shown at compile-time, similar to this example:

```text
error: unnecessary error assignment
  ┌─ :2:1
  │
2 │ ok, err = 5;
  │ ^^^^^^^   - because this expression cannot fail
  │ │
  │ this error assignment is unnecessary
  │
  = hint: assign to "ok", without assigning to "err"
  = see language documentation at: https://vector.dev/docs/reference/vrl/
  = try your code in the VRL REPL, learn more at https://vrl.dev/examples
```
