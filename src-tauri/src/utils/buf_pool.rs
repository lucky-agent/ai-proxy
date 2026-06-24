use bytes::Bytes;
use rama::error::{BoxError, ErrorContext};
use rama::http::Body;
use rama::http::body::util::BodyExt;

/// 收集 Body 所有 chunk，返回 `Bytes`（零拷贝 freeze）。

pub(crate) async fn collect_body(body: Body) -> Result<Bytes, BoxError> {
    let collected = body.collect().await.context("failed to collect body")?;
    Ok(collected.to_bytes())
}
