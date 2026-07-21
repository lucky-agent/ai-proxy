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
