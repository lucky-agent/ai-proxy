# Proposal: BodyView 基于 content_type 格式解析

## 问题描述

`BodyView` 组件的 `format: 'auto'` 模式纯靠内容启发式检测决定渲染格式：

- 尝试 `JSON.parse` → 成功则 JSON
- 首字符 `<` → HTML
- 其他 → 纯文本

不使用 `content-type` 响应头来辅助判断，导致以下问题：
- `Content-Type: application/xml` 但 body 不以 `<` 开头时无法正确格式化为 XML
- 无法根据 `content-type` 区分 HTML/XML 等 `<` 开头的标记语言
- `Content-Type: text/plain` 的 JSON 内容不会被格式化为 JSON（这可能是好事，边界情况）

## 根因分析

`TrafficEntry` 已包含 `requestContentType` 和 `responseContentType` 字段（由 Rust 后端 `parser.rs` 从 `content-type` 头提取），但 `BodyView` 的 props 只接收 `body: string`，没有接收 `contentType`。

## 修复目标

1. `BodyView` 新增可选 `contentType?: string` prop
2. `auto` 模式下优先根据 `contentType` 判断格式，`contentType` 不可用时回退到现有启发式
3. 调用方 (`ResponsePanel`, `RequestPanel`) 传入对应的 `contentType`