# 左侧工具栏 + Tab 切换系统设计

## 目标

在应用最左侧新增一个 42px 紧凑图标工具栏，包含 3 个视图入口（代理、新请求、AI）。点击图标切换主内容区的独立布局。Title bar 内嵌可关闭 tab 显示当前活跃视图，关闭 tab 回到代理视图。

## 当前状态

- 主内容区 = `TrafficLog`（代理视图），无视图切换机制
- "新请求"功能通过 `EditRequestDialog`（overlay dialog）实现
- Title bar 包含：AlignJustify 工具栏展开按钮、菜单、布局切换按钮、操作按钮、窗口控制
- 无 AI 功能模块

## 设计

### 1. 视图状态模型

```ts
type ViewId = 'proxy' | 'new-request' | 'ai'

// App.tsx 管理全局状态
const [activeView, setActiveView] = useState<ViewId>('proxy')
```

- 默认视图为 `'proxy'`（当前代理流量日志）
- 点击左侧图标 → `setActiveView(id)`
- 关闭 tab → `setActiveView('proxy')`
- 各视图状态独立保留，切换不丢失

### 2. 左侧工具栏 (ToolBar)

**位置**：应用最左侧，垂直排列，宽度 42px

**结构**：
```
┌────┐
│ 🌐 │  ← 代理（GlobeIcon）
│ 📝 │  ← 新请求（SquarePenIcon）
│ 🤖 │  ← AI（BotIcon / SparklesIcon）
└────┐
```

**交互**：
- 选中项：左侧 2px 主题色条 + `bg-primary/10` 背景 + 主题色图标
- 未选中项：灰色图标，hover 时 `bg-surface-elevated/50` + 亮色图标
- 点击切换 `activeView`

**组件**：`src/features/tool-bar/ToolBar.tsx`

Props:
```ts
interface ToolBarProps {
  activeView: ViewId
  onViewChange: (view: ViewId) => void
}
```

### 3. Title Bar 内嵌 Tab

**位置**：Title bar 内，紧跟在 AlignJustify 菜单区域之后

**结构**：
```
[⚡图标] [AI Proxy│工具] [代理 ✕] [新请求 ✕] [←弹性空间→] [操作按钮] [窗口控制]
```

**交互**：
- 活跃 tab：`bg-primary/10` 背景 + 主题色文字
- 非活跃 tab：`text-muted-foreground`
- 每个 tab 右侧 ✕ 按钮，hover 时变为更明显
- 关闭 tab → `setActiveView('proxy')`（始终回到代理视图）
- 代理 tab 始终可见（默认打开），不可完全移除

**组件**：`src/features/title-bar/TabBar.tsx`

Props:
```ts
interface TabBarProps {
  activeView: ViewId
  onViewChange: (view: ViewId) => void
  onCloseTab: (view: ViewId) => void
}
```

**重要约束**：AlignJustify 工具栏展开/收起功能完全保持不变，不做任何修改。

### 4. 视图内容区

**代理视图**（`'proxy'`）：当前的 `TrafficLog` + `TypeFilterBar`，不做改动。

**新请求视图**（`'new-request'`）：独立的全屏编辑面板 `NewRequestView`。
- 从 `EditRequestDialog` 提取表单逻辑（方法选择、URL 输入、Headers/Body 编辑、发送按钮）
- 布局改为全屏面板而非 overlay dialog
- 发送成功后自动切换回代理视图并选中对应请求

**AI 视图**（`'ai'`）：占位 UI `AiView`。
- 暂时只显示居中占位文案："AI 功能即将推出"
- 后续迭代再定义具体功能

### 5. App.tsx 整体布局变更

当前布局：
```
[TitleBar] → [TypeFilterBar] → [TrafficLog] → [Dialog overlays]
```

新布局：
```
[ToolBar] | [TitleBar + Tabs] → [TypeFilterBar*] → [MainContent*] → [Dialog overlays]

* TypeFilterBar 和 MainContent 根据 activeView 切换：
  - proxy: TypeFilterBar + TrafficLog
  - new-request: NewRequestView
  - ai: AiView
```

具体 JSX 结构：
```tsx
<div className="flex h-full">
  <ToolBar activeView={activeView} onViewChange={setActiveView} />
  <div className="flex h-full flex-col overflow-hidden flex-1">
    <TitleBar
      activeView={activeView}
      onViewChange={setActiveView}
      onCloseTab={(view) => setActiveView('proxy')}
      ... // 其他现有 props 保持不变
    />
    {activeView === 'proxy' && (
      <>
        <TypeFilterBar ... />
        <TrafficLog ... />
      </>
    )}
    {activeView === 'new-request' && <NewRequestView ... />}
    {activeView === 'ai' && <AiView />}
    {/* Dialog overlays 保持不变 */}
  </div>
</div>
```

### 6. 项目结构变更

新增：
- `src/features/tool-bar/` — ToolBar.tsx + index.ts
- `src/features/ai-view/` — AiView.tsx + index.ts
- `src/features/new-request/` — NewRequestView.tsx + index.ts

调整：
- `src/features/title-bar/` — 新增 TabBar.tsx，修改 TitleBar.tsx 引入 TabBar
- `src/App.tsx` — 新增 activeView 状态，重构布局为 ToolBar + 主区域

不变：
- `src/features/traffic-log/` — EditRequestDialog 保留在 traffic-log 内
- `src/features/settings/`、`ssl-config/`、`script-config/`、`about/` — 不变

### 7. 国际化

新增 key（`en.json` + `zh.json`）：

```json
// en.json
{
  "view": {
    "proxy": "Proxy",
    "newRequest": "New Request",
    "ai": "AI"
  }
}

// zh.json
{
  "view": {
    "proxy": "代理",
    "newRequest": "新请求",
    "ai": "AI"
  }
}
```

### 8. 图标选择

| 视图 | 图标 | 来源 |
|------|------|------|
| 代理 | `GlobeIcon` | lucide-react |
| 新请求 | `SquarePenIcon` | lucide-react（已在项目中使用） |
| AI | `SparklesIcon` | lucide-react |

### 9. 样式细节

工具栏：
- 宽度 42px，背景 `bg-surface-deep`
- 右侧 `border-r border-border`
- 选中指示：左侧 2px `bg-primary` 色条 + `bg-primary/10` 背景
- 图标尺寸 18px

Tab：
- 嵌入 Title bar 行内
- 活跃：`bg-primary/10` + `text-primary` + `rounded-md`
- 非活跃：`text-muted-foreground` + hover `text-foreground`
- ✕ 按钮：`text-muted-foreground/50`，hover `text-muted-foreground` + `bg-surface-elevated/30`

## 不做的事情

- 不修改 AlignJustify 工具栏展开/收起功能
- 不移除 traffic-log 内的 EditRequestDialog（代理视图内的编辑/重发仍用 dialog）
- 不实现 AI 视图的具体功能（只做占位）
- 不更改现有的布局切换按钮（domain sidebar / detail bottom / detail right）
