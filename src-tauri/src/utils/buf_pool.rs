use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use rama::error::BoxError;
use rama::http::Body;
use rama::http::body::CollectOptions;
use rama::http::body::util::BodyExt;

/// body 收集上限（16 MiB）：超过则停止缓冲，剩余部分保持可转发。
pub(crate) const BODY_CAPTURE_LIMIT: usize = 16 * 1024 * 1024;

/// [`collect_body`] 的收集结果。不再返回 `Result`——三种结果各自携带必要信息，
/// 调用方按需 match。
pub(crate) enum CollectedBody {
    /// body 在上限内完整收集。
    Full(Bytes),
    /// 超过 [`BODY_CAPTURE_LIMIT`]：`prefix` 为已读前缀（供日志/展示），
    /// `body` 为重组后的完整 body（前缀 + 未读余量），可原样转发。
    Capped { prefix: Bytes, body: Body },
    /// 流读取中途出错：`prefix` 为出错前已读前缀（供尽力展示），
    /// `error` 为原始错误信息。
    Error { prefix: Bytes, error: BoxError },
}

/// 收集 Body 所有 chunk（带大小上限）。超限时停止缓冲，流错误时保留已读前缀。
/// 调用方对所有三种分支做显式处理。
pub(crate) async fn collect_body(body: Body) -> CollectedBody {
    match body
        .collect_with(CollectOptions::new().with_max_size(BODY_CAPTURE_LIMIT))
        .await
    {
        Ok(collected) => CollectedBody::Full(collected.to_bytes()),
        Err(err) if err.is_cap_reached() => {
            let prefix = err.bytes_read();
            let body = err
                .into_full_body()
                .expect("cap reached implies forwardable remainder");
            CollectedBody::Capped { prefix, body }
        }
        Err(err) => {
            let prefix = err.bytes_read();
            let error: BoxError = err.into();
            CollectedBody::Error { prefix, error }
        }
    }
}

/// 追加 chunk 到累积缓冲，总量超过 [`BODY_CAPTURE_LIMIT`] 后停止（DB 只存前缀）。
/// 截断处按 UTF-8 字符边界回退，避免 panic。
pub(crate) fn push_capped(buf: &mut String, chunk: &str) {
    let room = BODY_CAPTURE_LIMIT.saturating_sub(buf.len());
    if room == 0 {
        return;
    }
    if chunk.len() <= room {
        buf.push_str(chunk);
    } else {
        let mut end = room;
        while !chunk.is_char_boundary(end) {
            end -= 1;
        }
        buf.push_str(&chunk[..end]);
    }
}

/// Body 观测器 trait——实现此 trait 即可接入 [`observe_body`]，
/// 无需自行处理 `Arc`/`Mutex`/drop-安全等并发样板。
pub(crate) trait BodyObserver: Send + 'static {
    /// 每个 chunk 到达时回调。
    fn on_chunk(&mut self, bytes: &Bytes);
    /// 流正常结束或 body 被 drop（客户端断开）时回调，保证只调一次。
    fn on_eos(&mut self);
}

/// 逐 chunk 观测 + on_drop 安全兜底骨架：将 body 数据流包裹一层，
/// 每个 chunk 到达时调 `observer.on_chunk`；流正常结束或 body 被 drop 时
/// 调 `observer.on_eos`——由 [`Ordering::AcqRel`] 原子标志保证只执行一次。
///
/// 内部维护 `Arc<Mutex<Observer>>` 以在 stream future 与 drop guard 间共享，
/// 调用侧无需关心并发细节。
pub(crate) fn observe_body(body: Body, observer: impl BodyObserver) -> Body {
    let inner = Arc::new((Mutex::new(observer), AtomicBool::new(false)));

    let stream_state = inner.clone();
    let drop_state = inner;

    let stream = rama::futures::stream::unfold(
        (body.into_data_stream(), stream_state),
        move |(mut ds, st)| async move {
            match rama::futures::StreamExt::next(&mut ds).await {
                Some(Ok(bytes)) => {
                    st.0.lock().expect("observe_body").on_chunk(&bytes);
                    Some((Ok(bytes), (ds, st)))
                }
                Some(Err(err)) => Some((Err(err), (ds, st))),
                None => {
                    if !st.1.swap(true, Ordering::AcqRel) {
                        st.0.lock().expect("observe_body").on_eos();
                    }
                    None
                }
            }
        },
    );

    Body::from_stream(stream).on_drop(move || {
        if !drop_state.1.swap(true, Ordering::AcqRel) {
            drop_state.0.lock().expect("observe_body").on_eos();
        }
    })
}
