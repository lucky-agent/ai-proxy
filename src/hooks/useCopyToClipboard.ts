import { useState, useCallback, useRef } from 'react'

/**
 * 通用的复制到剪贴板 hook，支持自动重置 copied 状态
 */
export function useCopyToClipboard(resetMs = 1500) {
  const [copied, setCopied] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const copy = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current)
      }
      timerRef.current = setTimeout(() => setCopied(false), resetMs)
    } catch {
      // 静默失败（权限或环境不支持）
    }
  }, [resetMs])

  return { copied, copy }
}
