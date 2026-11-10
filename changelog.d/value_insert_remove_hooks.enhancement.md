Added `Value::insert_with_hook` and `Value::remove_with_hook`, which call a hook with the slot about to be modified, so that callers can maintain derived state such as size estimates without walking the value again.

authors: gwenaskell
