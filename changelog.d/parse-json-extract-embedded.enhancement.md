The `parse_json` function now accepts `extract_embedded` and `max_documents` parameters.
With `extract_embedded: true`, it extracts every JSON object and array embedded in
surrounding text (such as a log preamble) and returns them as an array. `max_documents`
limits how many documents are extracted. Default behavior is unchanged.

authors: sgargan
