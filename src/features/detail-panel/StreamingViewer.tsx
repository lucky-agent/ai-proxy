import { useState, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { CopyIcon, CheckIcon, ChevronRight, ChevronDown, ListChecks } from 'lucide-react'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { TrafficEntry } from '@/types/proxy'
import { parseSse, isStreamingContentType, mergeSseMessages, estimateTokens, extractTokenUsage, type SseEvent, type MergeFormat, type MergeResult, type TokenUsage } from '@/lib/sse'
import { useCopyToClipboard } from '@/hooks/useCopyToClipboard'

type ViewMode = 'chunks' | 'events' | 'merged'

interface Props {
  entry: TrafficEntry
  onClose?: () => void
}

const MERGE_FORMATS: { value: MergeFormat; labelKey: string }[] = [
  { value: 'openai', labelKey: 'detail.mergeFormatOpenai' },
]

export default function StreamingViewer({ entry }: Props) {
  const { t } = useTranslation()
  const [view, setView] = useState<ViewMode>('chunks')
  const [mergeFormat, setMergeFormat] = useState<MergeFormat>('openai')

  const isSse = isStreamingContentType(entry.responseHeaders)
  const chunks = entry.responseChunks ?? []

  const sseEvents = useMemo(() => {
    if (!isSse || !entry.responseBody) return []
    return parseSse(entry.responseBody)
  }, [isSse, entry.responseBody])

  const mergedResult = useMemo(() => {
    if (!isSse || sseEvents.length === 0) return null
    return mergeSseMessages(sseEvents, mergeFormat)
  }, [isSse, sseEvents, mergeFormat])

  const tokenUsage = useMemo(() => {
    if (!isSse) return null
    return extractTokenUsage(sseEvents)
  }, [isSse, sseEvents])

  const chunkStats = useMemo(() => {
    let totalBytes = 0
    for (const c of chunks) totalBytes += c.data.length
    return { count: chunks.length, totalBytes }
  }, [chunks])

  const hasMerged = mergedResult !== null && mergedResult.content.length > 0

  const tabValue = view

  return (
    <Tabs value={tabValue} onValueChange={(v) => setView(v as ViewMode)} className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <TabsList variant="line" className="shrink-0 justify-start border-b border-surface-elevated bg-surface-elevated/20 px-0 rounded-none">
        <TabsTrigger
          value="chunks"
          className="text-[11px]"
          title={`${chunkStats.count} ${t('detail.streamChunks')} · ${chunkStats.totalBytes} B`}
        >
          {t('detail.streamChunks')}
        </TabsTrigger>
        {isSse && (
          <TabsTrigger
            value="events"
            className="text-[11px]"
            title={`${sseEvents.length} ${t('detail.streamEvents')}`}
          >
            {t('detail.streamEvents')}
          </TabsTrigger>
        )}
        {isSse && hasMerged && (
          <TabsTrigger
            value="merged"
            className="text-[11px]"
            title={`${mergedResult!.eventCount} ${t('detail.streamEvents')} → ${t('detail.streamMerged')}`}
          >
            {t('detail.streamMerged')}
          </TabsTrigger>
        )}
        {/* 合并选项：选中合并 Tab 时显示在同一行 */}
        {view === 'merged' && hasMerged && (
          <div className="ml-auto flex items-center gap-1.5 px-2">
            <span className="flex items-center gap-1 text-[10px] text-muted-foreground/60">
              <ListChecks className="size-2.5" />
            </span>
            <select
              value={mergeFormat}
              onChange={(e) => setMergeFormat(e.target.value as MergeFormat)}
              className="rounded border-b border-surface-elevated/50 bg-transparent px-1.5 py-0.5 text-[10px] text-foreground outline-none focus:border-primary"
            >
              {MERGE_FORMATS.map((fmt) => (
                <option key={fmt.value} value={fmt.value}>
                  {t(fmt.labelKey)}
                </option>
              ))}
            </select>
            <span className="text-[10px] text-muted-foreground/50 tabular-nums">
              {sseEvents.length} · {mergedResult!.content.length} B
            </span>
          </div>
        )}
      </TabsList>

      <TabsContent value="chunks" className="min-h-0 flex-1 overflow-hidden mt-0">
        <ScrollArea className="h-full">
        {chunks.length === 0 ? (
          <EmptyState label={t('detail.noStreamData')} />
        ) : (
          chunks.map((chunk, idx) => (
            <ChunkItem key={idx} index={idx} data={chunk.data} totalChunks={chunks.length} />
          ))
        )}
        </ScrollArea>
      </TabsContent>

      <TabsContent value="events" className="min-h-0 flex-1 overflow-hidden mt-0">
        <ScrollArea className="h-full">
        {sseEvents.length === 0 ? (
          <EmptyState label={t('detail.noSseEvents')} />
        ) : (
          sseEvents.map((evt, idx) => <SseEventItem key={idx} index={idx} event={evt} />)
        )}
        </ScrollArea>
      </TabsContent>

      <TabsContent value="merged" className="min-h-0 flex-1 overflow-hidden mt-0">
        <ScrollArea className="h-full">
        {mergedResult ? (
          <MergedView result={mergedResult} tokenUsage={tokenUsage} />
        ) : (
          <EmptyState label={t('detail.noSseEvents')} />
        )}
        </ScrollArea>
      </TabsContent>
    </Tabs>
  )
}

// -------------------------------------------------------------------
// EmptyState
// -------------------------------------------------------------------
function EmptyState({ label }: { label: string }) {
  return (
    <div className="flex items-center justify-center py-10 text-sm text-muted-foreground">
      {label}
    </div>
  )
}

// -------------------------------------------------------------------
// MergedView — 合并消息展示
// -------------------------------------------------------------------
function MergedView({ result, tokenUsage }: { result: MergeResult; tokenUsage: TokenUsage | null }) {
  const { t } = useTranslation()
  const { copied, copy } = useCopyToClipboard()
  const estTokens = useMemo(() => estimateTokens(result.content), [result.content])

  return (
    <div className="flex flex-col h-full">
      <div className="relative min-h-0 flex-1 group">
        <button
          onClick={() => copy(result.formatted)}
          className="absolute right-1.5 top-1.5 z-10 rounded p-1 text-muted-foreground opacity-0 hover:text-foreground hover:bg-surface-elevated/50 transition-all group-hover:opacity-100"
          title={copied ? t('detail.copied') : t('detail.copyUri')}>
          {copied ? <CheckIcon className="size-3 text-primary" /> : <CopyIcon className="size-3" />}
        </button>
        {/* Token — 右下角固定 */}
        <div className="absolute bottom-1.5 right-2 z-10 flex items-center gap-1.5 rounded bg-background/70 px-1.5 py-0.5 text-[10px] text-muted-foreground/60 tabular-nums backdrop-blur-sm select-none">
          <span>
            {tokenUsage?.totalTokens != null ? tokenUsage.totalTokens : '\u2248' + estTokens} tokens
          </span>
          {tokenUsage?.totalTokens == null && <span className="text-muted-foreground/40">（估）</span>}
          {tokenUsage?.cachedTokens != null && tokenUsage.cachedTokens > 0 && (
            <>
              <span className="text-muted-foreground/20 mx-0.5">|</span>
              <span className="text-emerald-600 dark:text-emerald-400">缓存 {tokenUsage.cachedTokens}</span>
            </>
          )}
        </div>
        <pre className="overflow-x-auto whitespace-pre-wrap break-all px-3 py-2 pb-7 font-mono text-[12px] leading-6 text-foreground/80">
          {result.formatted}
        </pre>
      </div>
    </div>
  )
}

// -------------------------------------------------------------------
// ChunkItem
// -------------------------------------------------------------------
function ChunkItem({
  index,
  data,
  totalChunks,
}: {
  index: number
  data: string
  totalChunks: number
}) {
  const [expanded, setExpanded] = useState(false)
  const trimmed = data.replace(/\n$/, '')
  const isLast = index === totalChunks - 1
  // Detect [DONE] marker
  const isDone = trimmed.trim() === '[DONE]'

  return (
    <div
      className={`border-b border-surface-elevated/30 transition-colors hover:bg-surface-elevated/10 ${
        isDone ? 'opacity-60' : ''
      }`}>
      {/* Summary line */}
      <button
        onClick={() => setExpanded(e => !e)}
        className="flex w-full items-start gap-1.5 px-3 py-1.5 text-left text-xs">
        {expanded ? (
          <ChevronDown className="mt-0.5 size-3 shrink-0 text-muted-foreground/60" />
        ) : (
          <ChevronRight className="mt-0.5 size-3 shrink-0 text-muted-foreground/60" />
        )}
        <span className="shrink-0 font-medium text-foreground/80">#{index + 1}</span>
        <span className="shrink-0 text-muted-foreground tabular-nums">{data.length} B</span>
        <span className="min-w-0 flex-1 truncate text-muted-foreground/70 font-mono">
          {trimmed.slice(0, 120)}
          {trimmed.length > 120 ? '...' : ''}
        </span>
        {isDone && (
          <span className="shrink-0 rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-600 dark:text-amber-400">
            [DONE]
          </span>
        )}
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="px-3 pb-2 pl-9">
          <ChunkContent data={data} />
        </div>
      )}
    </div>
  )
}

