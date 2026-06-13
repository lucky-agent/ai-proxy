import { useTranslation } from 'react-i18next'
import { useState, useCallback, useRef, useMemo, useEffect, memo, type ReactNode } from 'react'
import type { TrafficEntry } from '@/types/proxy'
import FormDataView from './FormDataView'
import StreamingViewer from './StreamingViewer'
import { useTheme } from '@/hooks/useTheme'
import { useShiki } from '@/hooks/useShiki'
import { useCopyToClipboard } from '@/hooks/useCopyToClipboard'
import { statusCategory, formatDuration } from '@/lib/format'
import { isStreamingContentType } from '@/lib/sse'
import {
  CopyIcon,
  CheckIcon,
  TextWrap,
 ArrowLeftToLine,
  ListRestart,
 ChevronRight,
 ChevronDown,
  XIcon,
} from 'lucide-react'

const MIN_REQUEST_RATIO = 0.15
const MAX_REQUEST_RATIO = 0.85

type PanelTab = 'header' | 'query' | 'body' | 'raw' | 'form' | 'stream'

type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue }

interface Props {
  entry: TrafficEntry | undefined
  onClose?: () => void
}

const TABS: { id: PanelTab; labelKey: string }[] = [
  { id: 'header', labelKey: 'detail.headers' },
  { id: 'body', labelKey: 'detail.body' },
  { id: 'raw', labelKey: 'detail.raw' },
]

const requestTabs: (hasQuery: boolean) => { id: PanelTab; labelKey: string }[] = (hasQuery) => {
  const base: { id: PanelTab; labelKey: string }[] = [
    { id: 'header', labelKey: 'detail.headers' },
  ]
  if (hasQuery) base.push({ id: 'query', labelKey: 'detail.query' })
  base.push({ id: 'form', labelKey: 'detail.formData' })
  base.push(
    { id: 'body', labelKey: 'detail.body' },
    { id: 'raw', labelKey: 'detail.raw' },
  )
  return base
}

