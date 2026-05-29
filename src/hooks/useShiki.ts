import { useState, useEffect, useRef } from 'react'

type CodeToHtmlFn = (code: string, options: { lang: string; theme: string }) => Promise<string>

// 模块级缓存，避免每次渲染都动态 import
let _codeToHtml: CodeToHtmlFn | null = null

async function getCodeToHtml(): Promise<CodeToHtmlFn> {
  if (!_codeToHtml) {
    const { codeToHtml } = await import('shiki')
    _codeToHtml = codeToHtml
  }
  return _codeToHtml!
}

/**
 * 使用 shiki 进行语法高亮，自动处理异步加载和清理
 */
export function useShiki(
  code: string,
  lang: string,
  theme: string,
): string {
  const [html, setHtml] = useState('')
  const codeRef = useRef(code)
  codeRef.current = code

  useEffect(() => {
    let cancelled = false

    ;(async () => {
      const codeToHtml = await getCodeToHtml()
      if (cancelled) return

      const result = await codeToHtml(codeRef.current, {
        lang,
        theme,
      })
      // 去掉 .line 之间多余的换行符，避免 pre-wrap 下多出空行
      const fixed = result.replace(/<\/span>\n(?=<span class="line")/g, '</span>')
      if (!cancelled) {
        setHtml(fixed)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [code, lang, theme])

  return html
}
