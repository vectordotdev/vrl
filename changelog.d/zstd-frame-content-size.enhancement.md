The `encode_zstd` function now includes the uncompressed size in the Zstandard
frame header for inputs smaller than 128 KiB, following the `zstd` 0.14.4 update.
This changes the compressed bytes and their hashes while remaining compatible
with existing Zstandard decoders.

authors: pront
