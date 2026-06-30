# Tasks: 修复 CM6 语法高亮失效

- [x] 补全 `vite.config.ts` 的 `resolve.dedupe`，加入 `@lezer/highlight`、`@lezer/lr`、`codemirror`、`@codemirror/commands`、`@codemirror/autocomplete`、`@codemirror/search`
- [x] 删除 `node_modules/.vite/deps` 预构建缓存
- [x] 新增 `optimizeDeps.exclude: ["@lezer/common","@lezer/highlight","@lezer/lr"]`（dedupe 不足以阻止 esbuild 内联，改用 exclude 强制 raw ESM 单实例；静态验证通过：lang-json 与 language chunk 均以 bare specifier 外部化 `@lezer/highlight`，不再有独立 `@lezer_highlight.js` 预构建 chunk）
- [x] 改写 `CodeEditor.tsx` 使用 `@uiw/react-codemirror`（默认 basicSetup 含 defaultHighlightStyle，叠加 `syntaxHighlighting(classHighlighter)` 走项目 tok-* 配色）；静态验证含 @uiw 后 `@lezer/highlight` 仍全局单实例（lang-json 与 @uiw chunk 解析到同一 `/node_modules/@lezer/highlight/dist/index.js?v=...`）
- [ ] 启动 dev，在 Body 编辑器输入 JSON，DOM 验证 `tok-*` span > 0，亮色/暗色 key-value 颜色区分
