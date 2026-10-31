//! 网络节流（模拟 2G / 3G / 4G 弱网），按域名规则生效。
//!
//! 规则见 [`ThrottleConfig::params_for`]：总开关 + 自上而下命中第一条，`*` 即全局。
//! 带宽按**每个请求 / 每条隧道**独立计算（rama 的 per-conn 令牌桶），不在连接之间共享：
//! 一个大下载不会拖慢同域名下的其他请求，每个请求都按档位速度跑。
//!
//! - **可见的 HTTP 请求**（明文 HTTP、MITM 解密的 HTTPS、反向代理）：[`ThrottleLayer`]
//!   按请求 host 匹配规则，转发前等待延迟，并给请求体（上行）/ 响应体（下行）各套一个限速桶。
//! - **未解密的 CONNECT 隧道**：没有请求层，按隧道目标 host 匹配，建立前延迟一次，
//!   字节流用 [`ThrottledIo`] 限速（[`throttle_tunnel`]）。
//!
//! 配置在每个请求 / 每条隧道开始时读取，保存后对新请求立即生效，无需重启代理；
//! 已在传输中的响应体与已建立的隧道沿用开始时的档位。

use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use bytes::Bytes;
use rama::Layer;
use rama::Service;
use rama::error::BoxError;
use rama::http::body::{Frame, SizeHint, StreamingBody};
use rama::http::{Body, Request, Response, header};
use rama::net::stream::layer::{ThrottleMode, ThrottledIo};
use rama::utils::rate::{Acquire, Rate, RateLimiter};
use tokio::time::Sleep;

use crate::config::ThrottleParams;
use crate::proxy::ext::RequestExt;
use crate::proxy::state::State;

/// 1 kbps = 1000 bit/s = 125 B/s
const BYTES_PER_KBPS: u64 = 125;

/// kbps → (字节速率, 突发容量)；0 表示该方向不限速。
/// 突发容量取 1/4 秒的字节数，避免刚开始时一口气放行整秒的数据。
fn rate_of(kbps: u64) -> Option<(Rate, u64)> {
    let rate = Rate::try_new(kbps.checked_mul(BYTES_PER_KBPS)?, Duration::from_secs(1))?;
    Some((rate, (rate.units() / 4).max(1)))
}

/// 单条隧道的限速模式（每条连接独立一个桶）。
fn mode(kbps: u64) -> Option<ThrottleMode> {
    rate_of(kbps).map(|(rate, burst)| ThrottleMode::per_conn_with_burst(rate, burst))
}

async fn sleep_latency(params: &ThrottleParams) {
    if params.latency_ms > 0 {
        tokio::time::sleep(Duration::from_millis(params.latency_ms)).await;
    }
}

/// 未解密隧道：按目标 host 匹配规则，命中则先等待一次延迟，再给字节流套上限速。
/// 读方向 = 客户端 → 上游（上行），写方向 = 上游 → 客户端（下行）。
pub(crate) async fn throttle_tunnel<IO>(
    state: &State,
    host: Option<&str>,
    io: IO,
) -> ThrottledIo<IO> {
    let params = host.and_then(|h| state.settings().throttle.params_for(h));
    let io = ThrottledIo::new(io);
    let Some(params) = params else {
        return io;
    };
    sleep_latency(&params).await;
    io.maybe_with_read_mode(mode(params.upload_kbps))
        .maybe_with_write_mode(mode(params.download_kbps))
}

/// HTTP 请求节流层：按请求 host 匹配规则，施加延迟并限速请求体 / 响应体。
/// 放在 `TrafficRecorderLayer` 内侧，使流量日志里的耗时包含这段延迟。
#[derive(Clone, Default)]
pub(crate) struct ThrottleLayer;

impl<S> Layer<S> for ThrottleLayer {
    type Service = ThrottleService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ThrottleService { inner }
    }
}

#[derive(Clone)]
pub(crate) struct ThrottleService<S> {
    inner: S,
}

