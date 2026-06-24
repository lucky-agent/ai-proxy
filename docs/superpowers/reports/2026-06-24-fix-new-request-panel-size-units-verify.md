# 验证报告：fix-new-request-panel-size-units

**日期**: 2026-06-24
**模式**: 轻量验证（覆盖自 full——实际源码改动仅 1 文件、无 delta spec、无 Design Doc，full 路径多数检查项 N/A）
**review_mode**: off

## 改动概述

`NewRequestView.tsx` 垂直分割（editor/response）的 `ResizablePanel` 尺寸属性由纯数字改为字符串百分比。

**根因**：`react-resizable-panels@4.11.2` 的 `bt()` 解析函数对纯数字按像素（px）解析，对带 `%` 的字符串按百分比解析。原代码 `maxSize={80}` 实为 80px（在约 550px 容器中≈14.5%），导致 editor 被压制在≈15%、response 膨胀至≈85%，覆盖 request body 且分隔条无法向下拖过 editor 的 px 上限。

## 轻量验证结果

| # | 检查项 | 结果 | 说明 |
|---|--------|------|------|
| 1 | tasks.md 全部完成任务 | PASS | `grep -c '\- \[ \]'` = 0（4 项全勾选） |
| 2 | 改动文件与 tasks 一致 | PASS | 提交 `f69fa7f`：`src/features/new-request/NewRequestView.tsx`（+`scripts/verify-panel-units.ts` 验证脚本），与 tasks 1.1/1.2 描述完全匹配 |
| 3 | 编译通过 | PASS | `bun run build:vite` exit 0（新鲜执行） |
| 4 | 相关测试通过 | N/A | 项目当前无测试套件 |
| 5 | 无明显安全问题 | PASS | diff 无硬编码密钥、令牌、unsafe 或 eval |
| 6 | 代码审查 | SKIPPED | `review_mode: off` |

## 根因消除 / 原始症状验证

运行 `scripts/verify-panel-units.ts`（复现库 `bt()`/`ie()` 解析数学，group=550px）：

| 配置 | editor maxSize 解析 | editor 结果 | response 结果 |
|------|---------------------|------------|---------------|
| 旧（纯数字） | `80` → 14.545% | 14.545% | **85.455%**（覆盖 body） |
| 新（% 字符串） | `"80%"` → 80% | 60% | **40%**（合理，可向下拖至 20%） |

- 旧配置复现用户症状：response≈85% 覆盖 request body，editor maxSize≈15% 阻止分隔条向下拖动。
- 新配置消除根因：response=40%，editor maxSize=80% 使 response 可向下拖至 20%。
- 根因消除检查：`grep` 确认 new-request 垂直分割不再有纯数字尺寸属性（左侧 collection 面板的 `collapsedSize={0}` 为 0，0px≡0%，非本 bug，未改）。

## 结论

全部检查通过 ✅。根因（尺寸属性单位 px vs %）已消除，原始症状由验证脚本复现并证明修复有效。

## 备注

- `TrafficLog.tsx:250` 的 `defaultSize={showDomainSidebar ? 18 : 0}` 存在同类 px 隐患（18px），不在本次修复范围，建议后续单独处理。
- 实际运行时视觉确认建议：`bun run dev` 在 new-request 视图新建请求，观察 response 面板≈40%、可向下拖动缩小。
