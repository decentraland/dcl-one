//! Method routers whose handlers are boxed behind one handler type per state.
//!
//! axum stacks `MapFuture<MapIntoResponse<HandlerService<H, T, S>>>` on every
//! handler and proves the stack's bounds once per route at codegen. Boxing the
//! handler as [`Erased`] proves them once per state type instead; routing,
//! extraction and HEAD handling stay axum's own.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::Request;
use axum::handler::Handler;
use axum::response::Response;
use axum::routing::MethodRouter;

type BoxedResponse = Pin<Box<dyn Future<Output = Response> + Send>>;

/// A handler boxed to one type per state `S`.
pub struct Erased<S>(Arc<dyn Fn(Request, S) -> BoxedResponse + Send + Sync>);

impl<S> Clone for Erased<S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<S: Clone + Send + Sync + 'static> Handler<(), S> for Erased<S> {
    type Future = BoxedResponse;

    fn call(self, req: Request, state: S) -> Self::Future {
        (self.0)(req, state)
    }
}

/// Boxes `handler`; it is cloned per request, as axum does.
pub fn erase<H, T, S>(handler: H) -> Erased<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    Erased(Arc::new(move |req, state| {
        Box::pin(handler.clone().call(req, state))
    }))
}

macro_rules! method {
    ($($name:ident),*) => {$(
        #[doc = concat!("`axum::routing::", stringify!($name), "` over an [`erase`]d handler.")]
        pub fn $name<H, T, S>(handler: H) -> MethodRouter<S>
        where
            H: Handler<T, S>,
            T: 'static,
            S: Clone + Send + Sync + 'static,
        {
            #[allow(clippy::disallowed_methods)]
            axum::routing::$name(erase(handler))
        }
    )*};
}

method!(any, delete, get, patch, post, put);

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::{Body, Bytes};
    use axum::extract::DefaultBodyLimit;
    use axum::http::{Method, StatusCode};
    use tower::ServiceExt;

    use super::*;

    async fn send(app: Router, method: Method, body: &'static [u8]) -> (StatusCode, Bytes) {
        let request = Request::builder()
            .method(method)
            .uri("/")
            .body(Body::from(body))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        (
            status,
            axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn an_erased_get_answers_head_without_a_body() {
        let app = Router::new().route("/", get(|| async { "hello" }));
        assert_eq!(
            send(app.clone(), Method::GET, b"").await,
            (StatusCode::OK, Bytes::from("hello"))
        );
        assert_eq!(
            send(app, Method::HEAD, b"").await,
            (StatusCode::OK, Bytes::new())
        );
    }

    #[tokio::test]
    async fn an_erased_body_extractor_keeps_the_route_body_limit() {
        let app = Router::new().route(
            "/",
            post(|body: Bytes| async move { body.len().to_string() })
                .layer(DefaultBodyLimit::max(4)),
        );
        assert_eq!(
            send(app.clone(), Method::POST, b"1234").await,
            (StatusCode::OK, Bytes::from("4"))
        );
        assert_eq!(
            send(app, Method::POST, b"12345").await.0,
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    /// CI runs no clippy over this crate, so this scan keeps production routes on
    /// `crate::routing`. Items under `#[cfg(test)]`, files named `*test*` and
    /// `tests/` folders are test code and exempt.
    #[test]
    fn production_routes_do_not_use_axum_methods_directly() {
        const METHODS: [&str; 6] = ["any", "delete", "get", "patch", "post", "put"];
        fn names_a_method(after: &str) -> bool {
            let names = match after.strip_prefix('{') {
                Some(group) => group.split('}').next().unwrap_or(""),
                None => after,
            };
            names
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .take(if after.starts_with('{') {
                    usize::MAX
                } else {
                    1
                })
                .any(|name| METHODS.contains(&name))
        }
        fn visit(dir: &std::path::Path, found: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if path.is_dir() {
                    if name != "tests" {
                        visit(&path, found);
                    }
                    continue;
                }
                if !name.ends_with(".rs") || name.contains("test") || name == "routing.rs" {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                // Brace depth, the depth a skipped test item closes at, and whether
                // a `#[cfg(test)]` item has not reached its `;` or `{` yet.
                let (mut depth, mut skip_until, mut pending) = (0i64, None, false);
                for (number, line) in text.lines().enumerate() {
                    let trimmed = line.trim();
                    let delta = line.matches('{').count() as i64 - line.matches('}').count() as i64;
                    if let Some(end) = skip_until {
                        depth += delta;
                        if depth <= end {
                            skip_until = None;
                        }
                        continue;
                    }
                    if pending {
                        if delta > 0 {
                            skip_until = Some(depth);
                            pending = false;
                        } else if !trimmed.starts_with("#[")
                            && (trimmed.ends_with(';') || trimmed.ends_with('}'))
                        {
                            pending = false;
                        }
                        depth += delta;
                        continue;
                    }
                    if trimmed == "#[cfg(test)]" {
                        pending = true;
                        continue;
                    }
                    if line
                        .split_once("axum::routing::")
                        .is_some_and(|(_, after)| names_a_method(after))
                    {
                        found.push(format!("{}:{}", path.display(), number + 1));
                    }
                    depth += delta;
                }
            }
        }
        let mut found = Vec::new();
        visit(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut found,
        );
        assert!(
            found.is_empty(),
            "use crate::routing instead of axum::routing at {found:?}"
        );
    }
}
