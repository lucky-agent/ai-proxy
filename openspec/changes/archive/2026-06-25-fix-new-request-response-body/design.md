## Context

`resend_request` (Rust) 通过 rama 客户端向上游发送请求，收集完整响应体（`collect_body`），存入 SQLite 数据库，然后向前端 Channel 发送 `ProxyEvent::Response`。然而，它**没有**发送 `ProxyEvent::ResponseChunk` 事件。

前端 `useProxyEvents` hook 在 `response` 事件中写入状态码、响应头等元数据，但 **`responseBody` 仅通过 `response_chunk` 事件累加**：

```ts
case 'response_chunk': {
  const { id, chunk } = event
  // ...
  entry.responseBody += chunk
}
```

正常代理流程中，`parser::log_response` 在流式读取上游响应时发送 `ResponseChunk` 事件；但 `resend_request` 在非代理路径下运行，绕过了这个流式管道，直接 `collect_body` 收集完整响应体后只发了 `Response` 事件。

## Goals / Non-Goals

**Goals:**
- 修复 new-request 发送请求后 Body 标签页为空的问题

**Non-Goals:**
- 不修改前端逻辑
- 不改变正常代理流量（proxy）的行为
- 不引入新的 IPC 事件类型

## Decisions

**决策：在 `resend_request` 中，收集完响应体后发送一个 `ResponseChunk` 事件**，携带完整的响应体字符串。

这是最简单、最小风险的修复：

- 复用现有 Channel 事件类型，前端无需改动
- 将完整 body 作为单个 chunk 一次性发送，因为 `resend_request` 已通过 `collect_body` 获得了完整 body
- 发送位置在 `resp_bytes` 收集完成后、`Response` 事件发送之前

## Risks / Trade-offs

- **对 SSE 流式响应**：resend_request 本身是收集完整 body 后再发送单个 chunk，将来如需支持流式展示，需要在 rama 客户端层逐 chunk 处理。但这是已有限制，本次修复使 body 可展示是净改进，不引入新回归。
- **大响应体**：前端 `MAX_BODY_ACCUMULATE` = 2MB 对完整 body 同样生效，大 body 会被截断，行为与其他流量一致。
