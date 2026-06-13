# 左侧工具栏 + Tab 切换系统 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在应用最左侧新增 42px 紧凑图标工具栏，支持代理/新请求/AI 三个独立视图切换，Title bar 内嵌可关闭 tab。

**架构：** App.tsx 管理全局 `activeView` 状态，ToolBar 和 TabBar 共享此状态驱动视图切换。主内容区按 `activeView` 条件渲染对应视图组件。各视图状态独立保留。

**技术栈：** React 19 + TypeScript + Tailwind CSS 4 + lucide-react + i18next

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/types/view.ts` | 创建 | ViewId 类型定义 |
| `src/features/tool-bar/ToolBar.tsx` | 创建 | 左侧图标工具栏组件 |
| `src/features/tool-bar/index.ts` | 创建 | 对外导出入口 |
| `src/features/title-bar/TabBar.tsx` | 创建 | Title bar 内嵌可关闭 tab 组件 |
| `src/features/title-bar/TitleBar.tsx` | 修改 | 接收 activeView/onViewChange/onCloseTab props，嵌入 TabBar |
| `src/features/title-bar/index.ts` | 修改 | 导出 TabBar |
| `src/features/ai-view/AiView.tsx` | 创建 | AI 占位视图组件 |
| `src/features/ai-view/index.ts` | 创建 | 对外导出入口 |
| `src/features/new-request/NewRequestView.tsx` | 创建 | 全屏请求编辑面板 |
| `src/features/new-request/index.ts` | 创建 | 对外导出入口 |
| `src/App.tsx` | 修改 | 新增 activeView 状态，重构布局为 ToolBar + 主区域，条件渲染视图 |
| `src/locales/en.json` | 修改 | 新增 view 翻译 key |
| `src/locales/zh.json` | 修改 | 新增 view 翻译 key |

---

### 任务 1：ViewId 类型定义

**文件：**
- 创建：`src/types/view.ts`

- [ ] **步骤 1：创建 ViewId 类型文件**

```ts
// src/types/view.ts
export type ViewId = 'proxy' | 'new-request' | 'ai'
```

- [ ] **步骤 2：验证构建**

运行：`bun run build:vite`
预期：构建成功，无类型错误

- [ ] **步骤 3：Commit**

```bash
git add src/types/view.ts
git commit -m "feat: add ViewId type for sidebar toolbar view switching"
```

---

### 任务 2：ToolBar 组件

**文件：**
- 创建：`src/features/tool-bar/ToolBar.tsx`
- 创建：`src/features/tool-bar/index.ts`

- [ ] **步骤 1：创建 ToolBar 组件**

```tsx
// src/features/tool-bar/ToolBar.tsx
import { GlobeIcon, SquarePenIcon, SparklesIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { ViewId } from '@/types/view'

const VIEW_ITEMS: { id: ViewId; icon: typeof GlobeIcon; labelKey: string }[] = [
  { id: 'proxy', icon: GlobeIcon, labelKey: 'view.proxy' },
  { id: 'new-request', icon: SquarePenIcon, labelKey: 'view.newRequest' },
  { id: 'ai', icon: SparklesIcon, labelKey: 'view.ai' },
]

interface ToolBarProps {
  activeView: ViewId
  onViewChange: (view: ViewId) => void
}

export function ToolBar({ activeView, onViewChange }: ToolBarProps) {
  const { t } = useLocale()

  return (
    <div className="flex w-[42px] shrink-0 flex-col items-center border-r border-border bg-surface-deep py-1.5">
      {VIEW_ITEMS.map(({ id, icon: Icon, labelKey }) => (
        <button
          key={id}
          type="button"
          onClick={() => onViewChange(id)}
          className={cn(
            'relative flex h-8 w-8 items-center justify-center rounded-md transition-colors my-0.5',
            activeView === id
              ? 'bg-primary/10 text-primary'
              : 'text-muted-foreground hover:bg-surface-elevated/50 hover:text-foreground'
          )}
          title={t(labelKey)}>
          <Icon className="size-[18px]" />
          {activeView === id && (
            <span className="absolute -left-1.5 top-1/2 -translate-y-1/2 h-4 w-[2px] rounded-full bg-primary" />
          )}
        </button>
      ))}
    </div>
  )
}
```

- [ ] **步骤 2：创建 index.ts 导出**

```ts
// src/features/tool-bar/index.ts
export { ToolBar } from './ToolBar'
```

- [ ] **步骤 3：验证构建**

运行：`bun run build:vite`
预期：构建成功

- [ ] **步骤 4：Commit**

```bash
git add src/features/tool-bar/
git commit -m "feat: add ToolBar component for left sidebar view switching"
```

---

### 任务 3：TabBar 组件

**文件：**
- 创建：`src/features/title-bar/TabBar.tsx`

- [ ] **步骤 1：创建 TabBar 组件**

```tsx
// src/features/title-bar/TabBar.tsx
import { XIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { ViewId } from '@/types/view'

const VIEW_TABS: { id: ViewId; labelKey: string }[] = [
  { id: 'proxy', labelKey: 'view.proxy' },
  { id: 'new-request', labelKey: 'view.newRequest' },
  { id: 'ai', labelKey: 'view.ai' },
]

interface TabBarProps {
  activeView: ViewId
  onViewChange: (view: ViewId) => void
  onCloseTab: (view: ViewId) => void
}

function stopTitleBarDrag(event: { stopPropagation: () => void }) {
  event.stopPropagation()
}

export function TabBar({ activeView, onViewChange, onCloseTab }: TabBarProps) {
  const { t } = useLocale()

  return (
    <div className="flex items-center gap-1" data-tauri-drag-region={false}>
      {VIEW_TABS.map(({ id, labelKey }) => (
        <button
          key={id}
          type="button"
          data-tauri-drag-region={false}
          onMouseDown={stopTitleBarDrag}
          onPointerDown={stopTitleBarDrag}
          onClick={() => onViewChange(id)}
          className={cn(
            'inline-flex items-center gap-1 rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors',
            activeView === id
              ? 'bg-primary/10 text-primary'
              : 'text-muted-foreground hover:text-foreground'
          )}>
          {t(labelKey)}
          <span
            role="button"
            tabIndex={-1}
            data-tauri-drag-region={false}
            onMouseDown={stopTitleBarDrag}
            onPointerDown={stopTitleBarDrag}
            onClick={(e) => {
              e.stopPropagation()
              onCloseTab(id)
            }}
            className={cn(
              'inline-flex items-center justify-center rounded p-0.5 transition-colors',
              'text-muted-foreground/50 hover:text-muted-foreground hover:bg-surface-elevated/30'
            )}>
            <XIcon className="size-3" />
          </span>
        </button>
      ))}
    </div>
  )
}
```

- [ ] **步骤 2：验证构建**

运行：`bun run build:vite`
预期：构建成功

- [ ] **步骤 3：Commit**

```bash
git add src/features/title-bar/TabBar.tsx
git commit -m "feat: add TabBar component for title bar view tabs"
```

---

### 任务 4：AiView 占位组件

**文件：**
- 创建：`src/features/ai-view/AiView.tsx`
- 创建：`src/features/ai-view/index.ts`

- [ ] **步骤 1：创建 AiView 组件**

```tsx
// src/features/ai-view/AiView.tsx
import { SparklesIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'

export function AiView() {
  const { t } = useLocale()

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-surface-deep text-muted-foreground">
      <SparklesIcon className="size-12 text-muted-foreground/30" />
      <p className="text-sm font-medium">{t('view.aiComingSoon')}</p>
    </div>
  )
}
```

- [ ] **步骤 2：创建 index.ts 导出**

```ts
// src/features/ai-view/index.ts
export { AiView } from './AiView'
```

- [ ] **步骤 3：验证构建**

运行：`bun run build:vite`
预期：构建成功

- [ ] **步骤 4：Commit**

```bash
git add src/features/ai-view/
git commit -m "feat: add AiView placeholder component"
```

---

### 任务 5：NewRequestView 全屏请求编辑面板

**文件：**
- 创建：`src/features/new-request/NewRequestView.tsx`
- 创建：`src/features/new-request/index.ts`

这个组件从 `EditRequestDialog` 提取核心表单逻辑（method/URL/headers/body/发送），改为全屏面板布局，去掉 Dialog 包裹。

- [ ] **步骤 1：创建 NewRequestView 组件**

```tsx
// src/features/new-request/NewRequestView.tsx
import { useState, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { PlusIcon, Trash2Icon, SendIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'

interface HeaderPair {
  key: string
  value: string
}

interface NewRequestViewProps {
  onSendSuccess: (entryId: string) => void
}

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']

const METHOD_COLORS: Record<string, string> = {
  GET: 'badge-get',
  POST: 'badge-post',
  PUT: 'badge-put',
  DELETE: 'badge-delete',
  PATCH: 'badge-patch',
  HEAD: 'badge-head',
  OPTIONS: 'badge-options',
}

export function NewRequestView({ onSendSuccess }: NewRequestViewProps) {
  const { t } = useLocale()
  const [method, setMethod] = useState('GET')
  const [url, setUrl] = useState('')
  const [headers, setHeaders] = useState<HeaderPair[]>([])
  const [body, setBody] = useState('')
  const [sending, setSending] = useState(false)
  const [error, setError] = useState('')

  const handleAddHeader = useCallback(() => setHeaders(h => [...h, { key: '', value: '' }]), [])
  const handleRemoveHeader = useCallback((i: number) => setHeaders(h => h.filter((_, idx) => idx !== i)), [])
  const handleHeaderChange = useCallback((i: number, field: 'key' | 'value', val: string) => {
    setHeaders(h => h.map((pair, idx) => idx === i ? { ...pair, [field]: val } : pair))
  }, [])

  const handleSend = useCallback(async () => {
    if (sending) return
    if (!url.trim()) return

    setSending(true)
    setError('')

    const headerMap: Record<string, string> = {}
    for (const { key, value } of headers) {
      if (key.trim()) headerMap[key.trim()] = value
    }

    try {
      const entryId = await invoke<string>('resend_request', {
        method,
        url: url.trim(),
        headers: headerMap,
        body: body || null,
      })
      onSendSuccess(entryId)
    } catch (err) {
      setError(String(err))
    } finally {
      setSending(false)
    }
  }, [sending, url, method, headers, body, onSendSuccess])

  return (
    <div className="flex h-full flex-col bg-surface-deep">
      {/* Top bar: method + URL + send */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border bg-surface-base/50">
        <select
          value={method}
          onChange={e => setMethod(e.target.value)}
          className={cn(
            'shrink-0 rounded-md border border-input bg-background px-2.5 py-1.5 text-xs font-semibold outline-none focus:ring-1 focus:ring-primary',
            METHOD_COLORS[method] && `text-${METHOD_COLORS[method]}`
          )}>
          {METHODS.map(m => (
            <option key={m} value={m}>{m}</option>
          ))}
        </select>
        <input
          type="text"
          value={url}
          onChange={e => setUrl(e.target.value)}
          className="flex-1 rounded-md border border-input bg-background px-3 py-1.5 text-xs text-foreground font-mono outline-none focus:ring-1 focus:ring-primary"
          placeholder="https://api.example.com/v1/endpoint"
        />
        <Button onClick={handleSend} disabled={sending || !url.trim()} size="sm">
          <SendIcon className="size-3.5" />
          {sending ? '...' : t('sendRequest.send')}
        </Button>
      </div>

      {/* Content area: headers + body */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4 min-h-0">
        {/* Headers */}
        <div>
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-xs font-medium text-foreground/80">{t('detail.headers')}</span>
            <button
              onClick={handleAddHeader}
              className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors">
              <PlusIcon className="size-3" />
              {t('sendRequest.addHeader')}
            </button>
          </div>
          <div className="space-y-1">
            {headers.map((pair, i) => (
              <div key={i} className="flex gap-1 items-center">
                <input
                  type="text"
                  value={pair.key}
                  onChange={e => handleHeaderChange(i, 'key', e.target.value)}
                  className="flex-1 rounded border border-input bg-background px-2 py-1 text-[11px] text-foreground font-mono outline-none focus:ring-1 focus:ring-primary"
                  placeholder="Key"
                />
                <input
                  type="text"
                  value={pair.value}
                  onChange={e => handleHeaderChange(i, 'value', e.target.value)}
                  className="flex-[2] rounded border border-input bg-background px-2 py-1 text-[11px] text-foreground font-mono outline-none focus:ring-1 focus:ring-primary"
                  placeholder="Value"
                />
                <button
                  onClick={() => handleRemoveHeader(i)}
                  className="shrink-0 rounded p-1 text-muted-foreground hover:text-destructive transition-colors">
                  <Trash2Icon className="size-3" />
                </button>
              </div>
            ))}
          </div>
        </div>

        {/* Body */}
        <div>
          <span className="text-xs font-medium text-foreground/80 block mb-1.5">{t('detail.body')}</span>
          <textarea
            value={body}
            onChange={e => setBody(e.target.value)}
            className="w-full min-h-[200px] rounded-md border border-input bg-background px-3 py-2 text-xs text-foreground font-mono resize-y outline-none focus:ring-1 focus:ring-primary"
            placeholder="{ &quot;key&quot;: &quot;value&quot; }"
          />
        </div>

        {/* Error */}
        {error && (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}
      </div>
    </div>
  )
}
```

- [ ] **步骤 2：创建 index.ts 导出**

```ts
// src/features/new-request/index.ts
export { NewRequestView } from './NewRequestView'
```

- [ ] **步骤 3：验证构建**

运行：`bun run build:vite`
预期：构建成功

- [ ] **步骤 4：Commit**

```bash
git add src/features/new-request/
git commit -m "feat: add NewRequestView full-panel request editor"
```

---

### 任务 6：集成 TabBar 到 TitleBar

**文件：**
- 修改：`src/features/title-bar/TitleBar.tsx`
- 修改：`src/features/title-bar/index.ts`

需要：1) TitleBarProps 增加 `activeView`、`onViewChange`、`onCloseTab` 三个 prop；2) 在 TitleBar 内 AlignJustify 菜单区域之后嵌入 `<TabBar>`。

**重要约束**：AlignJustify 工具栏展开/收起功能完全保持不变，不做任何修改。

- [ ] **步骤 1：修改 TitleBar — 增加 props 和嵌入 TabBar**

在 `TitleBarProps` 中新增：

```ts
import type { ViewId } from '@/types/view'
import { TabBar } from './TabBar'

// 在 TitleBarProps 类型末尾追加：
interface TitleBarProps {
  // ... 所有现有 props 保持不变 ...
  activeView: ViewId
  onViewChange: (view: ViewId) => void
  onCloseTab: (view: ViewId) => void
}
```

在 TitleBar 函数签名中追加对应参数：

```ts
export function TitleBar({ onOpenSettings, onOpenAbout, onOpenSslConfig, onOpenScriptConfig, onOpenSendRequest, showDomainSidebar, onToggleDomainSidebar, showDetailBottom, onToggleDetailBottom, showDetailRight, onToggleDetailRight, running, onStartProxy, onStopProxy, onClearTraffic, activeView, onViewChange, onCloseTab }: TitleBarProps) {
```

在 TitleBar JSX 中，在 toolbar expanded 区域之后、第一个 spacer div 之前，嵌入 TabBar：

```tsx
{/* View tabs — always visible */}
<TabBar activeView={activeView} onViewChange={onViewChange} onCloseTab={onCloseTab} />
```

具体插入位置：在 `<div className="min-w-0 flex-1" data-tauri-drag-region />` (第一个 spacer) 之前，紧接 toolbar expanded div 结束标签之后。

- [ ] **步骤 2：修改 index.ts 导出 TabBar**

```ts
// src/features/title-bar/index.ts
export { TitleBar } from './TitleBar'
export { TabBar } from './TabBar'
```

- [ ] **步骤 3：验证构建**

运行：`bun run build:vite`
预期：构建成功

- [ ] **步骤 4：Commit**

```bash
git add src/features/title-bar/
git commit -m "feat: integrate TabBar into TitleBar for view switching"
```

---

### 任务 7：重构 App.tsx 布局

**文件：**
- 修改：`src/App.tsx`

这是最关键的集成步骤：1) 新增 `activeView` 状态；2) 引入所有新组件；3) 重构 JSX 布局为 ToolBar + 主区域；4) 主内容区条件渲染三个视图；5) NewRequestView 发送成功后切换回 proxy 视图。

- [ ] **步骤 1：修改 App.tsx**

需要做以下修改：

**新增导入：**

```ts
import type { ViewId } from '@/types/view'
import { ToolBar } from '@/features/tool-bar'
import { AiView } from '@/features/ai-view'
import { NewRequestView } from '@/features/new-request'
```

**新增状态：** 在 App 函数内部，现有 `typeFilter` 状态之后：

```ts
const [activeView, setActiveView] = useState<ViewId>('proxy')
```

**新增 handleSendSuccess 回调：**

```ts
const handleNewRequestSuccess = useCallback((entryId: string) => {
  setActiveView('proxy')
}, [])
```

**修改 return JSX：** 将现有 `<div className="flex h-full flex-col ...">` 外层包裹改为包含 ToolBar 的 flex 行：

```tsx
return (
  <div className="flex h-full overflow-hidden bg-surface-deep text-foreground">
    <ToolBar activeView={activeView} onViewChange={setActiveView} />
    <div className="flex h-full flex-col overflow-hidden flex-1">
      <TitleBar
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenAbout={() => setAboutOpen(true)}
        onOpenSslConfig={() => setSslConfigOpen(true)}
        onOpenScriptConfig={() => setScriptConfigOpen(true)}
        onOpenSendRequest={() => setSendRequestOpen(true)}
        showDomainSidebar={showDomainSidebar}
        onToggleDomainSidebar={() => setShowDomainSidebar(v => !v)}
        showDetailBottom={showDetailBottom}
        onToggleDetailBottom={() => {
          if (showDetailBottom) {
            setShowDetailBottom(false)
          } else {
            setShowDetailBottom(true)
            setShowDetailRight(false)
          }
        }}
        showDetailRight={showDetailRight}
        onToggleDetailRight={() => {
          if (showDetailRight) {
            setShowDetailRight(false)
          } else {
            setShowDetailRight(true)
            setShowDetailBottom(false)
          }
        }}
        running={running}
        onStartProxy={startProxy}
        onStopProxy={stopProxy}
        onClearTraffic={clear}
        activeView={activeView}
        onViewChange={setActiveView}
        onCloseTab={() => setActiveView('proxy')}
      />

      {activeView === 'proxy' && (
        <>
          <TypeFilterBar active={typeFilter} counts={typeCounts} onChange={setTypeFilter} running={running} status={status} />
          {error && (
            <div className="shrink-0 border-b border-destructive/30 bg-destructive/10 px-5 py-2 text-sm text-destructive">
              {error}
            </div>
          )}
          <TrafficLog entries={entries} showDomainSidebar={showDomainSidebar} showDetailBottom={showDetailBottom} showDetailRight={showDetailRight} onAutoOpenDetail={() => setShowDetailBottom(true)} typeFilter={typeFilter} />
        </>
      )}
      {activeView === 'new-request' && <NewRequestView onSendSuccess={handleNewRequestSuccess} />}
      {activeView === 'ai' && <AiView />}

      <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} />
      <SslConfigDialog open={sslConfigOpen} onOpenChange={setSslConfigOpen} />
      <ScriptConfigDialog open={scriptConfigOpen} onOpenChange={setScriptConfigOpen} />
      <EditRequestDialog
        open={sendRequestOpen}
        onOpenChange={setSendRequestOpen}
        entry={null}
      />
      <SettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        theme={theme}
        onThemeChange={setTheme}
      />
    </div>
  </div>
)
```

注意：移除原外层 div 的 `flex-col`（改为内部 div 承担 flex-col），移除原外层 div 的 `bg-surface-deep text-foreground`（保留在新外层 div 上）。

- [ ] **步骤 2：验证构建**

运行：`bun run build:vite`
预期：构建成功

- [ ] **步骤 3：视觉验证（手动）**

运行：`bun run dev`
预期：
- 左侧出现 42px 紧凑图标工具栏，显示 Globe/PenEdit/Sparkles 三个图标
- 默认选中"代理"图标，主内容区显示当前流量日志布局
- Title bar 在菜单区域后出现"代理"、"新请求"、"AI"三个 tab
- 点击"新请求"图标 → 主内容区切换为全屏请求编辑面板
- 点击 ✕ 关闭"新请求" tab → 回到代理视图
- AlignJustify 工具栏展开/收起功能不受影响

- [ ] **步骤 4：Commit**

```bash
git add src/App.tsx
git commit -m "feat: integrate ToolBar + view switching into App layout"
```

---

### 任务 8：国际化 key

**文件：**
- 修改：`src/locales/en.json`
- 修改：`src/locales/zh.json`

- [ ] **步骤 1：修改 en.json — 新增 view 和 aiComingSoon key**

在 `en.json` 末尾（`scriptConfig` 块之后），新增：

```json
  "view": {
    "proxy": "Proxy",
    "newRequest": "New Request",
    "ai": "AI",
    "aiComingSoon": "AI features coming soon"
  }
```

- [ ] **步骤 2：修改 zh.json — 新增对应翻译**

在 `zh.json` 末尾（`scriptConfig` 块之后），新增：

```json
  "view": {
    "proxy": "代理",
    "newRequest": "新请求",
    "ai": "AI",
    "aiComingSoon": "AI 功能即将推出"
  }
```

- [ ] **步骤 3：验证构建**

运行：`bun run build:vite`
预期：构建成功，i18n key 无缺失

- [ ] **步骤 4：Commit**

```bash
git add src/locales/en.json src/locales/zh.json
git commit -m "feat: add i18n keys for view switching tabs"
```

---

### 任务 9：最终验证和清理

- [ ] **步骤 1：完整构建验证**

运行：`bun run build`
预期：Tauri + Vite 完整构建成功

- [ ] **步骤 2：功能验证（手动运行）**

运行：`bun run dev`

逐项检查：
1. 左侧 42px 工具栏显示 3 个图标
2. 默认选中"代理"图标，主内容区为流量日志
3. Title bar 菜单后有 3 个 tab：代理（active）/ 新请求 / AI
4. 点击"新请求" → 主内容区切换为请求编辑面板
5. 点击 ✕ 关闭 tab → 回到代理视图
6. 点击"AI" → 显示占位文案
7. AlignJustify 工具栏展开/收起不受影响
8. 代理视图内的编辑/重发 dialog 不受影响
9. 各视图切换时状态独立保留（代理的筛选/选中不丢失）

- [ ] **步骤 3：Commit（如有微调）**

```bash
git add -A
git commit -m "feat: sidebar toolbar + tab switching system complete"
```
