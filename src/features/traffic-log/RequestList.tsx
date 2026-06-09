import { useState, useRef, useCallback, useMemo, useEffect } from 'react'
import { createPortal } from 'react-dom'
import { VList } from 'virtua'
import { useTranslation } from 'react-i18next'
import {
  ArrowDownIcon,
  ArrowUpIcon,
  CopyIcon,
  CheckIcon,
  RefreshCwIcon,
  PencilIcon,
} from 'lucide-react'
import type { TrafficEntry } from '@/types/proxy'
import { statusCategory, formatDuration, formatTime, shortenUri, formatCurl } from '@/lib/format'
import { useCopyToClipboard } from '@/hooks/useCopyToClipboard'
import { cn } from '@/lib/utils'

export type ListEntry = TrafficEntry
export type SortOrder = 'desc' | 'asc'
export type SortColumn = ColKey | null
type ColKey = 'id' | 'url' | 'method' | 'status' | 'duration' | 'time' | 'edited'
const COLS: ColKey[] = ['id', 'url', 'method', 'status', 'duration', 'time', 'edited']
const DEFAULT_WIDTHS: Record<ColKey, number> = {
  id: 9,
  url: 29,
  method: 9,
  status: 9,
  duration: 9,
  time: 22,
  edited: 13,
}
const MIN_PCT = 5
const MAX_PCT = 40

function useColumnResize() {
  const [columnWidths, setColumnWidths] = useState<Record<ColKey, number>>(DEFAULT_WIDTHS)
  const [draggingCol, setDraggingCol] = useState<ColKey | null>(null)
  const gridRef = useRef<HTMLDivElement | null>(null)
  const liveWidths = useRef(DEFAULT_WIDTHS)
  const dragStart = useRef<{ col: ColKey; startX: number; startPct: number } | null>(null)
  if (!draggingCol) liveWidths.current = columnWidths

  const onPointerDown = useCallback((colKey: ColKey, e: React.PointerEvent) => {
    e.preventDefault()
    e.currentTarget.setPointerCapture(e.pointerId)
    setDraggingCol(colKey)
    dragStart.current = { col: colKey, startX: e.clientX, startPct: liveWidths.current[colKey] }
  }, [])

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragStart.current || !gridRef.current) return
    const { col, startX, startPct } = dragStart.current
    const gridWidth = gridRef.current.offsetWidth
    if (!gridWidth) return
    const deltaPct = ((e.clientX - startX) / gridWidth) * 100
    const newPct = Math.round(Math.max(MIN_PCT, Math.min(MAX_PCT, startPct + deltaPct)))
    liveWidths.current = { ...liveWidths.current, [col]: newPct }
    const template = COLS.map(c => `${liveWidths.current[c]}%`).join(' ')
    gridRef.current.style.setProperty('--grid-cols', template)
  }, [])

  const onPointerUp = useCallback(() => {
    if (!dragStart.current) return
    setColumnWidths({ ...liveWidths.current })
    setDraggingCol(null)
    dragStart.current = null
  }, [])

  const isDragging = draggingCol !== null
  return {
    gridRef,
    columnWidths,
    draggingCol,
    isDragging,
    onPointerDown,
    onPointerMove,
    onPointerUp,
  }
}

const Grip = ({
  className,
  onPointerDown,
  onPointerMove,
  onPointerUp,
}: {
  className?: string
  onPointerDown: (e: React.PointerEvent) => void
  onPointerMove: (e: React.PointerEvent) => void
  onPointerUp: (e: React.PointerEvent) => void
}) => (
  <span
    className={cn(
      'absolute -right-1.5 top-0 h-full w-3 flex items-center justify-center cursor-col-resize z-20 opacity-0 group-hover/grid:opacity-100 transition-opacity',
      className
    )}
    onPointerDown={onPointerDown}
    onPointerMove={onPointerMove}
    onPointerUp={onPointerUp}>
    <svg
      viewBox="0 0 4 12"
      width={4}
      height={12}
      className="text-current pointer-events-none"
      aria-hidden>
      <circle cx="2" cy="2" r="1.15" fill="currentColor" />
      <circle cx="2" cy="6" r="1.15" fill="currentColor" />
      <circle cx="2" cy="10" r="1.15" fill="currentColor" />
    </svg>
  </span>
)

function badgeStyle(varName: string) {
  return {
    color: `var(${varName})`,
    background: `color-mix(in oklch, var(${varName}) 12%, transparent)`,
  }
}

