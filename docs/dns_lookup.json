{
  "anchor": "dns_lookup",
  "name": "dns_lookup",
  "category": "System",
  "description": "Performs a DNS lookup on the provided domain name.",
  "arguments": [
    {
      "name": "value",
      "description": "The domain name to query.",
      "required": true,
      "type": [
        "string"
      ]
    },
    {
      "name": "qtype",
      "description": "The DNS record type to query (e.g., A, AAAA, MX, TXT). Defaults to A.",
      "required": false,
      "type": [
        "string"
      ],
      "default": "A"
    },
    {
      "name": "class",
      "description": "The DNS query class. Defaults to IN (Internet).",
      "required": false,
      "type": [
        "string"
      ],
      "default": "IN"
    },
    {
      "name": "options",
      "description": "DNS resolver options. Supported fields: servers (array of nameserver addresses), timeout (seconds), attempts (number of retry attempts), ndots, aa_only, tcp, recurse, rotate.",
      "required": false,
      "type": [
        "object"
      ],
      "default": "{  }"
    }
  ],
  "return": {
    "types": [
      "object"
    ]
  },
  "notices": [
    "This function performs network calls and blocks on each request until a response is\nreceived. It is not recommended for frequent or performance-critical workflows."
  ],
  "pure": true
}
