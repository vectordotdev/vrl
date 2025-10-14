//! # HTTP Get Function
//!
//! This function allows making HTTP requests but is not recommended for frequent or performance-critical workflows.
//! It performs synchronous blocking operations, which can negatively impact concurrency and increase response times.
//!
//! Due to potential network-related delays or failures, avoid using this function in latency-sensitive contexts.

use crate::compiler::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
mod non_wasm {
    use super::{
        Context, Expression, ExpressionError, FunctionExpression, Resolved, TypeDef, TypeState,
        Value, VrlValueConvert,
    };
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

    async fn http_request(
        client: &ClientWithMiddleware,
        url: &Value,
        method: &Value,
        headers: Value,
        body: &Value,
    ) -> Resolved {
        let url = url.try_bytes_utf8_lossy()?;
        let method = method.try_bytes_utf8_lossy()?.to_uppercase();
        let headers = headers.try_object()?;
        let body = body.try_bytes_utf8_lossy()?;

        let method = Method::try_from(method.as_str())
            .map_err(|_| format!("Unsupported HTTP method: {method}"))?;
        let mut header_map = HeaderMap::new();

        for (key, value) in &headers {
            let key = key
                .parse::<HeaderName>()
                .map_err(|_| format!("Invalid header key: {key}"))?;
            let val = value
                .try_bytes_utf8_lossy()?
                .parse::<HeaderValue>()
                .map_err(|_| format!("Invalid header value: {value}"))?;
            header_map.insert(key, val);
        }

        let response = client
            .request(method, url.as_ref())
            .headers(header_map)
            .body(body.as_bytes().to_owned())
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        Ok(body.into())
    }

    #[allow(clippy::similar_names)]
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

        if proxies.is_empty() {
            Ok(None)
        } else {
            Ok(Some(proxies))
        }
    }

    // Used to avoid clones
    enum RefOrOwnedClient<'c> {
        Owned(ClientWithMiddleware),
        Ref(&'c ClientWithMiddleware),
    }

    impl AsRef<ClientWithMiddleware> for RefOrOwnedClient<'_> {
        fn as_ref(&self) -> &ClientWithMiddleware {
            match self {
                Self::Owned(client) => client,
                Self::Ref(client) => client,
            }
        }
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
        #[allow(clippy::similar_names)]
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

        #[allow(clippy::similar_names)]
        pub fn new_proxies_no_const_resolve(
            http_proxy: Option<Box<dyn Expression>>,
            https_proxy: Option<Box<dyn Expression>>,
        ) -> Self {
            Self::Proxies {
                http_proxy,
                https_proxy,
            }
        }

        #[allow(clippy::similar_names)]
        fn get_client<'c>(
            &'c self,
            ctx: &mut Context,
        ) -> Result<RefOrOwnedClient<'c>, ExpressionError> {
            match self {
                Self::Client(client) => Ok(RefOrOwnedClient::Ref(client)),
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
                        Ok(RefOrOwnedClient::Owned(build_client(Some(proxies))))
                    } else {
                        Ok(RefOrOwnedClient::Ref(&STD_CLIENT))
                    }
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    pub(super) struct HttpRequestFn {
        pub(super) url: Box<dyn Expression>,
        pub(super) method: Box<dyn Expression>,
        pub(super) headers: Box<dyn Expression>,
        pub(super) body: Box<dyn Expression>,
        pub(super) client_or_proxies: ClientOrProxies,
    }

    impl FunctionExpression for HttpRequestFn {
        #[allow(clippy::similar_names)]
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let url = self.url.resolve(ctx)?;
            let method = self.method.resolve(ctx)?;
            let headers = self.headers.resolve(ctx)?;
            let body = self.body.resolve(ctx)?;
            let client = self.client_or_proxies.get_client(ctx)?;
            let client_ref = client.as_ref();

            // block_in_place runs the HTTP request synchronously
            // without blocking Tokio's async worker threads.
            // This temporarily moves execution to a blocking-compatible thread.
            task::block_in_place(|| {
                if let Ok(handle) = Handle::try_current() {
                    handle.block_on(async {
                        http_request(client_ref, &url, &method, headers, &body).await
                    })
                } else {
                    let runtime = runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("tokio runtime creation failed");

                    runtime.block_on(async move {
                        http_request(client_ref, &url, &method, headers, &body).await
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

#[derive(Clone, Copy, Debug)]
pub struct HttpRequest;

impl Function for HttpRequest {
    fn identifier(&self) -> &'static str {
        "http_request"
    }

    #[cfg(not(feature = "test"))]
    fn examples(&self) -> &'static [Example] {
        &[]
    }

    #[cfg(feature = "test")]
    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "Basic HTTP request",
                source: r#"http_request("https://httpbin.org/get")"#,
                result: Ok(
                    r#"{"args":{},"headers":{"Accept":"*/*","Host":"httpbin.org"},"url":"https://httpbin.org/get"}"#,
                ),
            },
            Example {
                title: "HTTP request with bearer token",
                source: r#"http_request("https://httpbin.org/bearer", headers: {"Authorization": "Bearer my_token"})"#,
                result: Ok(r#"{"authenticated":true,"token":"my_token"}"#),
            },
            Example {
                title: "HTTP PUT request",
                source: r#"http_request("https://httpbin.org/put", method: "put")"#,
                result: Ok(r#"{"args":{},"data": "","url": "https://httpbin.org/put"}"#),
            },
            Example {
                title: "HTTP POST request with body",
                source: r#"http_request("https://httpbin.org/post", method: "post", body: "{\"data\":{\"hello\":\"world\"}}")"#,
                result: Ok(r#"{"data":"{\"data\":{\"hello\":\"world\"}}"}"#),
            },
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "url",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "method",
                kind: kind::BYTES,
                required: false,
            },
            Parameter {
                keyword: "headers",
                kind: kind::OBJECT,
                required: false,
            },
            Parameter {
                keyword: "body",
                kind: kind::BYTES,
                required: false,
            },
            Parameter {
                keyword: "http_proxy",
                kind: kind::BYTES,
                required: false,
            },
            Parameter {
                keyword: "https_proxy",
                kind: kind::BYTES,
                required: false,
            },
        ]
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
        let method = arguments.optional("method").unwrap_or_else(|| expr!("get"));
        let headers = arguments.optional("headers").unwrap_or_else(|| expr!({}));
        let body = arguments.optional("body").unwrap_or_else(|| expr!(""));
        let http_proxy = arguments.optional("http_proxy");
        let https_proxy = arguments.optional("https_proxy");

        let client_or_proxies = ClientOrProxies::new(state, http_proxy, https_proxy)
            .map_err(|err| Box::new(err) as Box<dyn DiagnosticMessage>)?;

        Ok(HttpRequestFn {
            url,
            method,
            headers,
            body,
            client_or_proxies,
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
    use tokio;

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
            method: expr!("get"),
            headers: expr!({}),
            body: expr!(""),
            client_or_proxies: ClientOrProxies::no_proxies(),
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
            method: expr!("get"),
            headers: expr!({}),
            body: expr!(""),
            client_or_proxies: ClientOrProxies::no_proxies(),
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
            method: expr!("get"),
            headers: expr!({"Invalid Header With Spaces": "value"}),
            body: expr!(""),
            client_or_proxies: ClientOrProxies::no_proxies(),
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
            method: expr!("get"),
            headers: expr!({}),
            body: expr!(""),
            client_or_proxies: ClientOrProxies::new_proxies_no_const_resolve(
                None,
                Some(expr!("not^a&valid*url")),
            ),
        };

        let result = execute_http_request(&func);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid proxy"));
    }
}
