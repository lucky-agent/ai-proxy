import { useState, useRef, useCallback, useMemo } from 'react'
import { VList } from 'virtua'
import { useTranslation } from 'react-i18next'
import { ArrowDownIcon, ArrowUpIcon } from 'lucide-react'
import type { TrafficEntry } from '@/types/proxy'
import { statusCategory, formatDuration, formatTime, shortenUri } from '@/lib/format'
import { cn } from '@/lib/utils'
 export type ListEntry = TrafficEntry
export type SortOrder = 'desc' | 'asc'
type ColKey = 'id' | 'url' | 'method' | 'status' | 'duration' | 'time' | 'edited'
const COLS: ColKey[] = ['id', 'url', 'method', 'status', 'duration', 'time', 'edited']
const DEFAULT_WIDTHS: Record<ColKey, number> = { id: 9, url: 29, method: 9, status: 9, duration: 9, time: 22, edited: 13 }
const MIN_PCT = 4
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
  return { gridRef, columnWidths, draggingCol, isDragging, onPointerDown, onPointerMove, onPointerUp }
}
const Grip = ({ className, onPointerDown, onPointerMove, onPointerUp }: {
  className?: string
  onPointerDown?: (e: React.PointerEvent) => void
  onPointerMove?: (e: React.PointerEvent) => void
  onPointerUp?: (e: React.PointerEvent) => void
}) => (
  <span className={cn('shrink-0 cursor-col-resize p-[2px] -mr-[2px] relative z-10', className)} onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp}>
    <svg viewBox="0 0 4 12" width={4} height={12} className="text-current pointer-events-none" aria-hidden>
      <circle cx="2" cy="2" r="1.15" fill="currentColor" />
      <circle cx="2" cy="6" r="1.15" fill="currentColor" />
      <circle cx="2" cy="10" r="1.15" fill="currentColor" />
    </svg>
  </span>
)
function badgeStyle(varName: string) { return { color: `var(${varName})`, background: `color-mix(in oklch, var(${varName}) 12%, transparent)` } }
interface Props { entries: ListEntry[]; selectedId: string | null; onSelectEntry: (id: string) => void; sortOrder: SortOrder; onSortOrderChange: (order: SortOrder) => void }
export default function RequestList({ entries, selectedId, onSelectEntry, sortOrder, onSortOrderChange }: Props) {
  const { t } = useTranslation()
  const { gridRef, columnWidths, draggingCol, isDragging, onPointerDown, onPointerMove, onPointerUp } = useColumnResize()
  const gridTemplate = useMemo(() => COLS.map(c => `${columnWidths[c]}%`).join(' '), [columnWidths])
  const gridColsStyle = { gridTemplateColumns: 'var(--grid-cols)' } as React.CSSProperties
  return (
    <div ref={gridRef} className={`h-full bg-background flex flex-col overflow-auto ${isDragging ? 'select-none' : ''}`} style={{ cursor: isDragging ? 'col-resize' : '', '--grid-cols': gridTemplate } as React.CSSProperties}>
      <div className="grid shrink-0 z-10 bg-muted/30 text-[11px] font-bold text-muted-foreground uppercase tracking-wide border-b border-border" style={gridColsStyle}>
        <div className="px-1.5 py-1.5 font-normal normal-case text-left">{t('requestList.id')}</div>
        <div className="px-1.5 py-1.5 text-left"><div className="flex items-center gap-1"><Grip className={draggingCol === 'id' ? 'text-primary' : 'text-muted-foreground/25 hover:text-muted-foreground/70'} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerDown={e => onPointerDown('id', e)} /><span className="truncate">{t('requestList.url')}</span></div></div>
        <div className="px-1.5 py-1.5 text-left"><div className="flex items-center gap-1"><Grip className={draggingCol === 'url' ? 'text-primary' : 'text-muted-foreground/25 hover:text-muted-foreground/70'} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerDown={e => onPointerDown('url', e)} /><span className="truncate">{t('requestList.method')}</span></div></div>
        <div className="px-1.5 py-1.5 text-left"><div className="flex items-center gap-1"><Grip className={draggingCol === 'method' ? 'text-primary' : 'text-muted-foreground/25 hover:text-muted-foreground/70'} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerDown={e => onPointerDown('method', e)} /><span className="truncate">{t('requestList.status')}</span></div></div>
        <div className="px-1.5 py-1.5 text-left"><div className="flex items-center gap-1"><Grip className={draggingCol === 'status' ? 'text-primary' : 'text-muted-foreground/25 hover:text-muted-foreground/70'} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerDown={e => onPointerDown('status', e)} /><span className="truncate">{t('requestList.duration')}</span></div></div>
        <div className="px-1.5 py-1.5 text-right"><div className="flex items-center justify-end gap-1"><Grip className={draggingCol === 'duration' ? 'text-primary' : 'text-muted-foreground/25 hover:text-muted-foreground/70'} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerDown={e => onPointerDown('duration', e)} /><button type="button" className="cursor-pointer hover:text-foreground transition-colors whitespace-nowrap" onClick={() => onSortOrderChange(sortOrder === 'desc' ? 'asc' : 'desc')}>{sortOrder === 'desc' ? (<ArrowDownIcon className="inline size-3 mr-0.5" />) : (<ArrowUpIcon className="inline size-3 mr-0.5" />)}{t('requestList.time')}</button></div></div>
        <div className="px-1.5 py-1.5 text-left"><div className="flex items-center gap-1"><Grip className={draggingCol === 'time' ? 'text-primary' : 'text-muted-foreground/25 hover:text-muted-foreground/70'} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerDown={e => onPointerDown('time', e)} /><span className="truncate">{t('requestList.edited')}</span></div></div>
      </div>
      {entries.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground text-sm"><p>{t('requestList.emptyTitle')}</p><p className="mt-1 text-xs">{t('requestList.emptyHint')}</p></div>
      ) : (
       <VList key={sortOrder} style={{ flex: 1, minHeight: 0 }}>
         {entries.map((entry, i) => (
            <div key={entry.id} className={cn('grid text-xs border-b border-border/50 cursor-pointer transition-colors', entry.id === selectedId && 'bg-accent')} style={gridColsStyle} onClick={() => onSelectEntry(entry.id)}>
              <div className="px-1 py-2 tabular-nums text-[10px] text-muted-foreground text-left">{entry.requestNumber}</div>
              <div className="px-1 py-2 text-foreground/80 min-w-0" title={entry.uri}><span className="block truncate">{shortenUri(entry.uri)}</span></div>
              <div className="px-1 py-2"><span className="badge-method" style={badgeStyle(`--badge-${entry.method.toLowerCase()}`)}>{entry.method}</span></div>
              <div className="px-1 py-2"><span className="badge-status" style={badgeStyle(`--badge-${statusCategory(entry.status)}`)}>{entry.status ?? t('requestList.pending')}</span></div>
              <div className="px-1 py-2 text-muted-foreground">{formatDuration(entry.durationMs)}</div>
              <div className="px-1 py-2 text-muted-foreground/60 text-right">{formatTime(entry.requestTimestamp)}</div>
              <div className="px-1 py-2 text-center">{entry.edited ? (<span className="text-[10px] text-amber-400 font-medium">{t('requestList.edited')}</span>) : null}</div>
            </div>
          ))}
        </VList>
      )}
    </div>
  )
}
