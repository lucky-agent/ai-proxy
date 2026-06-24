## Why

在 new-request 界面中，发送请求后下方 response 面板的 `minSize={25}`（25%）设置过大。用户拖拽分隔条缩小 response 区域时，要么保持在 ≥25% 以上的较大可见区域，要么直接塌陷到 0 完全隐藏。用户无法将 response 区域缩小到 10~20% 之间的小尺寸——这是一个不合理的拖拽范围间隙。

## What Changes

- 将 `NewRequestView.tsx` 中 response 面板的 `minSize` 从 `25` 降低到 `10`，使用户能将 response 区域缩小至 10% 的小尺寸再塌陷

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- 无

## Impact

- 仅 `src/features/new-request/NewRequestView.tsx` 第 286 行，一个常量值的更改
