Fixed incorrect parameter types in several stdlib functions:

- `md5`: `value` parameter was typed as `any`, now correctly typed as `bytes`.
- `seahash`: `value` parameter was typed as `any`, now correctly typed as `bytes`.
- `floor`: `value` parameter was typed as `any`, now correctly typed as `float | integer`; `precision` parameter was typed as `any`, now correctly typed as `integer`.
- `parse_key_value`: `key_value_delimiter` and `field_delimiter` parameters were typed as `any`, now correctly typed as `bytes`.

Note: the function documentation already reflected the correct types.

authors: thomasqueirozb
