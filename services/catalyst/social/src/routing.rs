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
            axum::routing::$name(erase(handler))
        }
    )*};
}

method!(any, delete, get, patch, post, put);
