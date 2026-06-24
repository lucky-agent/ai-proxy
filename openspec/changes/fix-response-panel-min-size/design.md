## Context

`NewRequestView.tsx` 第 286 行，response 面板（`react-resizable-panels` 的 `ResizablePanel`）的 `minSize={25}` 设置为 25%。这是面板拖拽缩小时的硬下限——当用户将分隔条拖到 <25% 位置时，面板会自动弹回 25% 或塌陷到 0（因为同时设置了 `collapsible collapsedSize={0}`）。结果：用户无法将 response 区域保持在一个 10~20% 的小尺寸。

## Goals / Non-Goals

**Goals:**
- 将 response 面板最小尺寸从 25% 降低到 10%，消除拖拽死区

**Non-Goals:**
- 不调整其他面板的尺寸参数
- 不改变面板的 collapsible 行为

## Decisions

- `minSize` 选择 10（而非 15）：与上方 editor 面板的 `minSize={15}` 相比，response 面板是内容展示区，用户可以接受更小的可见尺寸。10% 既不会太小（仍有 ~100px 在典型窗口上），也足以让用户判断是否需要 expand。

## Risks / Trade-offs

- 无风险。仅修改一个数字常量，不涉及逻辑变更。
