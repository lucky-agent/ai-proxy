## Why

在 new-request 区域发送请求后，响应面板的 Body 标签页始终为空——即使上游 API 返回了完整的响应体。问题根因是 `resend_request` Rust 命令将响应体存入了数据库，但没有通过 IPC Channel 向前端发送 `response_chunk` 事件，而前端 `useProxyEvents` 仅通过 `response_chunk` 事件写入 `responseBody`。

## What Changes

- `src-tauri/src/commands/resend.rs`: 在收集到完整响应体后，发送 `ProxyEvent::ResponseChunk` 事件到前端 Channel，确保前端 `TrafficEntry.responseBody` 被正确填充

## Capabilities

### New Capabilities

（无——这是已有功能的 bug 修复，不涉及新 capability）

### Modified Capabilities

（无——不涉及 spec 级别的行为变更）

## Impact

- **Affected code**: `src-tauri/src/commands/resend.rs`（后端，1 行添加）
- **No API changes**, **no breaking changes**
