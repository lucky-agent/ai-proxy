## 1. 类型定义与 Hook

- [ ] 1.1 新增 `RequestTab` interface 到 `src/types/collection.ts`
- [ ] 1.2 新建 `src/features/new-request/useRequestTabs.ts`：实现 `useRequestTabs` hook（openTab / closeTab / activateTab / updateActiveTab）

## 2. TabBar 组件

- [ ] 2.1 新建 `src/features/new-request/RequestTabBar.tsx`：水平排列 tab 标签 + ✕ 关闭 + [+] 新建 + 溢出滚动

## 3. NewRequestView 重构

- [ ] 3.1 重构 `NewRequestView.tsx`：用 `useRequestTabs` 替代原有 flat state，将右侧区域改为 tab 容器渲染
- [ ] 3.2 实现无 tab 空状态占位视图（"点击左侧接口或 [+] 新建请求"）
- [ ] 3.3 树节点点击 → `openTab(node.id, node)`；树节点删除时将关联 tab 转为临时 tab

## 4. i18n

- [x] 4.1 在 `src/locales/en.json` 和 `src/locales/zh.json` 中新增 tab 相关翻译 key（Untitled Request、空状态提示、tab tooltip 等）
