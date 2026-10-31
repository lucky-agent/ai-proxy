use std::sync::Arc;
use std::time::Duration;

use rama::error::BoxError;
use rama::extensions::ExtensionsRef;
use rama::http::client::EasyHttpWebClient;
use rama::http::layer::decompression::DecompressionLayer;
use rama::http::layer::map_response_body::MapResponseBodyLayer;
use rama::http::layer::timeout::{ResponseBodyTimeoutLayer, TimeoutLayer};
use rama::http::{Request, Response, StatusCode, Version};
use rama::layer::Layer;
use rama::net::AuthorityInputExt;
use rama::net::address::ProxyAddress;
use rama::net::client::{ProxyAddressLayer, ProxyRoute, SystemProxyLayer};
use rama::rt::Executor;
use rama::service::BoxService;
use rama::service::Service;
use rama::tls::client::{ServerVerifyMode, TlsClientConfig};

use super::ext::RequestExt;
use super::state::State;
use crate::utils::proxy_rules::UpstreamRules;

/// Pure upstream forwarding — no recording or error handling.
/// Traffic recording, DB persistence, AI pipeline, and error-to-Infallible
/// conversion are handled by [`super::layer::traffic_record::TrafficRecorderLayer`].
pub(crate) async fn forward_to_upstream(req: Request) -> Result<Response, BoxError> {
    log::info!(
        "MITM request: {} {} ({:?})",
        req.method(),
        req.uri(),
        req.version()
    );

    let state: State = req.ext();
    // RwLockReadGuard from state.settings() is dropped before the await point,
    // keeping the future Send-compatible.
    state.upstream_client().serve(req).await
}

pub(crate) fn build_upstream_service(
    upstream_proxy: Option<ProxyAddress>,
    follow_system: bool,
    rules: Option<Arc<UpstreamRules>>,
    self_listen_port: u16,
    skip_tls_verify: bool,
) -> BoxService<Request, Response, BoxError> {
    // 代理地址来自运行时配置，不参与缓存：只把 client 按 skip_tls_verify 缓存两份，
    // 每次调用外挂一层 ProxyAddressLayer（往 extensions 写 ProxyRoute）。
    // 连接池的分组标识 HttpConnIdentifier 已包含所选路由，
    // 因此同一个池不会把不同代理（或代理与直连）的连接串用。
    //
    // 跟随系统代理时，在显式 ProxyAddressLayer 之下叠一层 SystemProxyLayer：
    // 显式地址优先，无显式地址则按请求读取并跟随 OS 系统代理（内部自带缓存与刷新）。
    // SelfProxyGuardLayer 再把「指向自身监听端口」的路由改回直连，避免回环；
    // 同一层里按包含/排除规则把不该走代理的目标改回直连（放在 SystemProxyLayer 之下，
    // 手动与跟随系统两种模式都生效）。
    let guard = SelfProxyGuardLayer::new(self_listen_port, rules);
    let client = cached_client(skip_tls_verify);
    if follow_system {
        ProxyAddressLayer::maybe(upstream_proxy)
            .into_layer(system_proxy_layer().into_layer(guard.into_layer(client)))
            .boxed()
    } else {
        ProxyAddressLayer::maybe(upstream_proxy)
            .into_layer(guard.into_layer(client))
            .boxed()
    }
}

/// 缓存的 `SystemProxyLayer`：克隆共享内部快照缓存，避免每个请求都重建导致冷读系统代理
/// （Linux 上会反复 spawn gsettings/kioslaverc 子进程）。
fn system_proxy_layer() -> SystemProxyLayer {
    use std::sync::OnceLock;

    static LAYER: OnceLock<SystemProxyLayer> = OnceLock::new();
    // 读取失败时回落直连（fail-open），而不是让每个上游请求都失败。
    LAYER
        .get_or_init(|| {
            SystemProxyLayer::new()
                .with_load_error_sink(rama::error_sink::TracingErrorSink::default())
        })
        .clone()
}

/// 回环守卫：当上游代理路由指向本代理自身的监听端口（loopback + 同端口）时，
/// 追加一个 `ProxyRoute::Direct`，按「最新插入优先」语义覆盖，避免转发回自己造成死循环。
///
/// 仅覆盖 loopback 场景（127.0.0.1 / ::1 / localhost）；系统代理指向本机 LAN IP
/// 的自引用不会被拦截（罕见配置，留待后续按需处理）。
///
/// 同时执行上游代理的包含/排除规则：已选定代理路由、但目标主机按规则应直连时，
/// 同样追加 `ProxyRoute::Direct`。
#[derive(Clone)]
struct SelfProxyGuardLayer {
    listen_port: u16,
    rules: Option<Arc<UpstreamRules>>,
}

impl SelfProxyGuardLayer {
    const fn new(listen_port: u16, rules: Option<Arc<UpstreamRules>>) -> Self {
        Self { listen_port, rules }
    }
}

impl<S> Layer<S> for SelfProxyGuardLayer {
    type Service = SelfProxyGuardService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SelfProxyGuardService {
            inner,
            listen_port: self.listen_port,
            rules: self.rules.clone(),
        }
    }
}

