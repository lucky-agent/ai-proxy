import { useCallback, useEffect, useRef, useState } from "react";

/** 与 index.css 中 `.animate-slide-out-right` 的时长一致 */
const CLOSE_ANIMATION_MS = 150;

/** 侧滑编辑面板的目标：`index === null` 表示新增，数字表示编辑对应行 */
export interface SlidePanelTarget {
  index: number | null;
}

/**
 * 配置弹窗里「列表 + 侧滑编辑」的面板状态：打开（新增 / 编辑）、带退场动画的关闭、强制重置。
 * 配合 `ConfigDialog` 的 `panel` / `panelClosing` / `onPanelDismiss` 使用。
 */
export function useSlidePanel() {
  const [target, setTarget] = useState<SlidePanelTarget | null>(null);
  const [closing, setClosing] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearTimer = useCallback(() => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
  }, []);

  useEffect(() => clearTimer, [clearTimer]);

  const open = useCallback(
    (index: number | null) => {
      clearTimer();
      setTarget({ index });
      setClosing(false);
    },
    [clearTimer],
  );

  /** 播放退场动画后卸载面板 */
  const close = useCallback(() => {
    clearTimer();
    setClosing(true);
    timer.current = setTimeout(() => {
      timer.current = null;
      setTarget(null);
      setClosing(false);
    }, CLOSE_ANIMATION_MS);
  }, [clearTimer]);

  /** 无动画立即重置（弹窗重新打开时用） */
  const reset = useCallback(() => {
    clearTimer();
    setTarget(null);
    setClosing(false);
  }, [clearTimer]);

  return { target, closing, open, close, reset };
}
