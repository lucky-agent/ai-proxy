import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { TrafficEntry } from '@/types/proxy'
import type { AiConversation, AiTurn, AiContentBlock } from '@/types/ai'
import { JsonTreeView } from '@/components/json-tree'

type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue }

/** 将单个 content block 渲染为纯文本摘要 */
function blockSummary(block: AiContentBlock): string {
  if (block.type === 'text') return block.text.slice(0, 200) + (block.text.length > 200 ? '…' : '')
  if (block.type === 'thinking') return `🧠 ${block.text.slice(0, 120)}${block.text.length > 120 ? '…' : ''}`
  if (block.type === 'tool_use') return `🔧 ${block.name}(${JSON.stringify(block.input).slice(0, 80)}${JSON.stringify(block.input).length > 80 ? '…' : ''})`
  if (block.type === 'tool_result') {
    const inner = block.content.map(b => blockSummary(b)).join(' | ')
    return `📋 ${inner.slice(0, 120)}${inner.length > 120 ? '…' : ''}`
  }
  return ''
}

function roleLabel(turn: AiTurn): string {
  switch (turn.role) {
    case 'system': return 'System'
    case 'user': return 'User'
    case 'assistant': return 'Assistant'
    case 'tool': return 'Tool'
    case 'tools_def': return 'Tools Def'
  }
}

function roleColor(turn: AiTurn): string {
  switch (turn.role) {
    case 'system': return 'border-purple-500/30 bg-purple-500/5'
    case 'user': return 'border-blue-500/30 bg-blue-500/5'
    case 'assistant': return 'border-emerald-500/30 bg-emerald-500/5'
    case 'tool': return 'border-amber-500/30 bg-amber-500/5'
    case 'tools_def': return 'border-violet-500/30 bg-violet-500/5'
  }
}

interface Props {
  entry: TrafficEntry
  conversation: AiConversation
}

export default function CompareView({ entry, conversation }: Props) {
  const { t } = useTranslation()

  const rawJson = useMemo(() => {
    if (!entry.responseBody) return null
    try {
      return JSON.parse(entry.responseBody) as JsonValue
    } catch {
      return null
    }
  }, [entry.responseBody])

  return (
    <div className="flex h-full min-h-0 divide-x divide-border">
      {/* 左侧：归一化 IR */}
      <div className="flex-1 min-w-0 overflow-y-auto">
        <div className="sticky top-0 z-10 px-3 py-1.5 text-ui-xs font-semibold uppercase tracking-wider text-muted-foreground bg-surface-base/80 backdrop-blur-sm border-b border-border">
          {t('detail.compareIR', '归一化 IR')}
          {conversation.streaming && (
            <span className="ml-1.5 text-amber-500 animate-pulse">● streaming</span>
          )}
        </div>
        <div className="p-2 space-y-1.5">
          {conversation.turns.length === 0 ? (
            <div className="flex items-center justify-center py-8 text-xs text-muted-foreground">
              {t('detail.compareWaiting', '等待归一化数据…')}
            </div>
          ) : (
            conversation.turns.map((turn, i) => (
              <div
                key={i}
                className={`rounded border px-2.5 py-1.5 text-prose-md ${roleColor(turn)}`}
              >
                <div className="flex items-center gap-1.5 mb-1">
                  <span className="text-ui-xs font-semibold text-muted-foreground/70 uppercase">
                    {roleLabel(turn)}
                  </span>
                  {i === 0 && (
                    <span className="text-ui-2xs text-muted-foreground/40">(#{i + 1})</span>
                  )}
                </div>
                {turn.content.map((block, j) => (
                  <div key={j} className="text-prose-sm leading-relaxed whitespace-pre-wrap break-all text-foreground/80">
                    {blockSummary(block)}
                  </div>
                ))}
              </div>
            ))
          )}
        </div>
      </div>

      {/* 右侧：原始 JSON */}
      <div className="flex-1 min-w-0 overflow-y-auto">
        <div className="sticky top-0 z-10 px-3 py-1.5 text-ui-xs font-semibold uppercase tracking-wider text-muted-foreground bg-surface-base/80 backdrop-blur-sm border-b border-border">
          {t('detail.compareRaw', '原始 JSON')}
        </div>
        <div className="p-2">
          {rawJson != null ? (
            <JsonTreeView data={rawJson} defaultExpanded depth={0} wrapped />
          ) : entry.responseBody ? (
            <pre className="text-prose-sm font-mono text-foreground/70 whitespace-pre-wrap break-all px-2 py-1">
              {entry.responseBody}
            </pre>
          ) : (
            <div className="flex items-center justify-center py-8 text-xs text-muted-foreground">
              {t('detail.noBody', '无正文')}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
