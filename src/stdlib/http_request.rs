//! # HTTP Get Function
//!
//! This function allows making HTTP requests but is not recommended for frequent or performance-critical workflows.
//! It performs synchronous blocking operations, which can negatively impact concurrency and increase response times.
//!
//! Due to potential network-related delays or failures, avoid using this function in latency-sensitive contexts.

use crate::compiler::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::similar_names)]
mod non_wasm {
    use super::{
        Context, Expression, ExpressionError, ExpressionExt, FunctionExpression, Resolved, TypeDef,
        TypeState, Value, VrlValueConvert,
    };
    use crate::value::value::ObjectMap;
    use reqwest_middleware::{
        ClientBuilder, ClientWithMiddleware,
        reqwest::{
            Client, Method, Proxy,
            header::{HeaderMap, HeaderName, HeaderValue},
        },
    };
    use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
    use std::sync::LazyLock;
    use tokio::runtime::Handle;
    use tokio::time::Duration;
    use tokio::{runtime, task};

    static STD_CLIENT: LazyLock<ClientWithMiddleware> = LazyLock::new(|| build_client(None));

    fn build_client(proxies: Option<Vec<Proxy>>) -> ClientWithMiddleware {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

        let mut client_builder = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10));

        if let Some(proxies) = proxies {
            for proxy in proxies {
                client_builder = client_builder.proxy(proxy);
            }
        }

        let client = client_builder
            .build()
            .expect("Failed to create HTTP client");

        ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    }

    /// Redacts sensitive header values to prevent them from appearing in error messages.
    /// Headers like Authorization, Cookie, and API keys are replaced with ***.
    fn redact_sensitive_headers(headers: &ObjectMap) -> ObjectMap {
        const SENSITIVE_HEADERS: &[&str] = &[
            "authorization",
            "cookie",
            "set-cookie",
            "x-api-key",
            "api-key",
            "x-auth-token",
            "proxy-authorization",
        ];

        headers
            .iter()
            .map(|(key, value)| {
                let key_lower = key.as_ref().to_lowercase();
                if SENSITIVE_HEADERS.contains(&key_lower.as_str())
                    || key_lower.contains("token")
                    || key_lower.contains("secret")
                    || key_lower.contains("password")
                {
                    (key.clone(), Value::from("***"))
                } else {
                    (key.clone(), value.clone())
                }
            })
            .collect()
    }

    async fn http_request(
        client: &ClientWithMiddleware,
        url: &Value,
        method: &Value,
        headers: Value,
        body: &Value,
        redact_headers: bool,
    ) -> Resolved {
        let url = url.try_bytes_utf8_lossy()?;
        let method = method.try_bytes_utf8_lossy()?.to_uppercase();
        let headers = headers.try_object()?;
        let body = body.try_bytes_utf8_lossy()?;

        let format_headers = |headers: &ObjectMap| -> Value {
            if redact_headers {
                Value::Object(redact_sensitive_headers(headers))
            } else {
                Value::Object(headers.clone())
            }
        };

        let method = Method::try_from(method.as_str())
            .map_err(|_| format!("Unsupported HTTP method: {method}"))?;
        let mut header_map = HeaderMap::new();

        for (key, value) in &headers {
            let key = key
                .parse::<HeaderName>()
                .map_err(|_| format!("Invalid header key: {key}"))?;
            let val = value
                .try_bytes_utf8_lossy()
                .map_err(|e| {
                    format!(
                        "Invalid header value for key '{key}': {} (headers: {})",
                        e,
                        format_headers(&headers)
                    )
                })?
                .parse::<HeaderValue>()
                .map_err(|_| {
                    format!(
                        "Invalid header value for key '{key}' (headers: {})",
                        format_headers(&headers)
                    )
                })?;
            header_map.insert(key, val);
        }

        let response = client
            .request(method.clone(), url.as_ref())
            .headers(header_map)
            .body(body.as_bytes().to_owned())
            .send()
            .await
            .map_err(|e| {
                format!(
                    "HTTP request failed: {} (url: {}, method: {}, headers: {})",
                    e,
                    url,
                    method,
                    format_headers(&headers)
                )
            })?;

        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        Ok(body.into())
    }

    fn make_proxies(
        http_proxy: Option<Value>,
        https_proxy: Option<Value>,
    ) -> Result<Option<Vec<Proxy>>, ExpressionError> {
        let mut proxies = Vec::new();

        if let Some(http_proxy) = http_proxy {
            proxies.push(
                Proxy::http(http_proxy.try_bytes_utf8_lossy()?.as_ref())
                    .map_err(|e| format!("Invalid proxy: {e}"))?,
            );
        }

        if let Some(https_proxy) = https_proxy {
            proxies.push(
                Proxy::https(https_proxy.try_bytes_utf8_lossy()?.as_ref())
                    .map_err(|e| format!("Invalid proxy: {e}"))?,
            );
        }

        Ok((!proxies.is_empty()).then_some(proxies))
    }

    #[derive(Debug, Clone)]
    pub(super) enum ClientOrProxies {
        Client(ClientWithMiddleware),
        Proxies {
            http_proxy: Option<Box<dyn Expression>>,
            https_proxy: Option<Box<dyn Expression>>,
        },
    }

    impl ClientOrProxies {
        pub(super) fn new(
            state: &TypeState,
            http_proxy: Option<Box<dyn Expression>>,
            https_proxy: Option<Box<dyn Expression>>,
        ) -> Result<Self, ExpressionError> {
            let const_http_proxy = http_proxy
                .as_ref()
                .map(|http_proxy| http_proxy.resolve_constant(state));
            let const_https_proxy = https_proxy
                .as_ref()
                .map(|https_proxy| https_proxy.resolve_constant(state));

            match (const_http_proxy, const_https_proxy) {
                // No proxies specified
                (None, None) => Ok(Self::no_proxies()),
                // Only http proxy specified and could resolve as constant
                (Some(Some(http)), None) => {
                    Ok(Self::Client(build_client(make_proxies(Some(http), None)?)))
                }
                // Only https proxy specified and could resolve as constant
                (None, Some(Some(https))) => {
                    Ok(Self::Client(build_client(make_proxies(None, Some(https))?)))
                }
                // Both proxies specified and could resolve as constants
                (Some(Some(http)), Some(Some(https))) => Ok(Self::Client(build_client(
                    make_proxies(Some(http), Some(https))?,
                ))),
                // Couldn't evaluate as constants
                _ => Ok(Self::new_proxies_no_const_resolve(http_proxy, https_proxy)),
            }
        }

        pub fn no_proxies() -> Self {
            Self::Proxies {
                http_proxy: None,
                https_proxy: None,
            }
        }

        pub fn new_proxies_no_const_resolve(
            http_proxy: Option<Box<dyn Expression>>,
            https_proxy: Option<Box<dyn Expression>>,
        ) -> Self {
            Self::Proxies {
                http_proxy,
                https_proxy,
            }
        }

        fn get_client(&self, ctx: &mut Context) -> Result<ClientWithMiddleware, ExpressionError> {
            match self {
                Self::Client(client) => Ok(client.clone()),
                Self::Proxies {
                    http_proxy,
                    https_proxy,
                } => {
                    let http_proxy = http_proxy
                        .as_ref()
                        .map(|http_proxy| http_proxy.resolve(ctx))
                        .transpose()?;

                    let https_proxy = https_proxy
                        .as_ref()
                        .map(|https_proxy| https_proxy.resolve(ctx))
                        .transpose()?;

                    if let Some(proxies) = make_proxies(http_proxy, https_proxy)? {
                        Ok(build_client(Some(proxies)))
                    } else {
                        Ok(STD_CLIENT.clone())
                    }
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    pub(super) struct HttpRequestFn {
        pub(super) url: Box<dyn Expression>,
        pub(super) method: Option<Box<dyn Expression>>,
        pub(super) headers: Option<Box<dyn Expression>>,
        pub(super) body: Option<Box<dyn Expression>>,
        pub(super) client_or_proxies: ClientOrProxies,
        pub(super) redact_headers: Option<Box<dyn Expression>>,
    }

    impl FunctionExpression for HttpRequestFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let url = self.url.resolve(ctx)?;
            let method = self
                .method
                .map_resolve_with_default(ctx, || super::DEFAULT_METHOD.clone())?;
            let headers = self
                .headers
                .map_resolve_with_default(ctx, || super::DEFAULT_HEADERS.clone())?;
            let body = self
                .body
                .map_resolve_with_default(ctx, || super::DEFAULT_BODY.clone())?;
            let client = self.client_or_proxies.get_client(ctx)?;
            let redact_headers = self
                .redact_headers
                .map_resolve_with_default(ctx, || super::DEFAULT_REDACT_HEADERS.clone())?
                .try_boolean()?;

            // block_in_place runs the HTTP request synchronously
            // without blocking Tokio's async worker threads.
            // This temporarily moves execution to a blocking-compatible thread.
            task::block_in_place(|| {
                if let Ok(handle) = Handle::try_current() {
                    handle.block_on(async {
                        http_request(&client, &url, &method, headers, &body, redact_headers).await
                    })
                } else {
                    let runtime = runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("tokio runtime creation failed");

                    runtime.block_on(async move {
                        http_request(&client, &url, &method, headers, &body, redact_headers).await
                    })
                }
            })
        }

        fn type_def(&self, _: &TypeState) -> TypeDef {
            TypeDef::bytes().fallible()
        }
    }
}

#[allow(clippy::wildcard_imports)]
#[cfg(not(target_arch = "wasm32"))]
use non_wasm::*;

use std::sync::LazyLock;

static DEFAULT_METHOD: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("get")));
static DEFAULT_HEADERS: LazyLock<Value> =
    LazyLock::new(|| Value::Object(std::collections::BTreeMap::new()));
