use std::convert::Infallible;
use std::sync::Arc;

use rama::Layer;
use rama::Service;
use rama::error::BoxError;
use rama::http::layer::map_response_body::MapResponseBodyLayer;
use rama::http::layer::remove_header::{RemoveRequestHeaderLayer, RemoveResponseHeaderLayer};
use rama::http::layer::trace::TraceLayer;
use rama::http::{Request, Response};
use rama::layer::ConsumeErrLayer;
use rama::service::service_fn;

use crate::config::ReverseProxyRule;
use crate::proxy::ext::RequestExt;
use crate::proxy::layer::authz::ExtAuthzLayer;
use crate::proxy::layer::rewrite_target::RewriteTargetLayer;
use crate::proxy::layer::script::ScriptLayer;
use crate::proxy::layer::traffic_record::TrafficRecorderLayer;
use crate::proxy::state::State;
use crate::proxy::throttle::ThrottleLayer;

/// 反向代理的上游转发：**直连**固定目标，忽略应用内的上游代理/系统代理设置。
/// 录制、DB 持久化、AI 检测与错误转 Infallible 由 [`TrafficRecorderLayer`] 处理。
async fn forward_direct(req: Request) -> Result<Response, BoxError> {
    let state: State = req.ext();
    state.direct_client().serve(req).await
}

/// 构建一条反向代理的服务管道：
/// 改写目标（origin-form → 固定上游 absolute-form）→ 录制/DB/事件 → 直连转发。
///
/// 与正向 MITM 管道（[`super::mitm::new_http_mitm_proxy`]）的区别：
/// 不含 `direct_reply_layer`（反向代理入站本就是 origin-form），
/// 且转发用直连客户端而非跟随上游代理。
/// 脚本/授权层位于改写层之后：请求先被改写为上游 absolute-form + 上游 Host，
/// 因此脚本与授权按**上游 host** 匹配（与正向 MITM 按目标 host 匹配一致）。
pub(crate) fn new_reverse_proxy(
    rule: &ReverseProxyRule,
) -> impl Service<Request, Output = Response, Error = Infallible> + Clone {
    let rewrite = RewriteTargetLayer::new(
        rule.upstream_scheme_normalized().to_string(),
        rule.upstream_authority(),
    );

    Arc::new(
        (
            rewrite,
            MapResponseBodyLayer::new_boxed_streaming_body(),
            TraceLayer::new_for_http(),
            ConsumeErrLayer::default(),
            RemoveResponseHeaderLayer::hop_by_hop(),
            RemoveRequestHeaderLayer::hop_by_hop(),
            // 授权先于脚本：被拒的请求在此短路，不进入 ScriptLayer / 录制 / 转发。
            ExtAuthzLayer,
            ScriptLayer,
            TrafficRecorderLayer,
            ThrottleLayer,
        )
            .into_layer(service_fn(forward_direct)),
    )
}
