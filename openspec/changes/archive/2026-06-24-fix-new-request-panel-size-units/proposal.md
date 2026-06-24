## Why

在 new-request 界面中，新建请求后下方 response 面板默认占用比率过高，几乎完全覆盖 request body 区域，且向下拖动分隔条无法将 response 区域缩小。

**根因分析**：`NewRequestView.tsx` 中垂直分割（editor / response）的 `ResizablePanel` 尺寸属性全部以**纯数字**形式传入：`defaultSize={60}`、`minSize={15}`、`maxSize={80}`、`defaultSize={40}`、`minSize={10}`。

但 `react-resizable-panels@4.11.2` 的尺寸解析函数 `bt()`（`dist/react-resizable-panels.js` 第 22-31 行）对纯数字按**像素（px）**解析，只有带 `%` 后缀的字符串（或无后缀字符串，默认 `%`）才按百分比解析。

后果：
- `maxSize={80}` 实际为 80px（在约 550px 的垂直容器中≈15%），editor 面板被限制在约 15%，response 被迫占据约 85% → 完全覆盖 request body。
- editor maxSize 上限≈15%，分隔条无法向下拖过该上限 → 向下无法拖动。
- 此前 `fix-response-panel-min-size` 将 `minSize` 从 `25` 改为 `10`，但 25 与 10 都是 px，对实际百分比约束无影响，故未解决问题；`defaultSize` 55→40 的改动也被 maxSize 的 px 上限压制，同样无效。

代码库中其他正常工作的面板（`DetailPanel`、`TrafficLog`、左侧 collection 面板）均使用字符串百分比（`"50"`、`"22%"`、`"15%"`），佐证本处为单位错误。

## What Changes

- 将 `NewRequestView.tsx` 垂直分割两个 `ResizablePanel` 的 `defaultSize` / `minSize` / `maxSize` / `collapsedSize` 从纯数字改为字符串百分比（如 `"60%"`、`"15%"`、`"80%"`、`"0%"`），与库的解析语义及代码库既有约定一致。

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- 无

## Impact

- 仅 `src/features/new-request/NewRequestView.tsx` 第 261 行（editor 面板）与第 285-288 行（response 面板）的尺寸属性字面量，不涉及逻辑变更。
