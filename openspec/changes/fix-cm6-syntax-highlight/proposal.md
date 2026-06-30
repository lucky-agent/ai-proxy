# Proposal: 修复 new-request Body 编辑器 CM6 语法高亮失效

## 问题描述

`new-request` 的 Body 编辑器（`src/components/code-editor/CodeEditor.tsx`，基于 CodeMirror 6）中，JSON 的 key 与 value 显示同一种颜色，无语法高亮。DOM 检查 `.cm-line` 内 `tok-*` span 数量为 0。

独立验证（browser import map 直接加载 `node_modules` CM6 源码）高亮正常（17 个 `tok-*` span），证明 CM6 + `classHighlighter` + `tok-*` CSS 全部正常，问题只在 Vite 预构建层。

## 根因分析

CodeMirror 6 的语法高亮依赖跨包**对象身份（identity）匹配**：

- `classHighlighter`（来自 `@lezer/highlight`）通过 `Tag` 对象身份匹配 token
- `@codemirror/lang-json` 的 `styleTags({...})` 把节点名映射到 `tags.propertyName` 等 `Tag` 实例
- 当 parser 侧的 `Tag` 与 highlighter 侧的 `Tag` 不是同一个 JS 对象时，匹配失败 → 不生成 `tok-*` 类 → 无颜色

Vite 预构建（esbuild）把 `@lezer/highlight` 打到了**两个不同的 chunk**：

| chunk | 来源 | 谁用它 |
|-------|------|--------|
| `@lezer_highlight.js` | `@lezer/highlight` 独立预构建 chunk | `CodeEditor.tsx` 的 `import { classHighlighter } from '@lezer/highlight'` |
| `dist-CxZDSs6v.js` | `@lezer/highlight` + `@lezer/common` 被内联进 `@codemirror/lang-json` 的依赖图 | `lang-json.js` 的 `import { tags, styleTags } from '@lezer/highlight'` |

两侧 `Tag` 实例分属不同模块 → 身份不匹配 → 高亮失效。

现有的 `resolve.dedupe` 列表不完整：包含 `@codemirror/language`、`@codemirror/view`、`@codemirror/state`、`@lezer/common`，但**遗漏了 `@lezer/highlight` 和 `@lezer/lr`**。dedupe 对已列入的包是生效的（`@codemirror/language` 已被 `codemirror.js` 与 `lang-json.js` 共享为 `dist-m-nkcs0Y.js`）；只是被遗漏的 `@lezer/highlight` 仍保持双实例。

## 修复目标

1. 补全 `vite.config.ts` 的 `resolve.dedupe`，加入 `@lezer/highlight` 与 `@lezer/lr`（以及 CM6 生态其他需单实例的包），使 `@lezer/highlight` 在预构建中收敛为单一 chunk
2. 清理 Vite 预构建缓存，使新配置生效
3. 亮色 / 暗色模式下 Body 编辑器 JSON key、value、number、string 等均出现对应 `tok-*` 颜色

## 非目标

- 不替换 CM6 框架（`@uiw/react-codemirror` 底层一致，无意义）
- 不改 `CodeEditor.tsx` 的扩展装配方式（`basicSetup + syntaxHighlighting(classHighlighter) + lang` 本身正确）
- 不改 `index.css` 的 `tok-*` 配色（standalone 测试已证明配色正确）