// ---------------------------------------------------------------------------
// DetailPanel — 入口
// ---------------------------------------------------------------------------
export default function DetailPanel({ entry, onClose }: Props) {
  const { t } = useTranslation()
  const [requestTab, setRequestTab] = useState<PanelTab>('header')
  const [responseTab, setResponseTab] = useState<PanelTab>('header')
  const [requestRatio, setRequestRatio] = useState(0.5)
    const [dragging, setDragging] = useState(false)

  const hasQuery = entry ? !!entry.requestQuery && Object.keys(entry.requestQuery).length > 0 : false
  const reqTabs = requestTabs(hasQuery)
  const respTabs = useMemo(() => {
    if (!entry) {
      return [
        { id: 'header' as PanelTab, labelKey: 'detail.headers' },
        { id: 'body' as PanelTab, labelKey: 'detail.body' },
        { id: 'raw' as PanelTab, labelKey: 'detail.raw' },
      ]
    }
    const hasStream = (entry.responseChunks?.length ?? 0) > 1 || isStreamingContentType(entry.responseHeaders)
    if (hasStream) {
      return [
        { id: 'header' as PanelTab, labelKey: 'detail.headers' },
        { id: 'body' as PanelTab, labelKey: 'detail.body' },
        { id: 'stream' as PanelTab, labelKey: 'detail.stream' },
        { id: 'raw' as PanelTab, labelKey: 'detail.raw' },
      ]
    }
    return [
      { id: 'header' as PanelTab, labelKey: 'detail.headers' },
      { id: 'body' as PanelTab, labelKey: 'detail.body' },
      { id: 'raw' as PanelTab, labelKey: 'detail.raw' },
    ]
  }, [entry])

  // 切换条目时如果 stream tab 不再可用，切回 header
  useEffect(() => {
    if (responseTab === 'stream') {
      const hasStream = entry && ((entry.responseChunks?.length ?? 0) > 1 || isStreamingContentType(entry.responseHeaders))
      if (!hasStream) setResponseTab('header')
    }
  }, [entry, responseTab])

  const containerRef = useRef<HTMLDivElement>(null)
  const requestPanelRef = useRef<HTMLDivElement>(null)
  const responsePanelRef = useRef<HTMLDivElement>(null)
  const liveRequestRatio = useRef(requestRatio)

  if (!dragging) liveRequestRatio.current = requestRatio

  const onDragPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setDragging(true)
    e.currentTarget.setPointerCapture(e.pointerId)
  }, [])

  const onDragPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!dragging || !containerRef.current) return
      const rect = containerRef.current.getBoundingClientRect()
      const ratio = Math.max(
        MIN_REQUEST_RATIO,
        Math.min(MAX_REQUEST_RATIO, (e.clientX - rect.left) / rect.width)
      )
      liveRequestRatio.current = ratio
      if (requestPanelRef.current) requestPanelRef.current.style.width = `${ratio * 100}%`
      if (responsePanelRef.current) responsePanelRef.current.style.width = `${(1 - ratio) * 100}%`
    },
    [dragging]
  )

  const onDragPointerUp = useCallback(() => {
    setRequestRatio(liveRequestRatio.current)
    setDragging(false)
  }, [])

  const applyRatio = useCallback((ratio: number) => {
    liveRequestRatio.current = ratio
    setRequestRatio(ratio)
    if (requestPanelRef.current)
      requestPanelRef.current.style.width = `${ratio * 100}%`
    if (responsePanelRef.current)
      responsePanelRef.current.style.width = `${(1 - ratio) * 100}%`
  }, [])

  const handleRequestTitleClick = useCallback(() => {
    applyRatio(liveRequestRatio.current > 0.9 ? 0.5 : 1.0)
  }, [applyRatio])

  const handleResponseTitleClick = useCallback(() => {
    applyRatio(liveRequestRatio.current < 0.1 ? 0.5 : 0.0)
  }, [applyRatio])

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-surface-base">
      {entry && <SummaryBar entry={entry} onClose={onClose} />}

      <div
        ref={containerRef}
        className={`flex min-h-0 flex-1 overflow-hidden ${dragging ? 'select-none' : ''}`}
        style={{ cursor: dragging ? 'col-resize' : '' }}>
        <div
          ref={requestPanelRef}
          className="flex flex-col min-h-0 min-w-0 shrink-0 overflow-hidden"
          style={{ width: `${liveRequestRatio.current * 100}%` }}>
          <SidePanel
            title={t('detail.request')}
            tab={requestTab}
            onTabChange={setRequestTab}
            tabs={reqTabs}
            onTitleClick={handleRequestTitleClick}>
            <PanelContent tab={requestTab} side="request" entry={entry} />
          </SidePanel>
        </div>

        {/* Draggable Resize Handle */}
        <div
          onPointerDown={onDragPointerDown}
          onPointerMove={onDragPointerMove}
          onPointerUp={onDragPointerUp}
          className="group relative w-[1px] shrink-0 cursor-col-resize bg-surface-elevated transition-colors hover:bg-primary/50 active:bg-primary/70">
          <div className="absolute inset-y-0 -left-2 -right-2" />
          <div className="absolute inset-y-0 left-1/2 -translate-x-1/2 flex flex-col items-center justify-center gap-[3px] opacity-0 group-hover:opacity-100 transition-opacity">
            <span className="block size-[3px] rounded-full bg-muted-foreground" />
            <span className="block size-[3px] rounded-full bg-muted-foreground" />
            <span className="block size-[3px] rounded-full bg-muted-foreground" />
          </div>
        </div>

        <div
          ref={responsePanelRef}
          className="flex flex-col min-h-0 min-w-0 shrink-0 overflow-hidden"
          style={{ width: `${(1 - liveRequestRatio.current) * 100}%` }}>
          <SidePanel
            title={t('detail.response')}
            tab={responseTab}
            onTabChange={setResponseTab}
            tabs={respTabs}
            onTitleClick={handleResponseTitleClick}>
            <PanelContent tab={responseTab} side="response" entry={entry} onCloseStream={() => setResponseTab("header")} />
          </SidePanel>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// SummaryBar
