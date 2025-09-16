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
        Context, Expression, FunctionExpression, Resolved, TypeDef, TypeState, Value,
        VrlValueConvert,
    };
    use reqwest_middleware::{
        ClientBuilder, ClientWithMiddleware,
        reqwest::{
            Client, Method,
            header::{HeaderMap, HeaderName, HeaderValue},
        },
    };
    use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
    use std::sync::LazyLock;
    use tokio::runtime::Handle;
    use tokio::time::Duration;
    use tokio::{runtime, task};

    static CLIENT: LazyLock<ClientWithMiddleware> = LazyLock::new(|| {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    });

    async fn http_request(url: &Value, method: &Value, headers: Value, body: &Value) -> Resolved {
        let url = url.try_bytes_utf8_lossy()?;
        let method = method.try_bytes_utf8_lossy()?.to_uppercase();
        let headers = headers.try_object()?;
        let body = body.try_bytes_utf8_lossy()?;

        let method = Method::try_from(method.to_uppercase().as_str())
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

        let response = CLIENT
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

    #[derive(Debug, Clone)]
    pub(super) struct HttpRequestFn {
        pub(super) url: Box<dyn Expression>,
        pub(super) method: Box<dyn Expression>,
        pub(super) headers: Box<dyn Expression>,
        pub(super) body: Box<dyn Expression>,
    }

    impl FunctionExpression for HttpRequestFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let url = self.url.resolve(ctx)?;
            let method = self.method.resolve(ctx)?;
            let headers = self.headers.resolve(ctx)?;
            let body = self.body.resolve(ctx)?;

            // block_in_place runs the HTTP request synchronously
            // without blocking Tokio's async worker threads.
            // This temporarily moves execution to a blocking-compatible thread.
            task::block_in_place(|| {
                if let Ok(handle) = Handle::try_current() {
                    handle.block_on(async { http_request(&url, &method, headers, &body).await })
                } else {
                    let runtime = runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("tokio runtime creation failed");
                    runtime
                        .block_on(async move { http_request(&url, &method, headers, &body).await })
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
        ]
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let url = arguments.required("url");
        let method = arguments.optional("method").unwrap_or_else(|| expr!("get"));
        let headers = arguments.optional("headers").unwrap_or_else(|| expr!({}));
        let body = arguments.optional("body").unwrap_or_else(|| expr!(""));

        Ok(HttpRequestFn {
            url,
            method,
            headers,
            body,
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
        };

        let result = execute_http_request(&func);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid header key"));
    }
}
