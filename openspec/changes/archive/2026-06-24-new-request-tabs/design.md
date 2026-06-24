## Context

NewRequestView 当前使用单一共享编辑器状态（method、url、params 等一组 state），左侧树任意点击一个 request 都会覆盖这份状态。需要改造为多 tab 架构，每个 tab 保有独立的请求编辑状态和响应结果。

**现有组件关系：**
```
NewRequestView（所有 state 在此）
├── ApiCollectionPanel（只读，props 穿透）
├── RequestEditor（受控组件，接收 props + onXxxChange）
├── DetailPanel（响应展示，接收 entry prop）
```

RequestEditor 内部有一个子 tab 状态（params/body/headers/cookies/auth），这是 UI ephemeral 状态，不计入数据模型。

## Goals / Non-Goals

**Goals:**
- 支持多个 RequestTab 并存，各自独立状态
- 从树节点打开 tab、从 [+] 新建临时 tab、切换、关闭
- 链接 tab 的内容实时同步回树节点（debounced）
- 每个 tab 有独立的发送状态和响应

**Non-Goals:**
- 不修改左侧 ApiCollectionPanel / ApiTreeView
- 不修改 RequestEditor / DetailPanel 内部实现
- 不修改后端 Tauri commands
- 不持久化 tab 列表（刷新后所有 tab 重新开始）

## Decisions

### 1. 数据模型：`RequestTab` 接口

```ts
interface RequestTab {
  id: string            // crypto.randomUUID() — tab 唯一标识
  name: string          // 显示的 tab 名称
  linkedNodeId: string | null  // 关联树节点 id，null = 临时 tab
  method: HttpMethod
  url: string
  params: KeyValuePair[]
  headers: KeyValuePair[]
  cookies: KeyValuePair[]
  bodyType: BodyType
  body: string
  // Response state（每个 tab 独立）
  responseEntryId: string | null
  sending: boolean
  error: string
}
```

**决策理由：** 将现有 flat state 包装进 `RequestTab`，每个实例是独立的编辑+响应单元。`linkedNodeId` 负责标示与树的关联，null 表示临时 tab。

### 2. 状态管理：自定义 `useRequestTabs` hook

```ts
// 返回值
{
  tabs: RequestTab[]
  activeTabId: string | null
  activeTab: RequestTab | undefined
  openTab: (linkedNodeId: string | null, nodeData?: ApiRequestNode) => void
  closeTab: (tabId: string) => void
  activateTab: (tabId: string) => void
  updateActiveTab: (patch: Partial<RequestTab>) => void  // 同时触发树同步
}
```

**放在 NewRequestView 内部还是独立 hook？** 选择独立 hook `useRequestTabs`：
- 将 150+ 行的 tab 逻辑从 NewRequestView 剥离，保持视图组件聚焦布局
- 方便以后复用或测试
- 接受 `updateRequest` 回调用于树同步

### 3. 渲染策略：only active tab mounted

只挂载活跃 tab 的内容组件（RequestEditor + DetailPanel），切换 tab 时卸载上一 tab 内容、挂载新 tab 内容。Tab 数据保存在 `tabs[]` 中，不依赖 DOM 保留。

**Alternatives considered:**
- **全部挂载 + CSS 隐藏**：state 保留简单但浪费 DOM（5+ tab 时明显）
- **只挂载 active**（选择）：性能好，RequestEditor 的子 tab 状态回退到默认（params→body），这是可接受的行为

### 4. TabBar 组件：新建 `RequestTabBar`

不复用顶部 TitleBar 的 `TabBar`（其交互模型不同：view-level tab 有关闭保护、特殊样式）。新建一个轻量 `RequestTabBar` 作为 NewRequestView 内部的子组件：
- 水平排列 tabs + [+] 按钮
- 每个 tab 显示 name + ✕ 关闭按钮
- 活跃 tab 高亮
- 溢出时水平滚动

### 5. 树同步机制

`updateActiveTab` 除了更新 `tabs[]` 中的当前 tab 外，当 `linkedNodeId` 非 null 时，调用 `updateRequest(linkedNodeId, {...})` 触发 debounced 持久化到 Rust 后端。这与现有 `useCollections.updateRequest` + `debouncedSave` 机制无缝对接。

**为什么不在 Tab 层面区分 linked/temp 的保存行为？** 统一通过 `linkedNodeId` 判断：有 link → 自动同步，无 link → 仅内存。简单且不易出错。

### 6. 树节点删除时关联 tab 的处理

当用户删除树节点时，所有链接到该节点的 tab 的 `linkedNodeId` 设为 `null`（变成临时 tab），避免数据丢失。不自动关闭 tab。

### 7. [+] 按钮与临时 tab

TabBar 右侧的 [+] 按钮创建 `linkedNodeId: null` 的空白 tab，name 为 "Untitled Request"（或 i18n key）。用户可在其中编辑并发送请求，但未保存到树。关闭时直接丢弃，无确认弹窗。

## Risks / Trade-offs

- **[状态膨胀]** 每个 tab 包含完整请求+响应状态，大量 tab 可能增加内存 → **缓解**：tab 数量通常很小（<20），此规模下内存可忽略
- **[树节点覆盖]** 临时 tab 编辑后用户期望"保存到树" → 可在后续迭代加 "Save to Collection" 功能，当前不在此变更范围

## Open Questions

（暂无，所有决策已在脑暴中确认）