// ---------------------------------------------------------------------------
function SummaryBar({ entry, onClose }: { entry: TrafficEntry; onClose?: () => void }) {
  const { t } = useTranslation()
  const { copied, copy } = useCopyToClipboard()

  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-surface-elevated px-3 py-1.5 text-xs">
      <span
        className="badge-method shrink-0"
        style={{
          color: `var(--badge-${entry.method.toLowerCase()})`,
          background: `color-mix(in oklch, var(--badge-${entry.method.toLowerCase()}) 10%, transparent)`,
          borderColor: `color-mix(in oklch, var(--badge-${entry.method.toLowerCase()}) 20%, transparent)`,
        }}>
        {entry.method}
      </span>
      <span
        className="badge-status shrink-0" data-dot={entry.status != null}
        style={{
          color: `var(--badge-${statusCategory(entry.status ?? 0)})`,
        }}>
        {entry.status ?? t('detail.pending')}
      </span>
      <span className="min-w-0 flex-1 truncate text-primary" title={entry.uri}>
        {entry.uri}
      </span>
      {onClose && (
        <button
          onClick={onClose}
          className="shrink-0 rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-colors"
          title="关闭详情">
          <XIcon className="size-3" />
        </button>
      )}
      <button
        onClick={() => copy(entry.uri)}
        className="shrink-0 rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-colors"
        title={copied ? t('detail.copied') : t('detail.copyUri')}>
        {copied ? <CheckIcon className="size-3 text-primary" /> : <CopyIcon className="size-3" />}
      </button>
      {entry.durationMs != null && (
        <span className="shrink-0 text-muted-foreground tabular-nums">
          {formatDuration(entry.durationMs)}
        </span>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// PanelContent — 根据 tab 分发
// ---------------------------------------------------------------------------
function PanelContent({
  tab,
  side,
  entry,
  onCloseStream,
}: {
  tab: PanelTab
  side: 'request' | 'response'
  entry: TrafficEntry | undefined
  onCloseStream?: () => void
}) {
  const { t } = useTranslation()

  // 空状态：未选中任何条目时，各 tab 均显示空白
  if (!entry) {
    if (tab === 'header' || tab === 'query') {
      return <KeyValueTable data={{}} emptyLabel="" />
    }
    return <EmptyContent label="" />
  }

  if (tab === 'header') {
    if (side === 'request') {
      return <KeyValueTable data={entry.requestHeaders} emptyLabel={t('detail.noHeaders')} />
    }
    return entry.responseHeaders ? (
      <KeyValueTable data={entry.responseHeaders} emptyLabel={t('detail.noHeaders')} />
    ) : (
      <EmptyContent label={t('detail.responsePending')} />
    )
  }

  if (tab === 'query' && side === 'request') {
    const queryData = entry.requestQuery ?? {}
    return <KeyValueTable data={queryData} emptyLabel={t('detail.noQuery')} />
  }

  if (tab === 'form' && side === 'request') {
    const formCt = entry.requestHeaders['content-type'] ?? entry.requestHeaders['Content-Type'] ?? ''
    return <FormDataView body={entry.requestBody ?? ''} contentType={formCt} />
  }

  if (tab === 'body') {
    const body = side === 'request' ? entry.requestBody : entry.responseBody
    return body ? (
      <BodyView body={body} />
    ) : (
      <EmptyContent label={side === 'request' ? t('detail.noRequestBody') : t('detail.noBody')} />
    )
  }

  if (tab === 'stream') {
    return <StreamingViewer entry={entry} onClose={onCloseStream} />
  }

  const content = side === 'request' ? formatRequestRaw(entry) : formatResponseRaw(entry)
  return <RawView content={content} />
}

// ---------------------------------------------------------------------------
// SidePanel — 带 tab 切换动画
// ---------------------------------------------------------------------------
function SidePanel({
  title,
  tab,
  onTabChange,
  tabs,
  children,
  onTitleClick,
}: {
  title: string
  tab: PanelTab
  onTabChange: (tab: PanelTab) => void
  tabs: { id: PanelTab; labelKey: string }[]
  children: ReactNode
  onTitleClick?: () => void
}) {
  const { t } = useTranslation()
  const [animating, setAnimating] = useState(false)
  const [animationKey, setAnimationKey] = useState(0)
  const slideDirectionRef = useRef(1)

  const handleTabChange = useCallback(
    (id: PanelTab) => {
      // 用 ref 避免闭包捕获旧的 tab
      const oldIdx = tabs.findIndex(x => x.id === tab)
      const newIdx = tabs.findIndex(x => x.id === id)
      slideDirectionRef.current = newIdx > oldIdx ? 1 : -1
      setAnimating(true)
      // 分两阶段：先出动画 → 切换内容 → 入动画
      setTimeout(() => {
        onTabChange(id)
        setAnimationKey(k => k + 1)
      }, 120)
      setTimeout(() => setAnimating(false), 240)
    },
    [tab, tabs, onTabChange]
  )

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center gap-0 border-b border-surface-elevated">
        <span
          onClick={onTitleClick}
          className={`px-3 py-1.5 text-xs font-medium text-foreground ${onTitleClick ? 'cursor-pointer hover:bg-surface-elevated/50 rounded transition-colors' : ''}`}
          title={onTitleClick ? 'Click to toggle full width' : undefined}>
          {title}
        </span>
        <div className="flex">
          {tabs.map(x => (
            <button
              key={x.id}
              className={`relative px-2.5 py-1.5 text-[11px] transition-colors ${
                tab === x.id
                  ? 'font-medium text-foreground'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
              onClick={() => handleTabChange(x.id)}>
              {t(x.labelKey)}
              {tab === x.id && (
                <span className="absolute bottom-0 left-2 right-2 h-0.5 bg-primary rounded-full" />
              )}
            </button>
          ))}
        </div>
      </div>
      <div
        key={animationKey}
        className={`min-h-0 flex-1 overflow-y-auto transition-all duration-200 ease-out ${
          animating
            ? slideDirectionRef.current > 0
              ? 'translate-x-2 opacity-0'
              : '-translate-x-2 opacity-0'
            : 'translate-x-0 opacity-100'
        }`}>
        {children}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// KeyValueTable — headers 表格
// ---------------------------------------------------------------------------

const MIN_KEY_RATIO = 0.15
const MAX_KEY_RATIO = 0.7

function KeyValueTable({ data, emptyLabel }: { data: Record<string, string>; emptyLabel: string }) {
  const entries = Object.entries(data)
  const [keyRatio, setKeyRatio] = useState(0.35)
    const [dragging, setDragging] = useState(false)

  const containerRef = useRef<HTMLDivElement>(null)
  const handleRef = useRef<HTMLDivElement>(null)
  const liveKeyRatio = useRef(keyRatio)

  if (!dragging) liveKeyRatio.current = keyRatio

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setDragging(true)
    e.currentTarget.setPointerCapture(e.pointerId)
  }, [])

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragging || !containerRef.current) return
    const rect = containerRef.current.getBoundingClientRect()
    const ratio = Math.max(
      MIN_KEY_RATIO,
      Math.min(MAX_KEY_RATIO, (e.clientX - rect.left) / rect.width),
    )
    liveKeyRatio.current = ratio
    const pct = `${ratio * 100}%`
    const cols = containerRef.current.querySelectorAll('col')
    if (cols[0]) (cols[0] as HTMLTableColElement).style.width = pct
    if (cols[1]) (cols[1] as HTMLTableColElement).style.width = `${(1 - ratio) * 100}%`
    if (handleRef.current) handleRef.current.style.left = pct
  }, [dragging])

  const onPointerUp = useCallback(() => {
    setKeyRatio(liveKeyRatio.current)
    setDragging(false)
  }, [])

  const keyPct = `${liveKeyRatio.current * 100}%`
  const valPct = `${(1 - liveKeyRatio.current) * 100}%`

  return (
    <div className="relative" ref={containerRef}>
      <table className={`w-full table-fixed text-xs ${dragging ? 'select-none' : ''}`} style={{ cursor: dragging ? 'col-resize' : '' }}>
        <colgroup>
          <col style={{ width: keyPct }} />
          <col style={{ width: valPct }} />
        </colgroup>
        <thead>
          <tr className="border-b border-surface-elevated bg-surface-elevated/30 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
            <th className="py-1.5 pl-3 pr-2 text-left font-semibold overflow-hidden">Key</th>
            <th className="py-1.5 pr-3 text-left font-semibold">Value</th>
          </tr>
        </thead>
        <tbody>
          {entries.length === 0 ? (
            <tr>
              <td colSpan={2} className="py-4 text-center text-muted-foreground">
                {emptyLabel}
              </td>
            </tr>
          ) : (
            entries.map(([key, value]) => <KeyValueRow key={key} entryKey={key} value={value} />)
          )}
        </tbody>
      </table>

      {/* Full-height drag handle overlay */}
      <div
        ref={handleRef}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        className="group/handle absolute top-0 bottom-0 z-10 cursor-col-resize"
        style={{ left: keyPct, width: 5, transform: 'translateX(-2.5px)' }}
      >
        <div className="absolute inset-y-0 left-1/2 -translate-x-1/2 w-[1px] bg-surface-elevated group-hover/handle:bg-primary/50 transition-colors" />
      </div>
    </div>
  )
}

function KeyValueRow({ entryKey, value }: { entryKey: string; value: string }) {
  const { copied: valueCopied, copy: copyValue } = useCopyToClipboard(1200)
  const { copied: rowCopied, copy: copyRow } = useCopyToClipboard(1200)
  const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const handleClick = useCallback(() => {
    if (clickTimer.current) {
      // 已经有定时器在跑 → 这是双击
      clearTimeout(clickTimer.current)
      clickTimer.current = null
      copyRow(`${entryKey}: ${value}`)
    } else {
      // 启动定时器，等待双击判定
      clickTimer.current = setTimeout(() => {
        clickTimer.current = null
        copyValue(value)
      }, 250)
    }
  }, [entryKey, value, copyValue, copyRow])

  return (
    <tr className="border-b border-surface-elevated/30 group hover:bg-surface-elevated/20 transition-colors">
      <td className="py-1.5 pl-3 pr-2 align-top font-mono text-xs text-muted-foreground whitespace-nowrap overflow-hidden">
        {entryKey}
      </td>
      <td className="py-1.5 pr-2 align-top text-foreground/80 text-sm break-all relative">
        <span
          onClick={handleClick}
          className="cursor-pointer"
          title={
            valueCopied
              ? 'Copied value'
              : rowCopied
                ? 'Copied key: value'
                : 'Click to copy value, double-click to copy key: value'
          }>
          {value}
        </span>
        <button
          onClick={handleClick}
          className="absolute right-0 top-0 p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-all opacity-0 group-hover:opacity-100"
          title={
            valueCopied
              ? 'Copied value'
              : rowCopied
                ? 'Copied key: value'
                : 'Click: copy value, DblClick: copy key: value'
          }>
          {rowCopied ? (
            <CheckIcon className="size-3 text-primary" />
          ) : valueCopied ? (
            <CheckIcon className="size-3 text-primary" />
          ) : (
            <CopyIcon className="size-3" />
          )}
        </button>
      </td>
    </tr>
  )
}

// ---------------------------------------------------------------------------
// EmptyContent
// ---------------------------------------------------------------------------
function EmptyContent({ label }: { label: string }) {
  return (
    <div className="flex items-center justify-center py-10 text-sm text-muted-foreground">
      {label}
    </div>
  )
}

// ---------------------------------------------------------------------------
// RawView
// ---------------------------------------------------------------------------
function RawView({ content }: { content: string }) {
  const { copied, copy } = useCopyToClipboard()

  return (
    <div className="flex flex-col h-full">
      <div className="relative min-h-0 flex-1 group/mini">
        <div className="absolute top-1.5 right-1.5 z-10 flex items-center gap-0.5 opacity-0 group-hover/mini:opacity-100 transition-all">
          <button
            onClick={() => copy(content)}
            className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/30 transition-colors"
            title={copied ? 'Copied' : 'Copy'}>
            {copied ? <CheckIcon className="size-3 text-primary" /> : <CopyIcon className="size-3" />}
          </button>
        </div>
        <div className="absolute inset-0 overflow-auto">
          {content ? (
            <pre className="whitespace-pre-wrap break-all px-3 py-2 text-xs text-foreground/80 font-mono">
              {content}
            </pre>
          ) : null}
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// format helpers
// ---------------------------------------------------------------------------
function formatRequestRaw(entry: TrafficEntry): string {
  const lines = [`${entry.method} ${entry.uri} HTTP/1.1`]
  for (const [key, value] of Object.entries(entry.requestHeaders)) {
    lines.push(`${key}: ${value}`)
  }
  if (entry.requestBody) {
    lines.push('', entry.requestBody)
  }
  return lines.join('\n')
}

function formatResponseRaw(entry: TrafficEntry): string {
  if (!entry.responseHeaders) return ''
  const lines = [`HTTP/1.1 ${entry.status ?? '...'}`]
  for (const [key, value] of Object.entries(entry.responseHeaders)) {
    lines.push(`${key}: ${value}`)
  }
  if (entry.responseBody) {
    lines.push('', entry.responseBody)
  }
  return lines.join('\n')
}

// ---------------------------------------------------------------------------
// JsonTreeView — 可折叠 JSON 树
// ---------------------------------------------------------------------------

/**
 * 格式化 JSON 原始值用于展示
 */
function formatPrimitive(val: JsonValue): { text: string; className: string } {
  if (val === null) return { text: 'null', className: 'text-purple-500' }
  if (typeof val === 'boolean') return { text: String(val), className: 'text-orange-500' }
  if (typeof val === 'number') return { text: String(val), className: 'text-emerald-500' }
  return { text: JSON.stringify(val), className: 'text-green-600 dark:text-green-400' }
}

/**
 * 预览 collapsed 节点的内容摘要 — 只显示第一个值与类型提示
 */
function previewValue(val: JsonValue): string {
  if (Array.isArray(val)) {
    if (val.length === 0) return '[]'
    const first = JSON.stringify(val[0])
    return `[${first}, …]`
  }
  if (val !== null && typeof val === 'object') {
    const keys = Object.keys(val)
    if (keys.length === 0) return '{}'
    const firstKey = keys[0]
    const firstVal = JSON.stringify((val as Record<string, JsonValue>)[firstKey])
    return `{ ${firstKey}: ${firstVal}, … }`
  }
  return JSON.stringify(val)
}

/**
 * 单个 JSON 树节点
 */
function TreeNode({
  label,
  value,
  defaultExpanded,
  depth,
}: {
  label?: string
  value: JsonValue
  defaultExpanded: boolean
  depth: number
}) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const isCollapsible = value !== null && typeof value === 'object'

  // 同步 defaultExpanded 变化（当用户点击"展开全部/折叠全部"时）
  useEffect(() => {
    if (isCollapsible) {
      setExpanded(defaultExpanded)
    }
  }, [defaultExpanded, isCollapsible])

  const indent = depth * 12

  if (!isCollapsible) {
    const { text, className } = formatPrimitive(value)
    const hasLabel = label !== undefined
    return (
      <div
        className="py-px hover:bg-surface-elevated/20 transition-colors"
        style={{ paddingLeft: indent + 12 }}>
        {hasLabel && (
          <span className="text-foreground/80">
            {JSON.stringify(label)}
            <span className="text-muted-foreground/60 mx-1">: </span>
          </span>
        )}
        <span className={`${className}`}>{text}</span>
      </div>
    )
  }

  const isArray = Array.isArray(value)
  const entries = isArray
    ? value.map((v, i) => [String(i), v] as const)
    : Object.entries(value as Record<string, JsonValue>)
  const count = isArray ? value.length : entries.length
  const openBracket = isArray ? '[' : '{'
  const closeBracket = isArray ? ']' : '}'

  return (
    <div>
      {/* 折叠状态显示在一行 */}
      <button
        onClick={() => setExpanded(e => !e)}
        className="flex items-center gap-0 w-full text-left py-px hover:bg-surface-elevated/20 transition-colors"
        style={{ paddingLeft: Math.max(indent, 0) }}>
        {expanded ? (
          <ChevronDown className="size-3 shrink-0 text-muted-foreground/60" />
        ) : (
          <ChevronRight className="size-3 shrink-0 text-muted-foreground/60" />
        )}
        {label !== undefined && (
          <>
            <span className="text-foreground/80 shrink-0 ml-0.5">{JSON.stringify(label)}</span>
            <span className="text-muted-foreground/60 mx-0.5 shrink-0">: </span>
          </>
        )}
        {expanded ? (
          <span className="text-muted-foreground/70">{openBracket}</span>
        ) : (
          <span className="text-muted-foreground/70 truncate min-w-0">
            {`${openBracket} ${previewValue(value)} ${closeBracket}`}
          </span>
        )}
        {expanded && (
          <span className="text-muted-foreground/40 text-[10px] ml-1 shrink-0">
            {isArray ? `${count} items` : `${count} keys`}
          </span>
        )}
      </button>
      {/* 展开后的子节点 */}
      {expanded && (
        <div>
          {entries.map(([key, val]) => (
            <TreeNode
              key={key}
              label={isArray ? undefined : key}
              value={val}
              defaultExpanded={defaultExpanded}
              depth={depth + 1}
            />
          ))}
          <div className="text-muted-foreground/70 py-px" style={{ paddingLeft: indent + 12 }}>
            {closeBracket}
          </div>
        </div>
      )}
    </div>
  )
}

/**
 * JSON 树形视图根组件
 */
function JsonTreeView({
  data,
  defaultExpanded,
  depth = 0,
  wrapped = false,
}: {
  data: JsonValue
  defaultExpanded: boolean
  depth?: number
  wrapped?: boolean
}) {
  return (
    <div
      className={`font-mono text-xs leading-5 select-none ${wrapped ? 'whitespace-pre-wrap break-all' : 'whitespace-nowrap overflow-x-auto'}`}>
      <TreeNode value={data} defaultExpanded={defaultExpanded} depth={depth} />
    </div>
  )
}

// ---------------------------------------------------------------------------
// BodyView — 内部组件
// ---------------------------------------------------------------------------

/**
 * 尝试格式化 JSON，失败返回 null
 */
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
/** 简单的 XML 格式化 */
function formatXml(input: string): string {
  // 规范化换行，在标签边界添加换行
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
    // 减少缩进：结束标签、处理指令
    if (line.match(/^<\//) || line.match(/^<\?/)) {
      indent--
    }
    result += '  '.repeat(Math.max(0, indent)) + line + '\n'
    // 增加缩进：开始标签（非自闭和、非处理指令、非结束标签、非注释、非CDATA）
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

const BodyView = memo(function BodyView({ body }: { body: string }) {
  const [wrapped, setWrapped] = useState(true)
  const [format, setFormat] = useState<'auto' | 'json' | 'xml' | 'html' | 'plaintext'>('auto')
  const [allExpanded, setAllExpanded] = useState(true)
  const { copied, copy } = useCopyToClipboard()

  // 格式化检测
  const { cleaned, formatted, parsedJson, isJson } = useMemo(() => {
    const c = body.replace(/\r\n/g, '\n').replace(/\r/g, '\n').trim()
    const f = tryFormatJson(c)
    let parsed: JsonValue | null = null
    if (f !== null) {
      try { parsed = JSON.parse(c) as JsonValue } catch { /* ignore */ }
    }
    return { cleaned: c, formatted: f, parsedJson: parsed, isJson: f !== null }
  }, [body])
  // 根据 format 决定显示内容和语言
  const { displayBody, displayLang, useTreeView } = useMemo(() => {
    if (format === 'auto') {
      if (isJson) {
        return { displayBody: formatted!.formatted, displayLang: 'json', useTreeView: true as const }
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
  }, [format, cleaned, isJson, formatted])

  return (
    <div className="flex flex-col h-full">
      <div className="relative min-h-0 flex-1 group/mini">
        {/* 操作栏 — 悬浮在右上角，不占空间，不随滚动消失 */}
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
        {/* 可滚动的内容区 — 用 absolute 填满父级 */}
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








