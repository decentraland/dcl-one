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

#[cfg(any(test, feature = "test-support"))]
pub mod guard;

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
}
