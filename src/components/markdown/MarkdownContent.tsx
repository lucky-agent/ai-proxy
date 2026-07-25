import { Component, useCallback, type ReactNode, type MouseEvent } from 'react'
import { Streamdown, type ThemeInput } from 'streamdown'
import { invoke } from '@tauri-apps/api/core'
import { cn } from '@/lib/utils'

interface MarkdownContentProps {
  text: string
  /** inverted：蓝底 user 气泡等深色实底场景，启用白色系覆盖样式 */
  variant?: 'default' | 'inverted'
}

/** md 渲染抛错时降级为纯文本，不影响气泡其余部分 */
class MdErrorBoundary extends Component<
  { fallback: ReactNode; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false }
  static getDerivedStateFromError() {
    return { failed: true }
  }
  render() {
    return this.state.failed ? this.props.fallback : this.props.children
  }
}

/** 模块级常量：保持引用稳定，避免破坏 Streamdown 的 memo 比较导致整体重解析 */
const SHIKI_THEMES: [ThemeInput, ThemeInput] = ['github-light', 'github-dark']

/** 通用 Markdown 渲染：streamdown（GFM + shiki 双主题 + 未闭合语法补全）。
 *  链接点击通过 Tauri command `open_url` 交系统浏览器打开。 */
export function MarkdownContent({ text, variant = 'default' }: MarkdownContentProps) {
  const handleClickCapture = useCallback((e: MouseEvent) => {
    const a = (e.target as HTMLElement).closest('a')
    if (!a) return
    const href = a.getAttribute('href')
    if (href && /^https?:\/\//i.test(href)) {
      e.preventDefault()
      invoke('open_url', { url: href }).catch(() => {})
    }
  }, [])

  return (
    <MdErrorBoundary fallback={<span className="whitespace-pre-wrap">{text}</span>}>
      <div
        onClickCapture={handleClickCapture}
        className={cn('md-content', variant === 'inverted' && 'md-inverted')}
      >
        <Streamdown linkSafety={{ enabled: false }} shikiTheme={SHIKI_THEMES}>{text}</Streamdown>
      </div>
    </MdErrorBoundary>
  )
}
