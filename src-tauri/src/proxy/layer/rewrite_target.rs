use std::convert::Infallible;

use rama::http::header;
use rama::http::{Request, Response};
use rama::layer::Layer;
use rama::service::Service;

/// 反向代理改写层：把 origin-form 请求改写为指向固定上游的 absolute-form，
/// 并把 `Host` 头改成上游 authority，然后透传给 inner（录制 + 转发管道）。
///
/// 入站请求形如 `GET /api?x=1`（Host 指向本地监听地址），改写后变为
/// `GET http(s)://upstream/api?x=1`，使下游的 rama HTTP 客户端按绝对地址
/// 直连上游；`target_protocol` 由 scheme 决定，https 走 TLS。
#[derive(Clone)]
pub(crate) struct RewriteTargetLayer {
    scheme: String,
    authority: String,
}

impl RewriteTargetLayer {
    pub(crate) fn new(scheme: String, authority: String) -> Self {
        Self { scheme, authority }
    }
}

impl<S> Layer<S> for RewriteTargetLayer {
    type Service = RewriteTargetService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RewriteTargetService {
            inner,
            scheme: self.scheme.clone(),
            authority: self.authority.clone(),
        }
    }
}

pub(crate) struct RewriteTargetService<S> {
    inner: S,
    scheme: String,
    authority: String,
}

impl<S: Clone> Clone for RewriteTargetService<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            scheme: self.scheme.clone(),
            authority: self.authority.clone(),
        }
    }
}

impl<S> Service<Request> for RewriteTargetService<S>
where
    S: Service<Request, Output = Response, Error = Infallible> + Clone + Send + 'static,
{
    type Output = Response;
    type Error = Infallible;

    async fn serve(&self, req: Request) -> Result<Response, Infallible> {
        let (mut parts, body) = req.into_parts();

        // 请求目标：origin-form 为 `/path?query`，absolute-form 也有同样的 path+query。
        let target = parts.uri.request_target().into_owned();
        if let Ok(uri) = format!("{}://{}{}", self.scheme, self.authority, target).parse() {
            parts.uri = uri;
        }
        if let Ok(value) = header::HeaderValue::from_str(&self.authority) {
            parts.headers.insert(header::HOST, value);
        }

        self.inner.serve(Request::from_parts(parts, body)).await
    }
}