static DEFAULT_BODY: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("")));
static DEFAULT_REDACT_HEADERS: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter::required("url", kind::BYTES, "The URL to make the HTTP request to."),
        Parameter::optional(
            "method",
            kind::BYTES,
            "The HTTP method to use (e.g., GET, POST, PUT, DELETE). Defaults to GET.",
        )
        .default(&DEFAULT_METHOD),
        Parameter::optional(
            "headers",
            kind::OBJECT,
            "An object containing HTTP headers to send with the request.",
        )
        .default(&DEFAULT_HEADERS),
        Parameter::optional("body", kind::BYTES, "The request body content to send.")
            .default(&DEFAULT_BODY),
        Parameter::optional(
            "http_proxy",
            kind::BYTES,
            "HTTP proxy URL to use for the request.",
        ),
        Parameter::optional(
            "https_proxy",
            kind::BYTES,
            "HTTPS proxy URL to use for the request.",
        ),
        Parameter::optional(
            "redact_headers",
            kind::BOOLEAN,
            "Whether to redact sensitive header values in error messages.",
        )
        .default(&DEFAULT_REDACT_HEADERS),
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct HttpRequest;

impl Function for HttpRequest {
    fn identifier(&self) -> &'static str {
        "http_request"
    }

    fn usage(&self) -> &'static str {
        "Makes an HTTP request to the specified URL."
    }

    fn notices(&self) -> &'static [&'static str] {
        &[indoc! {"
            This function performs synchronous blocking operations and is not recommended for
            frequent or performance-critical workflows due to potential network-related delays.
        "}]
    }

    fn category(&self) -> &'static str {
        Category::System.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    #[cfg(not(feature = "test"))]
    fn examples(&self) -> &'static [Example] {
        &[]
    }

    #[cfg(feature = "test")]
    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Basic HTTP request",
                source: r#"http_request("https://httpbin.org/get")"#,
                result: Ok(
                    r#"{"args":{},"headers":{"Accept":"*/*","Host":"httpbin.org"},"url":"https://httpbin.org/get"}"#,
                ),
            },
            example! {
                title: "HTTP request with bearer token",
                source: r#"http_request("https://httpbin.org/bearer", headers: {"Authorization": "Bearer my_token"})"#,
                result: Ok(r#"{"authenticated":true,"token":"my_token"}"#),
            },
            example! {
                title: "HTTP PUT request",
                source: r#"http_request("https://httpbin.org/put", method: "put")"#,
                result: Ok(r#"{"args":{},"data": "","url": "https://httpbin.org/put"}"#),
            },
            example! {
                title: "HTTP POST request with body",
                source: r#"http_request("https://httpbin.org/post", method: "post", body: "{\"data\":{\"hello\":\"world\"}}")"#,
                result: Ok(r#"{"data":"{\"data\":{\"hello\":\"world\"}}"}"#),
            },
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::similar_names)]
    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let url = arguments.required("url");
        let method = arguments.optional("method");
        let headers = arguments.optional("headers");
        let body = arguments.optional("body");
        let http_proxy = arguments.optional("http_proxy");
        let https_proxy = arguments.optional("https_proxy");
        let redact_headers = arguments.optional("redact_headers");

        let client_or_proxies = ClientOrProxies::new(state, http_proxy, https_proxy)
            .map_err(|err| Box::new(err) as Box<dyn DiagnosticMessage>)?;

        Ok(HttpRequestFn {
            url,
            method,
            headers,
            body,
            client_or_proxies,
            redact_headers,
        }
        .as_expr())
    }

    #[cfg(target_arch = "wasm32")]
    fn compile(
        &self,
        _state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        _arguments: ArgumentList,
    ) -> Compiled {
        Ok(
            super::wasm_unsupported_function::WasmUnsupportedFunction::new(
                ctx.span(),
                TypeDef::bytes().fallible(),
            )
            .as_expr(),
        )
    }
}