interface ContextMenuState {
  x: number
  y: number
  entry: TrafficEntry
}

function ContextMenu({
  state,
  onClose,
  onEdit,
  onResend,
}: {
  state: ContextMenuState
  onClose: () => void
  onEdit: (entry: TrafficEntry) => void
  onResend: (entry: TrafficEntry) => void
}) {
  const { t } = useTranslation()
  const { copied, copy } = useCopyToClipboard()
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose()
      }
    }
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClick)
      document.addEventListener('keydown', handleKey)
    }, 0)
    return () => {
      clearTimeout(timer)
      document.removeEventListener('mousedown', handleClick)
      document.removeEventListener('keydown', handleKey)
    }
  }, [onClose])

  const handleCopyCurl = useCallback(() => {
    copy(formatCurl(state.entry))
    onClose()
  }, [state.entry, copy, onClose])

  const handleEdit = useCallback(() => {
    onEdit(state.entry)
    onClose()
  }, [state.entry, onEdit, onClose])

  const handleResend = useCallback(() => {
    onResend(state.entry)
    onClose()
  }, [state.entry, onResend, onClose])

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-50 min-w-40 rounded-lg bg-popover py-1 text-popover-foreground shadow-lg ring-1 ring-foreground/10"
      style={{ left: state.x, top: state.y }}>
      <button onClick={handleEdit} className="flex w-full items-center gap-2 px-3 py-1.5 text-xs text-foreground/80 hover:bg-accent hover:text-accent-foreground transition-colors">
        <PencilIcon className="size-3.5" />
        <span>{t('requestList.edit')}</span>
      </button>
      <button onClick={handleResend} className="flex w-full items-center gap-2 px-3 py-1.5 text-xs text-foreground/80 hover:bg-accent hover:text-accent-foreground transition-colors">
        <RefreshCwIcon className="size-3.5" />
        <span>{t('requestList.repeat')}</span>
      </button>
      <button onClick={handleCopyCurl} className="flex w-full items-center gap-2 px-3 py-1.5 text-xs text-foreground/80 hover:bg-accent hover:text-accent-foreground transition-colors">
        <CopyIcon className="size-3.5" />
        <span>{t('requestList.copyCurl')}</span>
      </button>
    </div>,
    document.body
  )
}

interface Props {
  entries: ListEntry[]
  selectedId: string | null
  onSelectEntry: (id: string) => void
  sortColumn: SortColumn
  sortOrder: SortOrder
  onSortChange: (column: SortColumn, order: SortOrder) => void
  onResendRequest: (entry: TrafficEntry) => void
  onEditRequest: (entry: TrafficEntry) => void
}

