# 侧边工具栏 + 标题栏 Tab 设计

## 概述

在现有布局基础上新增左侧工具栏窄条和标题栏浏览器式 Tab，将"新建请求"从弹窗改为嵌入页面的独立 Tab。现有主内容区布局完全不变。

## 改动范围

### 新增：左侧工具栏

- 位置：DomainSidebar 最左侧，新增一个 ~36px 宽的垂直窄条
- 内容：只有两个图标按钮
  - 📡 **代理流量**：点击打开/切换到流量日志 Tab
  - ✏️ **新请求**：点击打开/切换到请求编辑器 Tab
- 交互：点击图标 → 标题栏打开对应 Tab（如已存在则切换到该 Tab）

### 新增：标题栏浏览器式 Tab

- 位置：标题栏区域，Chrome 风格圆角标签页
- Tab 类型：
  - "代理流量" Tab：默认 Tab，代理启动后自动存在
  - "新请求" Tab：点击工具栏 ✏️ 图标后打开
- 交互：
  - 多 Tab 并存，点击切换活跃 Tab
  - ✕ 按钮关闭 Tab（代理流量 Tab 不可关闭，为默认页）
  - 右侧保留现有按钮（▶ 代理控制等）

### 新增：新请求 Tab 内容区

- 请求编辑器，替代原有 EditRequestDialog 弹窗
- 包含：Method 选择、URL 输入、Headers 编辑、Body 编辑、Send 按钮
- 内容与原 EditRequestDialog（entry=null 模式）相同，只是从弹窗变为页面内嵌

### 不变的部分

- DomainSidebar + RequestList + DetailPanel 三栏布局不变
- 代理捕获请求的流程不变
- 右键菜单"编辑"仍使用弹窗，暂不调整
- 设置、SSL、脚本等弹窗暂不调整
- 工具栏暂只包含代理流量和新请求两个图标，不加入设置等

## 前端架构影响

### 新增组件

- `src/components/SidebarToolbar.tsx`：左侧工具栏窄条，渲染图标按钮，点击时调用 tab 管理
- `src/features/request-composer/RequestComposer.tsx`：请求编辑器页面组件（从 EditRequestDialog 表单逻辑迁移）

### 状态管理

- 新增 `openTabs` 状态（App 层级）：记录标题栏已打开的 Tab 列表
- 新增 `activeTab` 状态（App 层级）：当前活跃 Tab ID
- 工具栏点击 → 添加/切换 Tab → 标题栏渲染 Tab → 主内容区根据 activeTab 渲染对应内容

### 布局层级变化

```
App.tsx 原布局：
  TitleBar | TypeFilterBar | TrafficLog

App.tsx 新布局：
  TitleBar（含浏览器式 Tab） | TypeFilterBar | SidebarToolbar | TrafficLog / RequestComposer
```

主内容区根据 activeTab 条件渲染：
- activeTab === "traffic" → 渲染 TrafficLog（原有三栏，完全不变）
- activeTab === "composer" → 渲染 RequestComposer（新请求编辑器）
