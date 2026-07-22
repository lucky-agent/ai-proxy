use std::convert::Infallible;

use rama::extensions::ExtensionsRef;
use rama::http::{Body, Request, Response, StatusCode};
use rama::matcher::Matcher;
use rama::service::Service;

use super::super::state::ViaConnectTunnel;

/// Matches direct requests to the proxy itself.
///
/// A request is "direct" when:
/// - It is **not** from a CONNECT tunnel (no `ViaConnectTunnel` extension), AND
/// - Its URI is origin-form (non-absolute, e.g. `GET /path` rather than
///   `GET http://host/path`).
///
/// Origin-form requests that arrive outside a tunnel are clients talking to
/// the proxy server directly, not asking it to forward anywhere.
#[derive(Debug, Clone, Default)]
pub(crate) struct DirectRequestMatcher;

impl Matcher<Request> for DirectRequestMatcher {
    fn matches(&self, _ext: Option<&rama::extensions::Extensions>, req: &Request) -> bool {
        !req.uri().is_absolute() && req.extensions().get_ref::<ViaConnectTunnel>().is_none()
    }
}

/// Handler for direct requests: returns a simple 200 OK to indicate the proxy
/// is alive.
#[derive(Debug, Clone, Default)]
pub(crate) struct DirectReplyService;

impl Service<Request> for DirectReplyService {
    type Output = Response;
    type Error = Infallible;

    async fn serve(&self, _req: Request) -> Result<Response, Infallible> {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Body::from("ai-proxy is running"))
            .expect("valid status code and body for direct reply"))
    }
}

/// Convenience: build the [`HijackLayer`] that intercepts direct requests.
pub(crate) type DirectReplyLayer =
    rama::layer::HijackLayer<DirectReplyService, DirectRequestMatcher>;

pub(crate) fn direct_reply_layer() -> DirectReplyLayer {
    rama::layer::HijackLayer::new(DirectRequestMatcher, DirectReplyService)
}
