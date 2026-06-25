import { useState, useMemo, memo } from 'react'
import { CheckIcon, CopyIcon, ChevronDown, ChevronRight, ArrowLeftToLine, TextWrap } from 'lucide-react'
import { useCopyToClipboard } from '@/hooks/useCopyToClipboard'
import { useTheme } from '@/hooks/useTheme'
import { useShiki } from '@/hooks/useShiki'
import { JsonTreeView } from '@/components/json-tree'

type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue }

function tryFormatJson(input: string): { formatted: string; raw: string } | null {
  if (!input) return null
  try {
    const parsed = JSON.parse(input)
    return {
      formatted: JSON.stringify(parsed, null, 2),
      raw: JSON.stringify(parsed),
    }
  } catch {
    return null
  }
}

const SyntaxHighlightedBody = memo(function SyntaxHighlightedBody({
  content,
  lang,
  wrapped,
}: {
  content: string
  lang: string
  wrapped: boolean
}) {
  const { resolvedTheme } = useTheme()
  const theme = resolvedTheme === 'dark' ? 'github-dark' : 'github-light'
  const html = useShiki(content, lang, theme)

  if (!html) {
    return (
      <pre
        className={`px-3 py-2 text-xs text-foreground/80 font-mono overflow-y-auto ${
          wrapped ? 'whitespace-pre-wrap break-all' : 'whitespace-pre overflow-x-auto'
        }`}>
        {content}
      </pre>
    )
  }

  return (
    <div
      className={`shiki-root ${wrapped ? 'whitespace-pre-wrap break-all overflow-y-auto' : 'whitespace-pre overflow-x-auto overflow-y-auto'}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
 )
})

function formatXml(input: string): string {
  const lines = input
    .replace(/\r\n/g, '\n')
    .trim()
    .replace(/>(\s*)(?=<[^!?/])/g, '>\n')
    .replace(/>\s*$/gm, '>\n')
    .replace(/^\s*</gm, '<')
    .split('\n')
    .map(l => l.trim())
    .filter(l => l.length > 0)
  let indent = 0
  let result = ""
  for (const line of lines) {
    if (line.match(/^<\//) || line.match(/^<\?/)) {
      indent--
    }
    result += '  '.repeat(Math.max(0, indent)) + line + '\n'
    if (
      /^<[^!?/]/.test(line) &&
      !line.match(/\/>\s*\$/) &&
      !line.match(/^<\?/) &&
      !line.match(/^<!--/) &&
      !line.match(/^<!\[CDATA\[/)
    ) indent++
  }
  return result.trim()
}

const BodyView = memo(function BodyView({ body, contentType }: { body: string; contentType?: string }) {
  const [wrapped, setWrapped] = useState(true)
  const [format, setFormat] = useState<'auto' | 'json' | 'xml' | 'html' | 'plaintext'>('auto')
  const [allExpanded, setAllExpanded] = useState(true)
  const { copied, copy } = useCopyToClipboard()

  const { cleaned, formatted, parsedJson, isJson } = useMemo(() => {
    const c = body.replace(/\r\n/g, '\n').replace(/\r/g, '\n').trim()
    const f = tryFormatJson(c)
    let parsed: JsonValue | null = null
    if (f !== null) {
      try { parsed = JSON.parse(c) as JsonValue } catch { /* ignore */ }
    }
    return { cleaned: c, formatted: f, parsedJson: parsed, isJson: f !== null }
  }, [body])

  const { displayBody, displayLang, useTreeView } = useMemo(() => {
    const inferFromContentType = (ct?: string): { displayLang: string; useTreeView: false } | null => {
      if (!ct) return null
      const lower = ct.toLowerCase()
      if (lower.includes('json')) return { displayLang: 'json', useTreeView: false }
      if (lower.includes('xml') && !lower.includes('html')) return { displayLang: 'xml', useTreeView: false }
      if (lower.includes('html')) return { displayLang: 'html', useTreeView: false }
      return null
    }

    if (format === 'auto') {
      if (isJson) {
        return { displayBody: formatted!.formatted, displayLang: 'json', useTreeView: true as const }
      }
      const ctHint = inferFromContentType(contentType)
      if (ctHint) {
        return { displayBody: cleaned, displayLang: ctHint.displayLang, useTreeView: ctHint.useTreeView }
      }
      const l = cleaned.startsWith('<') ? 'html' : 'plaintext'
      return { displayBody: cleaned, displayLang: l, useTreeView: false as const }
    }
    if (format === 'json') {
      try { const parsed = JSON.parse(cleaned) as JsonValue; return { displayBody: JSON.stringify(parsed, null, 2), displayLang: 'json', useTreeView: true as const } }
      catch { return { displayBody: cleaned, displayLang: 'plaintext', useTreeView: false as const } }
    }
    if (format === 'xml') {
      try { return { displayBody: formatXml(cleaned), displayLang: 'xml', useTreeView: false as const } }
      catch { return { displayBody: cleaned, displayLang: 'plaintext', useTreeView: false as const } }
    }
    if (format === 'html') { return { displayBody: cleaned, displayLang: 'html', useTreeView: false as const } }
    return { displayBody: cleaned, displayLang: 'plaintext', useTreeView: false as const }
  }, [format, cleaned, isJson, formatted, contentType])

  return (
    <div className="flex flex-col h-full">
      <div className="relative min-h-0 flex-1 group/mini">
        <div className="absolute top-1.5 right-1.5 z-10 flex items-center gap-0.5 opacity-0 group-hover/mini:opacity-100 transition-all">
          <select
            value={format}
            onChange={(e) => setFormat(e.target.value as typeof format)}
            className="appearance-none rounded bg-surface-elevated/30 px-1.5 py-0.5 text-[10px] text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-colors cursor-pointer outline-none border border-surface-elevated/30"
            title="Format">
            <option value="auto">Auto</option>
            <option value="json">JSON</option>
            <option value="xml">XML</option>
            <option value="html">HTML</option>
            <option value="plaintext">Text</option>
          </select>
          <button
            onClick={() => setWrapped(w => !w)}
            className={`rounded p-1 transition-colors ${
              wrapped
                ? 'text-foreground bg-surface-elevated/50'
                : 'text-muted-foreground hover:text-foreground hover:bg-surface-elevated/30'
            }`}
            title={wrapped ? 'Disable wrap' : 'Enable wrap'}>
            {wrapped ? <ArrowLeftToLine className="size-3" /> : <TextWrap className="size-3" />}
          </button>
          {useTreeView && (
            <button
              onClick={allExpanded ? () => setAllExpanded(false) : () => setAllExpanded(true)}
              className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/30 transition-colors"
              title={allExpanded ? 'Collapse all' : 'Expand all'}>
              {allExpanded ? (
                <ChevronDown className="size-3" />
              ) : (
                <ChevronRight className="size-3" />
              )}
            </button>
          )}
          <button
            onClick={() => copy(displayBody)}
            className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/30 transition-colors"
            title={copied ? 'Copied' : 'Copy body'}>
            {copied ? (
              <CheckIcon className="size-3 text-primary" />
            ) : (
              <CopyIcon className="size-3" />
            )}
          </button>
        </div>
        <div className="absolute inset-0 overflow-auto">
          {useTreeView && parsedJson ? (
            <JsonTreeView
              data={parsedJson}
              defaultExpanded={allExpanded}
              depth={0}
              wrapped={wrapped}
            />
          ) : (
            <SyntaxHighlightedBody content={displayBody} lang={displayLang} wrapped={wrapped} />
          )}
        </div>
      </div>
    </div>
  )
})

export default BodyView