#[derive(Clone)]
struct SelfProxyGuardService<S> {
    inner: S,
    listen_port: u16,
    rules: Option<Arc<UpstreamRules>>,
}

impl<S> Service<Request> for SelfProxyGuardService<S>
where
    S: Service<Request>,
{
    type Output = S::Output;
    type Error = S::Error;

    async fn serve(&self, req: Request) -> Result<Self::Output, Self::Error> {
        let extensions = req.extensions();
        let Some(addr) = extensions
            .get_ref::<ProxyRoute>()
            .and_then(ProxyRoute::proxy_address)
        else {
            // 未选定代理（直连）：无需判定
            return self.inner.serve(req).await;
        };

        if addr.address.host.is_loopback() && addr.address.port == self.listen_port {
            log::debug!(
                "upstream proxy route points at own listen port {}; forcing direct",
                self.listen_port
            );
            extensions.insert(ProxyRoute::Direct);
        } else if let Some(rules) = &self.rules
            && let Some(host) = req.authority().map(|a| a.host)
        {
            let decision = rules.decide(&host.to_str());
            if !decision.use_proxy() {
                log::debug!("upstream proxy rules route {host} direct: {decision:?}");
                extensions.insert(ProxyRoute::Direct);
            }
        }
        self.inner.serve(req).await
    }
}

/// 上游 client（含超时/解压/流标准化），按 `skip_tls_verify` 缓存，首次调用时构建。
fn cached_client(skip_tls_verify: bool) -> BoxService<Request, Response, BoxError> {
    use std::sync::OnceLock;

    static CACHE: [OnceLock<BoxService<Request, Response, BoxError>>; 2] =
        [OnceLock::new(), OnceLock::new()];

    let slot = &CACHE[usize::from(skip_tls_verify)];
    if let Some(svc) = slot.get() {
        return svc.clone();
    }

    // 跳过 TLS 验证（不安全，仅用于测试）
    let tls_config = if skip_tls_verify {
        TlsClientConfig::default_http().with_server_verify(ServerVerifyMode::Disable)
    } else {
        TlsClientConfig::default_http()
    };

    // with_proxy_support 装的是 HttpProxyConnector::optional：
    // 没有 ProxyRoute 时直接回落直连，所以无需再区分「带代理/不带代理」两种 client。
    let client = EasyHttpWebClient::connector_builder()
        .with_default_transport_connector()
        .with_default_dns_connector()
        .with_tls_proxy_support_using_rustls()
        .with_proxy_support()
        .with_tls_support_using_rustls_and_default_http_version(tls_config, Version::HTTP_11)
        .with_default_http_connector(Executor::default())
        .with_default_connection_pool()
        .build_client();

    let svc = (
        MapResponseBodyLayer::new_boxed_streaming_body(),
        DecompressionLayer::new().with_insert_accept_encoding_header(false),
        // 300s overall timeout as safety net against hung requests
        TimeoutLayer::with_status_code(StatusCode::GATEWAY_TIMEOUT, Duration::from_secs(300)),
        // 60s per-chunk timeout: kills dead connections quickly while
        // streaming AI responses can run arbitrarily long under the 300s cap
        ResponseBodyTimeoutLayer::new(Duration::from_secs(60)),
    )
        .into_layer(client)
        .boxed();

    // get_or_init：多个请求竞争时只有第一个构建，其余等待后复用。
    slot.get_or_init(|| svc).clone()
}

/// extAuthz 授权检查用的**直连**客户端：不上游代理、不内置超时（超时由调用方
/// 按规则 `timeout_ms` 用 `tokio::time::timeout` 施加）。TLS 校验姿态与上游一致
/// （skip-verify，适配自签名测试环境）。
pub(crate) fn authz_client() -> BoxService<Request, Response, BoxError> {
    use std::sync::OnceLock;

    static CLIENT: OnceLock<BoxService<Request, Response, BoxError>> = OnceLock::new();

    CLIENT
        .get_or_init(|| {
            // 跳过 TLS 验证（与上游 client 同一种信任假设）
            let tls_config =
                TlsClientConfig::default_http().with_server_verify(ServerVerifyMode::Disable);

            // 直连：无 ProxyRoute 注入，`with_proxy_support` 的 optional 连接器回落直连，
            // 不会经上游代理转发。TLS 校验姿态与上游 client 一致（skip-verify）。
            let client = EasyHttpWebClient::connector_builder()
                .with_default_transport_connector()
                .with_default_dns_connector()
                .with_tls_proxy_support_using_rustls()
                .with_proxy_support()
                .with_tls_support_using_rustls_and_default_http_version(
                    tls_config,
                    Version::HTTP_11,
                )
                .with_default_http_connector(Executor::default())
                .with_default_connection_pool()
                .build_client();

            (
                MapResponseBodyLayer::new_boxed_streaming_body(),
                DecompressionLayer::new().with_insert_accept_encoding_header(false),
            )
                .into_layer(client)
                .boxed()
        })
        .clone()
}