export default function RequestList({
  entries,
  selectedId,
  onSelectEntry,
  sortColumn,
  sortOrder,
  onSortChange,
  onResendRequest,
  onEditRequest,
}: Props) {
  const { t } = useTranslation()
  const {
    gridRef,
    columnWidths,
    draggingCol,
    isDragging,
    onPointerDown,
    onPointerMove,
    onPointerUp,
  } = useColumnResize()

  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null)
  const [listKey, setListKey] = useState(0)

  useEffect(() => { setListKey(k => k + 1) }, [sortColumn, sortOrder])

  const handleSortClick = useCallback((col: ColKey) => {
    if (sortColumn === col) return onSortChange(col, sortOrder === 'desc' ? 'asc' : 'desc')
    onSortChange(col, 'desc')
  }, [sortColumn, sortOrder, onSortChange])

  const gridTemplate = useMemo(() => COLS.map(c => `${columnWidths[c]}%`).join(' '), [columnWidths])
  const totalColPct = useMemo(() => Object.values(columnWidths).reduce((s, w) => s + w, 0), [columnWidths])
  const rowStyle = useMemo(() => ({ gridTemplateColumns: 'var(--grid-cols)', minWidth: `${totalColPct}%` } as React.CSSProperties), [totalColPct])

  const handleContextMenu = useCallback((e: React.MouseEvent, entry: TrafficEntry) => {
    e.preventDefault()
    setCtxMenu({ x: e.clientX, y: e.clientY, entry })
  }, [])

  const renderHeaderCell = (col: ColKey, labelKey: string, extraClasses: string = '') => (
    <div className={`group px-2 py-1.5 text-left relative min-w-0 ${extraClasses}`}>
      <Grip
        className={draggingCol === col ? 'text-primary' : 'text-muted-foreground/25 hover:text-muted-foreground/70'}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerDown={e => onPointerDown(col, e)}
      />
      <span className="inline-flex items-center gap-0.5 max-w-full">
        <span className="truncate min-w-0">{t(labelKey)}</span>
        <button
          type="button"
          className={cn(
            'inline-flex items-center p-0 leading-none cursor-pointer shrink-0 transition-opacity',
            sortColumn === col ? 'opacity-100 text-foreground' : 'opacity-0 group-hover:opacity-100 text-muted-foreground'
          )}
          onClick={() => handleSortClick(col)}>
          <SortIcon column={col} active={sortColumn === col} order={sortOrder} />
        </button>
      </span>
    </div>
  )

  return (
    <div
      ref={gridRef}
      className={`h-full bg-background flex flex-col overflow-auto ${isDragging ? 'select-none' : ''}`}
      style={{ cursor: isDragging ? 'col-resize' : '', '--grid-cols': gridTemplate } as React.CSSProperties}>
      <div
        className="group/grid grid shrink-0 z-10 bg-muted/30 text-[11px] font-bold text-muted-foreground uppercase tracking-wide border-b border-border overflow-hidden h-7"
        style={rowStyle}>
        {renderHeaderCell('id', 'requestList.id', 'font-normal normal-case')}
        {renderHeaderCell('url', 'requestList.url')}
        {renderHeaderCell('method', 'requestList.method')}
        {renderHeaderCell('status', 'requestList.status')}
        {renderHeaderCell('duration', 'requestList.duration')}
        {renderHeaderCell('time', 'requestList.time')}
        {renderHeaderCell('edited', 'requestList.edited')}
      </div>

      {entries.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground text-sm">
          <p>{t('requestList.emptyTitle')}</p>
          <p className="mt-1 text-xs">{t('requestList.emptyHint')}</p>
        </div>
      ) : (
        <VList key={listKey} style={{ flex: 1, minHeight: 0 }}>
          {entries.map(entry => (
            <div
              key={entry.id}
              className={cn(
                'grid text-xs border-b border-border/50 cursor-pointer transition-colors',
                entry.id === selectedId && 'bg-accent'
              )}
              style={rowStyle}
              onClick={() => onSelectEntry(entry.id)}
              onContextMenu={e => handleContextMenu(e, entry)}>
              <div className="px-1 py-2 min-w-0 tabular-nums text-[10px] text-muted-foreground text-left">
                {entry.requestNumber}
              </div>
              <div className="px-1 py-2 text-foreground/80 min-w-0 overflow-hidden" title={entry.uri}>
                <span className="block truncate">{shortenUri(entry.uri)}</span>
              </div>
              <div className="px-1 py-2 min-w-0 overflow-hidden whitespace-nowrap">
                <span className="badge-method" style={badgeStyle(`--badge-${entry.method.toLowerCase()}`)}>
                  {entry.method}
                </span>
              </div>
              <div className="px-1 py-2 min-w-0 overflow-hidden whitespace-nowrap">
                <span className="badge-status" style={badgeStyle(`--badge-${statusCategory(entry.status)}`)} data-dot={entry.status != null}>
                  {entry.status ?? t('requestList.pending')}
                </span>
              </div>
              <div className="px-1 py-2 min-w-0 overflow-hidden whitespace-nowrap text-muted-foreground">
                {formatDuration(entry.durationMs)}
              </div>
              <div className="px-1 py-2 min-w-0 overflow-hidden whitespace-nowrap text-muted-foreground/60 text-left">
                {formatTime(entry.requestTimestamp)}
              </div>
              <div className="px-1 py-2 min-w-0 overflow-hidden text-left">
                {entry.edited ? (
                  <span className="text-[10px] text-amber-400 font-medium">{t('requestList.edited')}</span>
                ) : null}
              </div>
            </div>
          ))}
        </VList>
      )}

      {ctxMenu && (
        <ContextMenu state={ctxMenu} onClose={() => setCtxMenu(null)} onEdit={onEditRequest} onResend={onResendRequest} />
      )}
    </div>
  )
}

function SortIcon({ column: _c, active, order }: { column: ColKey; active: boolean; order: SortOrder }) {
  if (!active) return <ArrowDownIcon className="inline size-3 shrink-0" />
  return order === 'asc' ? (
    <ArrowUpIcon className="inline size-3 shrink-0" />
  ) : (
    <ArrowDownIcon className="inline size-3 shrink-0" />
  )
}
