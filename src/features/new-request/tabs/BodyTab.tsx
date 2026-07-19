import { useMemo } from 'react'
import { useLocale } from '@/hooks/useLocale'
import CodeEditor from '@/components/code-editor/CodeEditor'
import { AlignJustifyIcon } from 'lucide-react'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { BodyType } from '@/types/collection'

interface BodyTabProps {
  body: string
  bodyType: BodyType
  onBodyChange: (body: string) => void
  onBodyTypeChange: (bodyType: BodyType) => void
}

const BODY_FORMATS: { value: BodyType; labelKey: string }[] = [
  { value: 'json', labelKey: 'requestEditor.bodyFormatJson' },
  { value: 'xml', labelKey: 'requestEditor.bodyFormatXml' },
  { value: 'text', labelKey: 'requestEditor.bodyFormatText' },
]

function formatJson(input: string): string | null {
  if (!input.trim()) return null
  try {
    const parsed = JSON.parse(input)
    return JSON.stringify(parsed, null, 2)
  } catch {
    return null
  }
}

function minifyJson(input: string): string | null {
  if (!input.trim()) return null
  try {
    const parsed = JSON.parse(input)
    return JSON.stringify(parsed)
  } catch {
    return null
  }
}

/** JSON 是否已是多行美化格式（用于判断要做美化还是压缩） */
function isPrettyJson(input: string): boolean {
  const trimmed = input.trim()
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) return false
  return trimmed.includes('\n')
}

function formatOrMinifyJson(input: string): string | null {
  return isPrettyJson(input) ? minifyJson(input) : formatJson(input)
}

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

/** Determine the effective format for beautify, considering auto-detection */
function detectType(bodyType: BodyType, content: string): 'json' | 'xml' | null {
  if (bodyType === 'json') return 'json'
  if (bodyType === 'xml') {
    const trimmed = content.trim()
    return trimmed.startsWith('<') ? 'xml' : null
  }
  return null
}

export default function BodyTab({ body, bodyType, onBodyChange, onBodyTypeChange }: BodyTabProps) {
  const { t } = useLocale()

  const effectiveType = useMemo(() => detectType(bodyType, body), [bodyType, body])

  const handleBeautify = () => {
    if (!effectiveType) return
    let formatted: string | null = null
    if (effectiveType === 'json') {
      formatted = formatOrMinifyJson(body)
    } else if (effectiveType === 'xml') {
      formatted = formatXml(body)
      if (formatted === body.trim()) formatted = null
    }
    if (formatted) onBodyChange(formatted)
  }

  return (
    <div className="p-4 min-h-0 flex flex-col flex-1">
      <div className="flex-1 min-h-[180px] border rounded-md overflow-hidden relative group/body">
        <div className="absolute top-1.5 right-1.5 z-10 flex items-center gap-0.5 opacity-0 group-hover/body:opacity-100 transition-all">
          <Select
            value={bodyType}
            onValueChange={v => onBodyTypeChange(v as BodyType)}
          >
            <SelectTrigger
              size="sm"
              className="h-[22px] py-0 border-0 shadow-none rounded bg-surface-elevated/30 px-1.5 text-ui-xs text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-colors w-auto gap-0.5 [&_svg]:size-3"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent align="end" side="bottom" className="!min-w-0 [&_[data-slot=select-item]]:py-1 [&_[data-slot=select-item]]:text-xs">
              {BODY_FORMATS.map(fmt => (
                <SelectItem key={fmt.value} value={fmt.value}>{t(fmt.labelKey)}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          {effectiveType && (
            <button
              onClick={handleBeautify}
              className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/30 transition-colors"
            >
              <AlignJustifyIcon className="size-3" />
            </button>
          )}
        </div>
        <CodeEditor value={body} language={bodyType} onChange={onBodyChange} />
      </div>
    </div>
  )
}