/// 请求目标 host：absolute-form 取 URI，否则取 Host 头（去端口）。
fn request_host(req: &Request) -> Option<String> {
    if let Some(host) = req.uri().host_str() {
        return Some(host.into_owned());
    }
    let value = req.headers().get(header::HOST)?.to_str().ok()?;
    let authority: rama::net::uri::Uri = format!("http://{value}").parse().ok()?;
    authority.host_str().map(|h| h.into_owned())
}

impl<S> Service<Request> for ThrottleService<S>
where
    S: Service<Request, Output = Response>,
{
    type Output = Response;
    type Error = S::Error;

    async fn serve(&self, req: Request) -> Result<Response, Self::Error> {
        let params = req.try_ext::<State>().and_then(|state| {
            let host = request_host(&req)?;
            state.settings().throttle.params_for(&host)
        });
        let Some(params) = params else {
            return self.inner.serve(req).await;
        };

        sleep_latency(&params).await;
        let req = req.map(|body| ThrottledBody::wrap(body, params.upload_kbps));
        let resp = self.inner.serve(req).await?;
        Ok(resp.map(|body| ThrottledBody::wrap(body, params.download_kbps)))
    }
}

/// 单次放行上限：1/10 秒的字节数，最多 16 KiB（与 rama `ThrottledIo` 默认值一致），保证节奏平滑。
fn quantum(rate: Rate, burst: u64) -> u64 {
    (rate.units() / 10).clamp(1, 16 * 1024).min(burst)
}

/// 按字节限速的 body：数据帧按令牌桶授予的额度切片放行，trailers 原样透传。
/// 每个 body 独立一个桶（不与其他请求共享）。
struct ThrottledBody {
    inner: Body,
    limiter: RateLimiter,
    quantum: u64,
    sleep: Option<Pin<Box<Sleep>>>,
    /// 已从上游取出、尚未放行的数据
    pending: Bytes,
}

impl ThrottledBody {
    fn wrap(body: Body, kbps: u64) -> Body {
        let Some((rate, burst)) = rate_of(kbps) else {
            return body;
        };
        Body::new(Self {
            inner: body,
            limiter: RateLimiter::new(rate, burst),
            quantum: quantum(rate, burst),
            sleep: None,
            pending: Bytes::new(),
        })
    }

    /// 等待令牌桶允许放行下一片，返回本次放行的字节数。
    fn poll_grant(&mut self, cx: &mut Context<'_>) -> Poll<usize> {
        loop {
            if let Some(sleep) = self.sleep.as_mut() {
                ready!(sleep.as_mut().poll(cx));
                self.sleep = None;
            }
            let want = (self.pending.len() as u64).min(self.quantum).max(1);
            match self.limiter.try_acquire(want) {
                Acquire::RetryAt(at) => {
                    self.sleep = Some(Box::pin(tokio::time::sleep_until(
                        self.limiter.deadline(at),
                    )));
                }
                // Never 不会发生（want 已被 quantum 夹到 burst 以内），兜底直接放行
                Acquire::Granted | Acquire::Never => return Poll::Ready(want as usize),
            }
        }
    }
}

impl StreamingBody for ThrottledBody {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = &mut *self;
        loop {
            if !this.pending.is_empty() {
                let n = ready!(this.poll_grant(cx)).min(this.pending.len());
                return Poll::Ready(Some(Ok(Frame::data(this.pending.split_to(n)))));
            }
            match ready!(Pin::new(&mut this.inner).poll_frame(cx)) {
                Some(Ok(frame)) => match frame.into_data() {
                    Ok(data) if data.is_empty() => continue,
                    Ok(data) => this.pending = data,
                    Err(frame) => return Poll::Ready(Some(Ok(frame))),
                },
                other => return Poll::Ready(other),
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.pending.is_empty() && self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        let mut hint = self.inner.size_hint();
        let extra = self.pending.len() as u64;
        hint.set_lower(hint.lower() + extra);
        if let Some(upper) = hint.upper() {
            hint.set_upper(upper + extra);
        }
        hint
    }
}
