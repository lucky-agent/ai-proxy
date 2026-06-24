## Why

当前 NewRequestView 左侧接口管理树点击 request 时，右侧只显示一个共享的编辑器实例——点击新接口会直接覆盖掉上一个接口的所有编辑状态。用户无法同时编辑多个接口，也无法在多个请求之间切换对比。需要引入多 tab 管理，使每个 request 在独立的 tab 中打开、编辑和发送，提升多接口并发操作体验。

## What Changes

- 在 NewRequestView 右侧区域增加 **request 级别的 tab 系统**：每个 tab 维护独立的 method、url、params、headers、cookies、body、bodyType 及 send 状态
- 支持从左侧接口管理树点击 request **打开 tab**（而非覆盖）— 同节点重复点击激活已有 tab
- 支持 **[+] 按钮创建临时 tab**（Unnamed Request），仅存在于内存，无关联树节点
- Tab 关闭时状态丢弃，不弹出确认提示；若有 linked 节点，关闭时不自动回存
- 链接 tab 内编辑内容 **debounced 实时同步** 回树节点
- 每个 tab 拥有 **独立的发送和响应状态**：切换 tab 后回来仍可看到该 tab 上次的请求/响应
- 全部无 tab 时显示占位空状态视图

## Capabilities

### New Capabilities

- `request-editor-tabs`: 请求编辑器的多 tab 管理——打开、切换、关闭、独立状态维护

### Modified Capabilities

（无现有 capability 需要修改）

## Impact

| 层面 | 影响 |
|------|------|
| `NewRequestView.tsx` | 核心重构区：将单一编辑器+响应面板改为 tab 容器 |
| `useCollections` hook | 可能需要 debounce 写回能力已具备，无需大改 |
| `RequestEditor.tsx` | 不修改内部，继续作为 tab 体的内容组件使用 |
| `DetailPanel` | 不修改，继续作为 tab 体的响应区使用 |
| 类型定义 | 新增 `RequestTab` interface |
| 翻译 | 新增少量 i18n key（tab tooltip、空状态等） |
