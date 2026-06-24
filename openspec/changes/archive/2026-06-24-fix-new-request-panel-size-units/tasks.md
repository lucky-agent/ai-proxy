## 1. 修复

- [x] 1.1 将 `NewRequestView.tsx` 第 261 行 editor 面板的 `defaultSize` / `minSize` / `maxSize` 由纯数字改为字符串百分比：`defaultSize={activeEntry ? "60%" : "100%"} minSize="15%" maxSize={activeEntry ? "80%" : "100%"}`
- [x] 1.2 将 `NewRequestView.tsx` 第 285-288 行 response 面板的 `defaultSize` / `minSize` / `collapsedSize` 由纯数字改为字符串百分比：`defaultSize="40%" minSize="10%" collapsedSize="0%"`

## 2. 验证

- [x] 2.1 运行 `bun run build:vite` 确认前端构建通过（exit 0）
- [x] 2.2 复现库解析数学（`scripts/verify-panel-units.ts`）证明：旧配置 maxSize=80→14.5% 致 response≈85.5%；新配置 maxSize="80%"→80% 致 editor=60%/response=40%，向下可拖至 response=20%
