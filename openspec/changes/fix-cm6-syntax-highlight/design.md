# Design: 修复 CM6 语法高亮失效

## 方案

补全 `vite.config.ts` 的 `resolve.dedupe`，覆盖 CM6 生态所有依赖对象身份的共享包。这是 CodeMirror 6 + Vite 的标准配置（CM6 跨包以 Facet / Tag 对象身份匹配，必须保证单实例），并非过度配置。

### dedupe 完整列表

```ts
dedupe: [
  "codemirror",
  "@codemirror/state",
  "@codemirror/view",
  "@codemirror/language",
  "@codemirror/commands",
  "@codemirror/autocomplete",
  "@codemirror/search",
  "@lezer/common",
  "@lezer/highlight",
  "@lezer/lr",
]
```

关键新增项：`@lezer/highlight`（Tag 身份匹配的核心，本次 bug 直接根因）、`@lezer/lr`（LRParser 身份）、`codemirror`（meta 包）。

### 为什么 dedupe 而不是 exclude

- `resolve.dedupe` 强制所有 bare import 解析到同一文件路径，预构建时 esbuild 据此把 `@lezer/highlight` 外部化为单一共享 chunk 而非内联到各依赖图
- 现有 dedupe 已对 `@codemirror/language` 生效（共享为 `dist-m-nkcs0Y.js`），证明机制有效，只是列表不全
- `optimizeDeps.exclude` 会强制所有 CM6/lezer 包走 raw ESM，配置更重且 dev 启动更慢；dedupe 是更轻量的正确解法

### 缓存清理

修改 `vite.config.ts` 后必须删除 `node_modules/.vite/deps`，否则浏览器继续加载旧预构建 chunk（这正是之前多次"改了仍无效"的原因之一）。

### 验证手段

dev 启动后，在 Tauri 窗口 Body 编辑器输入 JSON，DOM 检查 `.cm-line` 内 `tok-*` span 数量应 > 0；视觉上 key/value 颜色区分。亮色与暗色模式各验证一次。

## 风险

- 若 dedupe 仍不足以让 esbuild 外部化 `@lezer/highlight`（极少数情况），回退方案为对 `@lezer/highlight`、`@lezer/common`、`@lezer/lr` 单独 `optimizeDeps.include` 强制单 chunk；本设计首选 dedupe
