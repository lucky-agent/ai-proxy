## Why

`resend.rs` 中 `collect_body` 调用处 match 分支类型不一致：`Ok(bytes)` 返回 `Bytes`，`Err(err)` 返回 `Result<_, String>`，编译器无法推断 `resp_bytes` 的类型，导致 `E0308` 编译错误。

## What Changes

- 将 `resend.rs` 中 `collect_body().await` 的 match 替换为 `map_err` + `?`，统一错误类型为 `String` 后向上传播

## Capabilities

### New Capabilities

无

### Modified Capabilities

无（纯 bug 修复，不改变 spec 级行为）

## Impact

- `src-tauri/src/commands/resend.rs`：第 97-100 行 match 表达式
