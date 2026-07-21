import { useRef } from 'react'
import { useShikiHighlighter } from 'react-shiki'

/**
 * 使用 react-shiki 进行语法高亮，返回 React 元素。
 * 缓存上一次有效结果：高亮计算中（返回 null）时沿用旧值，消除异步加载导致的闪烁。
 */
export function useShiki(code: string, lang: string, theme: string): React.ReactElement | null {
  const result = useShikiHighlighter(code, lang as any, theme, { addDefaultStyles: false })
  const cachedRef = useRef<React.ReactElement | null>(null)

  if (result !== null) {
    cachedRef.current = result
    return result
  }

  // 高亮尚未完成（WASM 加载 / 异步计算中），返回上一次的有效结果避免 fallback 闪烁
  return cachedRef.current
}
