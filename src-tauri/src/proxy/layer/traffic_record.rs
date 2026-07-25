use std::convert::Infallible;

use rama::error::BoxError;
use rama::extensions::ExtensionsRef;
use rama::http::{Body, Request, Response, StatusCode};
use rama::layer::Layer;
use rama::service::Service;

use crate::proxy::ctx::ProxyCtx;
use crate::proxy::error_response;
use crate::proxy::events::ProxyEvent;
use crate::proxy::ext::{CachedRequestBody, RequestExt};
use crate::proxy::record;
use crate::proxy::state::{self, State};
use crate::utils::{buf_pool, date};

/// Traffic recording layer: request event emission, DB persistence, AI pipeline,
/// response body observation, and error handling.
///
/// Reads [`CachedRequestBody`] from extensions (written by [`super::script::ScriptLayer`])
/// to avoid re-collecting the body. When absent, collects the body itself.
///
/// This is the **error boundary**: inner service errors ([`BoxError`]) are
/// converted to [`Infallible`] by recording the error (event + DB flag) and
/// returning a `502 Internal Server Error`.
#[derive(Clone, Default)]
pub(crate) struct TrafficRecorderLayer;

impl<S> Layer<S> for TrafficRecorderLayer {
    type Service = TrafficRecorderService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TrafficRecorderService { inner }
    }
}

pub(crate) struct TrafficRecorderService<S> {
    inner: S,
}

impl<S: Clone> Clone for TrafficRecorderService<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S> Service<Request> for TrafficRecorderService<S>
where
    S: Service<Request, Output = Response, Error = BoxError> + Clone + Send + 'static,
{
    type Output = Response;
    type Error = Infallible;

    async fn serve(&self, req: Request) -> Result<Response, Infallible> {
        let start_ms = req.try_ext::<state::StartTime>().map(|st| st.0);
        let state: State = req.ext();
        let (parts, body) = req.into_parts();

        // ── 读 CachedRequestBody（ScriptLayer 已收集 → 跳过重复收集）──
        let cached = parts.extensions().get_ref::<CachedRequestBody>().cloned();

        let (body_bytes, forward_body) = if let Some(CachedRequestBody(bytes)) = cached {
            // ScriptLayer 已收集；复用 body 避免不必要的 Body::from 重建。
            (bytes, Some(body))
        } else {
            match buf_pool::collect_body(body).await {
                buf_pool::CollectedBody::Full(bytes) => (bytes, None),
                buf_pool::CollectedBody::Capped { prefix, body } => {
                    log::warn!(
                        "[traffic] request body exceeds capture limit, logging {} bytes prefix only",
                        prefix.len()
                    );
                    (prefix, Some(body))
                }
                buf_pool::CollectedBody::Error { error, .. } => {
                    return Ok(error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        error.to_string(),
                    ));
                }
            }
        };

        // ── 构造上下文 + 请求侧录制 ──
        let ctx = ProxyCtx::new(
            parts.clone(),
            state.event_channel(),
            state.settings().clone(),
            start_ms,
        )
        .with_sessions(state.sessions())
        .with_db(state.db());

        record::record_request(&ctx, &body_bytes);
        log::info!(
            "[traffic] {} request logged, body={} bytes",
            ctx.request_id(),
            body_bytes.len()
        );

        // ── 转发到上游 ──
        let forward_req = Request::from_parts(
            parts,
            forward_body.unwrap_or_else(|| Body::from(body_bytes)),
        );

        // ── 转发到上游 ──
        let forward_start = date::instant_now();
        match self.inner.serve(forward_req).await {
            Ok(resp) => {
                log::info!(
                    "[traffic] {} upstream head: {} ({:?})",
                    ctx.request_id(),
                    resp.status(),
                    forward_start.elapsed()
                );
                let resp: Response = record::record_response(ctx, resp);
                Ok(resp)
            }
            Err(err) => {
                let msg = format!("{err}");
                log::error!(
                    "error proxying request [{} {}] after {:?}: {err:?}",
                    ctx.method(),
                    ctx.uri(),
                    forward_start.elapsed()
                );
                ctx.send(ProxyEvent::Error {
                    id: ctx.request_id(),
                    error: format!("{err:?}"),
                });
                if let (Some(db), Some(db_id)) = (ctx.db_ref(), ctx.db_id()) {
                    db.set_traffic_error(db_id, &msg).ok();
                }
                Ok(error_response(StatusCode::INTERNAL_SERVER_ERROR, msg))
            }
        }
    }
}
