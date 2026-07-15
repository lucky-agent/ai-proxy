# AI 会话标题提取（后端）设计

日期：2026-07-15
状态：已批准

## 背景与目标

部分 AI 客户端会在会话开始时发一个「标题生成」请求，模型第一次返回的内容形如：

```json
{"title": "调整气泡背景颜色搭配"}
```

目标：后端在归一化时识别这种回复，提取 `title` 作为会话标题，通过 `AiSession`
事件透出；前端 AI 侧栏用它替代当前的 `scopeHost / sessionId` 作为会话名称显示。

已确认的需求边界：

- 标题**只用作侧栏会话名**，Meta 面板不展示
- **只看会话内第一次请求的 assistant 回复**；后续回复即使出现 title JSON 也不覆盖
- 标题生成请求在对话时间线里的气泡**保持原样显示**，不隐藏、不加标识
- 采用**后端提取**（方案 C）：单一数据源，前端只消费

## 方案取舍（记录）

| 方案 | 内容 | 结论 |
| --- | --- | --- |
| A 前端 hook 派生 | useAiSessions 解析并存 AiSessionState.title | 未选 |
| B 展示层解析 | AiSidebar 渲染时现场解析 | 未选 |
| C 后端提取 | SessionEntry 存 title，AiSession 事件透出 | **已选** |

## 设计

### 1. 标题提取纯函数（`src-tauri/src/proxy/ai/normalize.rs`）

```rust
/// 从响应 conversation 提取会话标题：
/// 第一条 assistant turn 的文本若为 {"title": "..."} JSON，返回 title。
pub(crate) fn extract_title(conv: &AiConversation) -> Option<String>
```

规则（任一步不满足即返回 `None`，不报错不打日志）：

1. 取 `conv.turns` 中第一个 `role == "assistant"` 的 turn，按序拼接其全部
   `Text` block 文本
2. `trim`；若被 ```` ```json … ``` ```` 或 ```` ``` … ``` ```` 代码栅栏包裹，先剥掉
   （部分模型会包一层）
3. `serde_json::from_str::<serde_json::Value>` 解析为 JSON **对象**
4. 取 `title` 字段，须为非空字符串 → 返回 trim 后的值

宽松匹配：不要求对象只有 `title` 一个键。误判风险仅限「会话首请求的回复恰好是
含 title 字段的 JSON 对象」，可接受。

### 2. 会话状态与写入时机（`session.rs` + `parser.rs`）

- `SessionEntry` 增加字段 `pub title: Option<String>`
- `SessionStore` 增加方法：

  ```rust
  /// 仅当会话尚无标题、且 request_id 是该会话第一个请求时，
  /// 尝试从 conv 提取标题写入。
  pub(crate) fn set_title_if_first(&mut self, session_id: &str, request_id: &str, conv: &AiConversation)
  ```

  条件：`entry.title.is_none()` 且 `entry.request_ids.first() == Some(request_id)`
  ——严格实现「只看会话第一次请求的回复」。

- 写入时机：`parser.rs::log_body_chunks` 流结束分支拿到定稿 `conv` 后。把现有
  `commit_ai_usage` 扩展为「定稿提交」（同一把 `sessions` 锁内完成）：
  1. `set_title_if_first(...)`
  2. `usage` 为 `Some` 时 `add_usage(...)`
  3. 推送一次 `AiSession`（带 title）

  注意：`usage` 为 `None` 时不再提前 return，标题分支照走。

流式（SSE 定稿）与非流式（完整 body 解析）都在此汇合，天然覆盖两种响应模式。

### 3. 事件透出（`events.rs` + 前端类型）

- `ProxyEvent::AiSession` 增加字段：

  ```rust
  #[serde(skip_serializing_if = "Option::is_none")]
  title: Option<String>,
  ```

  两处构造点（`parser.rs::emit_ai_session` 与定稿提交处）均从 `entry.title.clone()` 带出。

- 前端 `src/types/proxy.ts`：`ai_session` 事件增加 `title?: string`
- 前端 `src/types/ai.ts`：`AiSessionState` 增加 `title?: string`
- `src/hooks/useAiSessions.ts` 的 `ai_session` 分支写入
  `title: event.title ?? prev?.title`（后端某次事件缺字段时不闪回）；
  `ai_normalized` 先到时的占位会话 `title` 留空

### 4. 展示（`src/features/ai-view/AiSidebar.tsx`）

会话头名称行（现第 101–103 行）改为：

```tsx
{session.title || session.scopeHost || session.sessionId}
```

其余一律不动：对话气泡时间线原样、Meta 面板原样、无新增 i18n 文案。

## 边界情况

- 首请求回复不是 title JSON（正常对话）→ `title` 保持 `None`，侧栏回退显示 host（现状）
- 会话被 LRU 淘汰 → 标题随 `SessionEntry` 消失；新流量重建会话后按「新会话第一次
  回复」重新判定
- 前端右键删除会话/请求是纯前端行为，不影响后端标题状态
- 现实限制：若客户端的标题生成请求走**前缀分组**，其 messages 通常不是主对话的
  前缀，会被分成独立会话（该独立会话反而以标题命名）；只有 header 分组或标题请求
  恰为会话首请求时主会话才能命名。与已确认的「只看第一次回复」规则一致，不做额外兜底

## 测试

- `extract_title` / `set_title_if_first` 为纯逻辑，若后端已有 `#[cfg(test)]` 单测
  模式则补对应单测；否则以 `cargo check` 通过 + 手动流量验证为准
- 前端无测试设施：`bun run build:vite` 通过 + 手动验证侧栏显示与回退链
