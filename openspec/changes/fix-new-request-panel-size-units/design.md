## Context

`NewRequestView.tsx` 第 260-295 行的垂直 `ResizablePanelGroup`（id=`new-request-vertical`）包含两个面板：
- editor 面板（上）：`defaultSize={activeEntry ? 60 : 100} minSize={15} maxSize={activeEntry ? 80 : 100}`
- response 面板（下）：`defaultSize={40} minSize={10} collapsible collapsedSize={0}`

这些值以纯数字传入，被 `react-resizable-panels@4.11.2` 当作像素解析，导致实际百分比约束严重偏离预期（maxSize=80px≈15%，editor 被压制、response 膨胀至约 85%）。

参考实现（同文件第 148 行左侧 collection 面板、`DetailPanel.tsx`、`TrafficLog.tsx`）均使用字符串百分比，是代码库既有正确约定。

## Goals / Non-Goals

**Goals:**
- 将垂直分割两个面板的尺寸属性统一为字符串百分比，恢复预期的百分比约束语义
- 使 response 默认占比合理（约 40%），不再覆盖 request body
- 使用户能向下拖动分隔条缩小 response 区域（至 minSize=10%）

**Non-Goals:**
- 不调整水平分割（collection / right）面板——其已正确使用字符串百分比
- 不调整 `DetailPanel` / `TrafficLog` 等其他视图的面板（其中 `TrafficLog` 第 250 行 `defaultSize={18}` 存在同类 px 隐患，但不在本次修复范围）
- 不引入布局持久化（`useDefaultLayout`）

## Decisions

- **采用用户已在工作区调整过的比例 60/40**（editor 60% / response 40%）：用户已通过未提交改动表达该偏好（editor defaultSize 45→60、response 55→40），只是因单位 bug 未生效。保留 60/40 并修正单位，既修复根因又符合用户意图。
- **maxSize 采用 80%**（response 可缩至 20%）：与用户未提交改动一致，给 editor 留出足够上限，同时 response 仍可缩到较小尺寸。
- **minSize 用 15%（editor）/ 10%（response）**：与现有提交值一致，仅转换单位。
- **字符串形式用带 `%` 后缀**（如 `"60%"` 而非 `"60"`）：虽然无后缀字符串也默认按 `%` 解析，但带 `%` 更显式、与左侧 collection 面板风格一致，避免歧义。
- **collapsedSize 用 `"0%"`**：0px 与 0% 数值等价，统一为字符串百分比以保持一致性。

## Risks / Trade-offs

- 风险极低：仅字面量单位转换，无逻辑变更。
- 注意 `defaultSize` 仅在面板首次挂载时生效；PanelGroup 在切换 tab / 发送请求时不会重新挂载，故 defaultSize 的变更只在首次进入 new-request 视图时体现——这符合预期，且 collapse/expand 的 useEffect 逻辑仍控制有/无 response 时的折叠。