#[cfg(all(feature = "test", test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::value;

    fn execute_http_request(http_request_fn: &HttpRequestFn) -> Resolved {
        let tz = TimeZone::default();
        let mut object = value!({});
        let mut runtime_state = state::RuntimeState::default();
        let mut ctx = Context::new(&mut object, &mut runtime_state, &tz);
        http_request_fn.resolve(&mut ctx)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_basic_get_request() {
        let func: HttpRequestFn = HttpRequestFn {
            url: expr!("https://httpbin.org/get"),
            method: Some(expr!("get")),
            headers: Some(expr!({})),
            body: Some(expr!("")),
            client_or_proxies: ClientOrProxies::no_proxies(),
            redact_headers: expr!(true),
        };

        let result = execute_http_request(&func).expect("HTTP request failed");

        let body = result
            .try_bytes_utf8_lossy()
            .expect("Failed to convert response to string");
        let response: serde_json::Value =
            serde_json::from_str(body.as_ref()).expect("Failed to parse JSON");

        assert!(response.get("url").is_some());
        assert_eq!(response["url"], "https://httpbin.org/get");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_malformed_url() {
        let func = HttpRequestFn {
            url: expr!("not-a-valid-url"),
            method: Some(expr!("get")),
            headers: Some(expr!({})),
            body: Some(expr!("")),
            client_or_proxies: ClientOrProxies::no_proxies(),
            redact_headers: expr!(true),
        };

        let result = execute_http_request(&func);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("HTTP request failed"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_invalid_header() {
        let func = HttpRequestFn {
            url: expr!("https://httpbin.org/get"),
            method: Some(expr!("get")),
            headers: Some(expr!({"Invalid Header With Spaces": "value"})),
            body: Some(expr!("")),
            client_or_proxies: ClientOrProxies::no_proxies(),
            redact_headers: expr!(true),
        };

        let result = execute_http_request(&func);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid header key"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_invalid_proxy() {
        let func = HttpRequestFn {
            url: expr!("https://httpbin.org/get"),
            method: Some(expr!("get")),
            headers: Some(expr!({})),
            body: Some(expr!("")),
            client_or_proxies: ClientOrProxies::new_proxies_no_const_resolve(
                None,
                Some(expr!("not^a&valid*url")),
            ),
            redact_headers: expr!(true),
        };

        let result = execute_http_request(&func);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid proxy"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sensitive_headers_redacted() {
        let func = HttpRequestFn {
            url: expr!("not-a-valid-url"),
            method: Some(expr!("get")),
            headers: Some(expr!({
                "Authorization": "Bearer super_secret_12345",
                "X-Api-Key": "key-67890",
                "Content-Type": "application/json",
                "Cookie": "session=abcdef",
                "X-Custom-Token": "another-secret",
                "User-Agent": "VRL/0.28"
            })),
            body: Some(expr!("")),
            client_or_proxies: ClientOrProxies::no_proxies(),
            redact_headers: expr!(true),
        };

        let result = execute_http_request(&func);
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();

        // Verify that sensitive values are redacted
        assert!(
            !error.contains("super_secret_12345"),
            "Authorization token should be redacted"
        );
        assert!(!error.contains("key-67890"), "API key should be redacted");
        assert!(!error.contains("abcdef"), "Cookie should be redacted");
        assert!(
            !error.contains("another-secret"),
            "Custom token should be redacted"
        );

        // Verify that redacted placeholder appears
        assert!(error.contains("***"), "Should contain *** placeholder");

        // Verify that non-sensitive headers are still visible
        assert!(
            error.contains("application/json"),
            "Non-sensitive headers should not be redacted"
        );
        assert!(
            error.contains("VRL/0.28"),
            "User-Agent should not be redacted"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_redact_headers_disabled() {
        let func = HttpRequestFn {
            url: expr!("not-a-valid-url"),
            method: Some(expr!("get")),
            headers: Some(expr!({
                "Authorization": "Bearer super_secret_12345",
                "Content-Type": "application/json"
            })),
            body: Some(expr!("")),
            client_or_proxies: ClientOrProxies::no_proxies(),
            redact_headers: expr!(false),
        };

        let result = execute_http_request(&func);
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();

        // With redaction disabled, sensitive values should be visible
        assert!(
            error.contains("super_secret_12345"),
            "Authorization token should not be redacted when redact_headers is false"
        );
    }
}
