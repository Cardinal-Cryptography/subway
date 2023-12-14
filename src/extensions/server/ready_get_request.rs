use std::{
    error::Error,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use crate::extensions::server::proxy_get_request::response::internal_error;
use futures::TryFutureExt;
use hyper::{
    header::{ACCEPT, CONTENT_TYPE},
    http::HeaderValue,
    Body, Method, Request, Response, Uri,
};
use jsonrpsee::types::{Id, RequestSer};
use tower::{Layer, Service};

type BoxedError = Box<dyn Error + Send + Sync + 'static>;
type PinnedFuture = Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send + 'static>>;

/// Layer that catches requests to '/ready' endpoint redirects them to rpc call `alephNode_ready`
/// and translate the response to plain get response 200 or 503.
#[derive(Debug, Clone)]
pub struct ReadyProxyLayer;

impl<S> Layer<S> for ReadyProxyLayer {
    type Service = ReadyRequest<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ReadyRequest::new(inner)
    }
}

fn ok_response() -> hyper::Response<hyper::Body> {
    hyper::Response::builder()
        .status(hyper::StatusCode::OK)
        .body(Body::empty())
        .expect("Unable to parse response body for type conversion")
}

fn bad_response() -> hyper::Response<hyper::Body> {
    hyper::Response::builder()
        .status(hyper::StatusCode::BAD_GATEWAY)
        .body(Body::empty())
        .expect("Unable to parse response body for type conversion")
}

#[derive(Debug, Clone)]
pub struct ReadyRequest<S> {
    inner: S,
}

impl<S> ReadyRequest<S>
where
    S: Service<Request<Body>, Response = Response<Body>>,
    S::Response: 'static,
    S::Error: Into<BoxedError> + 'static,
    S::Future: Send + 'static,
{
    #[inline]
    fn poll_ready_inner(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), BoxedError>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn prepare_request(req: &mut Request<Body>) {
        const RPC_METHOD_NAME: &str = "alephNode_ready";

        *req.method_mut() = Method::POST;
        // Precautionary remove the URI.
        *req.uri_mut() = Uri::from_static("/");

        // Requests must have the following headers:
        req.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        req.headers_mut()
            .insert(ACCEPT, HeaderValue::from_static("application/json"));

        // Adjust the body to reflect the method call.
        let body = Body::from(
            serde_json::to_string(&RequestSer::borrowed(&Id::Number(0), &RPC_METHOD_NAME, None))
                .expect("Valid request; qed"),
        );
        *req.body_mut() = body;
    }

    pub fn call_proxy(&mut self, mut req: Request<Body>) -> PinnedFuture {
        const METHOD_NAME: &str = "/ready";

        if req.uri().path() != METHOD_NAME {
            return Box::pin(self.inner.call(req).map_err(Into::into));
        }

        Self::prepare_request(&mut req);

        let fut = self.inner.call(req);

        let res_fut = async move {
            let res = fut.await.map_err(|err| err.into())?;

            let body = res.into_body();
            let bytes = hyper::body::to_bytes(body).await?;
            #[derive(serde::Deserialize, Debug)]
            struct RpcPayload {
                result: bool,
            }

            let response = match serde_json::from_slice::<RpcPayload>(&bytes) {
                Ok(RpcPayload { result }) if result => ok_response(),
                Ok(_) => bad_response(),
                _ => internal_error(),
            };

            Ok(response)
        };

        Box::pin(res_fut)
    }
}

impl<S> ReadyRequest<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<Request<Body>> for ReadyRequest<S>
where
    S: Service<Request<Body>, Response = Response<Body>>,
    S::Response: 'static,
    S::Error: Into<BoxedError> + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = BoxedError;
    type Future = PinnedFuture;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_ready_inner(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.call_proxy(req)
    }
}