// -------------------------------------------------------------------
// ChunkContent — 显示带复制功能的 chunk 内容
// -------------------------------------------------------------------
function ChunkContent({ data }: { data: string }) {
  const { copied, copy } = useCopyToClipboard()

  return (
    <div className="group relative">
      <button
        onClick={() => copy(data)}
        className="absolute right-1 top-1 z-10 rounded p-1 text-muted-foreground opacity-0 hover:text-foreground hover:bg-surface-elevated/50 transition-all group-hover:opacity-100">
        {copied ? <CheckIcon className="size-3 text-primary" /> : <CopyIcon className="size-3" />}
      </button>
      <pre className="overflow-x-auto whitespace-pre-wrap break-all rounded bg-surface-elevated/20 px-3 py-2 font-mono text-[11px] leading-5 text-foreground/80">
        {data}
      </pre>
    </div>
  )
}

// -------------------------------------------------------------------
// SseEventItem
// -------------------------------------------------------------------
function SseEventItem({ index, event }: { index: number; event: SseEvent }) {

  const [expanded, setExpanded] = useState(false)
  const { copied, copy } = useCopyToClipboard()
  const formattedData = useMemo(() => {
    try {
      const parsed = JSON.parse(event.data)
      return JSON.stringify(parsed, null, 2)
    } catch {
      return event.data
    }
  }, [event.data])

  // Detect [DONE] marker
  const isDone = event.data.trim() === '[DONE]'

  return (
    <div
      className={`border-b border-surface-elevated/30 transition-colors hover:bg-surface-elevated/5 ${
        isDone ? 'opacity-50' : ''
      }`}>
      {/* Summary line */}
      <button
        onClick={() => setExpanded(e => !e)}
        className="flex w-full items-start gap-1.5 px-3 py-1.5 text-left text-xs">
        {expanded ? (
          <ChevronDown className="mt-0.5 size-3 shrink-0 text-muted-foreground/60" />
        ) : (
          <ChevronRight className="mt-0.5 size-3 shrink-0 text-muted-foreground/60" />
        )}
        <span className="shrink-0 font-medium text-foreground/80">#{index + 1}</span>
        <span className="shrink-0 rounded bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary">
          {event.event}
        </span>
        {event.id && (
          <span className="shrink-0 text-muted-foreground/70 font-mono tabular-nums">
            id: {event.id}
          </span>
        )}
        <span className="min-w-0 flex-1 truncate text-muted-foreground/50 font-mono">
          {isDone ? '[DONE]' : event.data.slice(0, 80) + (event.data.length > 80 ? '...' : '')}
        </span>
        {!isDone && event.data.length > 80 && (
          <span className="shrink-0 text-muted-foreground/40 text-[10px]">
            {event.data.length} B
          </span>
        )}
      </button>

      {/* Expanded content */}
      {expanded && !isDone && (
        <div className="group relative px-3 pb-2 pl-9">
          <button
            onClick={() => copy(event.data)}
            className="absolute right-1 top-1 z-10 rounded p-1 text-muted-foreground opacity-0 hover:text-foreground hover:bg-surface-elevated/50 transition-all group-hover:opacity-100">
            {copied ? (
              <CheckIcon className="size-3 text-primary" />
            ) : (
              <CopyIcon className="size-3" />
            )}
          </button>
          <pre className="overflow-x-auto whitespace-pre-wrap break-all rounded bg-surface-elevated/20 px-3 py-1 font-mono text-[11px] leading-5 text-foreground/80">
            {formattedData}
          </pre>
        </div>
      )}
    </div>
  )
}
