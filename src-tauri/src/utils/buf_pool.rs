use bytes::Bytes;
use rama::error::BoxError;
use rama::http::Body;
use rama::http::body::CollectOptions;
use rama::http::body::util::BodyExt;

/// body 收集上限（16 MiB）：超过则停止缓冲，剩余部分保持可转发。
pub(crate) const BODY_CAPTURE_LIMIT: usize = 16 * 1024 * 1024;

/// [`collect_body`] 的收集结果。
pub(crate) enum CollectedBody {
    /// body 在上限内完整收集。
    Full(Bytes),
    /// 超过 [`BODY_CAPTURE_LIMIT`]：`prefix` 为已读前缀（供日志/展示），
    /// `body` 为重组后的完整 body（前缀 + 未读余量），可原样转发。
    Capped { prefix: Bytes, body: Body },
}

/// 收集 Body 所有 chunk（带大小上限），返回 `Bytes`（零拷贝 freeze）。
/// 超过上限时不再缓冲，返回 [`CollectedBody::Capped`] 供调用方原样转发。
pub(crate) async fn collect_body(body: Body) -> Result<CollectedBody, BoxError> {
    match body
        .collect_with(CollectOptions::new().with_max_size(BODY_CAPTURE_LIMIT))
        .await
    {
        Ok(collected) => Ok(CollectedBody::Full(collected.to_bytes())),
        Err(err) if err.is_cap_reached() => {
            let prefix = err.bytes_read();
            let body = err
                .into_full_body()
                .expect("cap reached implies forwardable remainder");
            Ok(CollectedBody::Capped { prefix, body })
        }
        Err(err) => Err(err.into()),
    }
}
