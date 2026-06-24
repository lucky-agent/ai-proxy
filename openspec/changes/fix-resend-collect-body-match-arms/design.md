## Context

`resend.rs:97-100` 对 `collect_body().await` 使用 `match` 解构，但两个分支返回不同类型：
- `Ok(bytes) => bytes` 类型为 `Bytes`
- `Err(err) => Err(...)` 类型为 `Result<_, String>`

Rust 要求 match 所有分支返回同一类型，编译失败。

## Goals / Non-Goals

**Goals:**
- 修复类型不匹配，让 `resp_bytes` 正确绑定为 `Bytes`

**Non-Goals:**
- 不改变 `collect_body` 的签名
- 不改变调用方 `resend_request` 的返回类型

## Decisions

**选择 `map_err` + `?` 替代 match**。`collect_body` 返回 `Result<Bytes, BoxError>`，通过 `map_err` 将 `BoxError` 转为 `String` 后用 `?` 传播错误，`resp_bytes` 直接拿到 `Bytes`。

```rust
let resp_bytes = crate::utils::buf_pool::collect_body(resp.into_body())
    .await
    .map_err(|e| format!("resend failed: {e:?}"))?;
```

无备选方案——这是一处明显的类型错误，修复方式唯一。

## Risks / Trade-offs

无风险。修复后代码逻辑与原始意图一致：成功时使用 `Bytes`，失败时向上传播 `Err(String)`。
