# Design: BodyView 基于 content_type 格式解析

## 方案

### BodyView 变更

新增 `contentType?: string` prop。在 `auto` 模式下的格式判断逻辑修改为：

1. 若 `contentType` 存在，按以下规则映射：
   - 包含 `json` → JSON 格式
   - 包含 `xml` → XML 格式
   - 包含 `html` → HTML 格式
   - 其他 → 回退到现有启发式
2. 若 `contentType` 不存在，保持现有启发式（JSON.parse → `<` 前缀 → plaintext）

### 调用方变更

- `ResponsePanel.tsx:127` — `BodyView body={entry.responseBody}` → 增加 `contentType={entry.responseContentType}`
- `RequestPanel.tsx:103` — `BodyView body={entry.requestBody}` → 增加 `contentType={entry.requestContentType}`

### 不涉及

- 不改变手动选择的 format（JSON/XML/HTML/Text 选项照常工作）
- 不改变 `FormDataView` 的 `contentType` 逻辑
- 不改变后端代码