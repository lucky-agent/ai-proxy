import { useState, useRef, useCallback, useMemo, useEffect } from 'react'
import { VList } from 'virtua'
import { useTranslation } from 'react-i18next'
import { ArrowDownIcon, ArrowUpIcon, CopyIcon, RefreshCwIcon, PencilIcon, LockKeyholeIcon, LockOpenIcon } from 'lucide-react'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { GripDots } from '@/components/icons'
import { Empty, EmptyTitle } from '@/components/ui/empty'
import { Badge } from '@/components/ui/badge'
import type { TrafficEntry } from '@/types/proxy'
import { statusCategory, formatDuration, formatTime, formatCurl } from '@/lib/format'
import { useCopyToClipboard } from '@/hooks/useCopyToClipboard'
import { cn } from '@/lib/utils'

export type ListEntry = TrafficEntry
export type SortOrder = 'desc' | 'asc'
export type SortColumn = ColKey | null
type ColKey = 'id' | 'url' | 'method' | 'status' | 'duration' | 'time' | 'ssl' | 'edited'
const COLS: ColKey[] = ['id', 'url', 'method', 'status', 'duration', 'time', 'ssl', 'edited']
const DEFAULT_WIDTHS: Record<ColKey, number> = {
  id: 9,
  url: 29,
  method: 9,
  status: 9,
  duration: 9,
  time: 15,
  ssl: 8,
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
    <GripDots className="text-current pointer-events-none" />
  </span>
)

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
  const { copy } = useCopyToClipboard()
  const {
    gridRef,
    columnWidths,
    draggingCol,
    isDragging,
    onPointerDown,
    onPointerMove,
    onPointerUp,
  } = useColumnResize()

  const [listKey, setListKey] = useState(0)

  useEffect(() => {
    setListKey(k => k + 1)
  }, [sortColumn, sortOrder])

  const handleSortClick = useCallback(
    (col: ColKey) => {
      if (sortColumn === col) return onSortChange(col, sortOrder === 'desc' ? 'asc' : 'desc')
      onSortChange(col, 'desc')
    },
    [sortColumn, sortOrder, onSortChange]
  )

  const gridTemplate = useMemo(() => COLS.map(c => `${columnWidths[c]}%`).join(' '), [columnWidths])
  const totalColPct = useMemo(
    () => Object.values(columnWidths).reduce((s, w) => s + w, 0),
    [columnWidths]
  )
  const rowStyle = useMemo(
    () =>
      ({
        gridTemplateColumns: 'var(--grid-cols)',
        minWidth: `${totalColPct}%`,
      }) as React.CSSProperties,
    [totalColPct]
  )

  const renderHeaderCell = (col: ColKey, labelKey: string, extraClasses: string = '') => (
    <div className={`group px-2 py-1.5 text-left relative min-w-0 ${extraClasses}`}>
      <Grip
        className={
          draggingCol === col
            ? 'text-primary'
            : 'text-muted-foreground/25 hover:text-muted-foreground/70'
        }
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
            sortColumn === col
              ? 'opacity-100 text-foreground'
              : 'opacity-0 group-hover:opacity-100 text-muted-foreground'
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
      className={`h-full bg-surface-deep flex flex-col overflow-auto ${isDragging ? 'select-none' : ''}`}
      style={
        {
          cursor: isDragging ? 'col-resize' : '',
          '--grid-cols': gridTemplate,
        } as React.CSSProperties
      }>
      <div
        className="group/grid grid shrink-0 z-10 bg-surface-base/50 text-[11px] font-bold text-muted-foreground uppercase tracking-wide border-b border-surface-elevated overflow-hidden h-7"
        style={rowStyle}>
        {renderHeaderCell('id', 'requestList.id', 'font-normal normal-case')}
        {renderHeaderCell('url', 'requestList.url')}
        {renderHeaderCell('method', 'requestList.method')}
        {renderHeaderCell('status', 'requestList.status')}
        {renderHeaderCell('duration', 'requestList.duration')}
        {renderHeaderCell('time', 'requestList.time')}
        {renderHeaderCell('ssl', 'requestList.ssl')}
        {renderHeaderCell('edited', 'requestList.edited')}
      </div>

      {entries.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <Empty>
            <EmptyTitle>{t('requestList.emptyTitle')}</EmptyTitle>
          </Empty>
        </div>
      ) : (
        <VList key={listKey} className="flex-1 min-h-0">
          {entries.map((entry, i) => (
            <ContextMenu key={entry.id || i}>
              <ContextMenuTrigger>
                <div
                  className={cn(
                    'grid text-xs border-b border-surface-elevated/50 cursor-pointer transition-colors hover:bg-surface-elevated/50 border-l-2',
                    entry.id === selectedId
                      ? 'bg-primary/10 border-primary'
                      : 'border-transparent'
                  )}
                  style={rowStyle}
                  onClick={() => onSelectEntry(entry.id)}>
                  <div className="px-1 py-2 min-w-0 tabular-nums text-[10px] text-muted-foreground text-left">
                    {entry.requestNumber}
                  </div>
                  <div
                    className="px-1 py-2 text-foreground/80 font-mono min-w-0 overflow-hidden"
                    title={entry.uri}>
                    <span className="block truncate">{entry.uri}</span>
                  </div>
                  <div className="px-1 py-2 min-w-0 overflow-hidden whitespace-nowrap">
                    <Badge
                      className="rounded font-semibold uppercase"
                      style={{
                        color: `var(--badge-${entry.method.toLowerCase()})`,
                        background: `color-mix(in oklch, var(--badge-${entry.method.toLowerCase()}) 10%, transparent)`,
                        borderColor: `color-mix(in oklch, var(--badge-${entry.method.toLowerCase()}) 20%, transparent)`,
                      }}>
                      {entry.method}
                    </Badge>
                  </div>
                  <div className="px-1 py-2 min-w-0 overflow-hidden whitespace-nowrap">
                    <Badge
                      className="rounded font-semibold"
                      style={{ color: `var(--badge-${statusCategory(entry.status)})` }}>
                      {entry.status != null && (
                        <span
                          className="inline-block size-1.5 rounded-full shrink-0 bg-current"
                        />
                      )}
                      {entry.status ?? t('requestList.pending')}
                    </Badge>
                  </div>
                  <div className="px-1 py-2 min-w-0 overflow-hidden whitespace-nowrap text-muted-foreground">
                    {formatDuration(entry.durationMs)}
                  </div>
                  <div className="px-1 py-2 min-w-0 overflow-hidden whitespace-nowrap text-muted-foreground/60 text-left">
                    {formatTime(entry.requestTimestamp)}
                  </div>
                  <div className="px-1 py-2 min-w-0 overflow-hidden flex items-center justify-center">
                    {entry.decrypted === false ? (
                      <LockKeyholeIcon className="size-3.5 text-muted-foreground/70 shrink-0" />
                    ) : entry.decrypted === true ? (
                      <LockOpenIcon className="size-3.5 text-muted-foreground/70 shrink-0" />
                    ) : null}
                  </div>
                  <div className="px-1 py-2 min-w-0 overflow-hidden text-left">
                    {entry.edited ? (
                      <span className="text-[10px] text-amber-400 font-medium">
                        {t('requestList.edited')}
                      </span>
                    ) : null}
                  </div>
                </div>
              </ContextMenuTrigger>
              <ContextMenuContent className="text-xs min-w-36">
                <ContextMenuItem onClick={() => onEditRequest(entry)}>
                  <PencilIcon className="size-3.5" />
                  <span>{t('requestList.edit')}</span>
                </ContextMenuItem>
                <ContextMenuItem onClick={() => onResendRequest(entry)}>
                  <RefreshCwIcon className="size-3.5" />
                  <span>{t('requestList.repeat')}</span>
                </ContextMenuItem>
                <ContextMenuItem onClick={() => { copy(formatCurl(entry)) }}>
                  <CopyIcon className="size-3.5" />
                  <span>{t('requestList.copyCurl')}</span>
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          ))}
        </VList>
      )}

    </div>
  )
}

function SortIcon({
  column: _c,
  active,
  order,
}: {
  column: ColKey
  active: boolean
  order: SortOrder
}) {
  if (!active) return <ArrowDownIcon className="inline size-3 shrink-0" />
  return order === 'asc' ? (
    <ArrowUpIcon className="inline size-3 shrink-0" />
  ) : (
    <ArrowDownIcon className="inline size-3 shrink-0" />
  )
}
