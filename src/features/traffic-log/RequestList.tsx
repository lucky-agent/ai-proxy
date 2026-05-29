import { useState, useRef, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { ArrowDownIcon, ArrowUpIcon } from 'lucide-react'
import type { TrafficEntry } from '@/types/proxy'
import { statusCategory, formatDuration, formatTime, shortenUri } from '@/lib/format'
import { cn } from '@/lib/utils'

export type ListEntry = TrafficEntry & { listIndex: number }
export type SortOrder = 'desc' | 'asc'

type ColKey = 'id' | 'url' | 'method' | 'status' | 'duration' | 'time' | 'edited'

const DEFAULT_WIDTHS: Record<ColKey, number> = {
  id: 9,
  url: 29,
  method: 9,
  status: 9,
  duration: 9,
  time: 22,
  edited: 13,
}

const MIN_PCT = 4
const MAX_PCT = 40

function useColumnResize() {
  const [columnWidths, setColumnWidths] = useState<Record<ColKey, number>>(DEFAULT_WIDTHS)
  const [draggingCol, setDraggingCol] = useState<ColKey | null>(null)

  const tableRef = useRef<HTMLTableElement | null>(null)
  const idColRef = useRef<HTMLTableColElement | null>(null)
  const urlColRef = useRef<HTMLTableColElement | null>(null)
  const methodColRef = useRef<HTMLTableColElement | null>(null)
  const statusColRef = useRef<HTMLTableColElement | null>(null)
  const durationColRef = useRef<HTMLTableColElement | null>(null)
  const timeColRef = useRef<HTMLTableColElement | null>(null)
  const editedColRef = useRef<HTMLTableColElement | null>(null)

  const colRefs: Record<ColKey, React.RefObject<HTMLTableColElement | null>> = {
    id: idColRef,
    url: urlColRef,
    method: methodColRef,
    status: statusColRef,
    duration: durationColRef,
    time: timeColRef,
    edited: editedColRef,
  }

  const liveWidths = useRef(DEFAULT_WIDTHS)
  const dragStart = useRef<{ col: ColKey; startX: number; startPct: number } | null>(null)

  if (!draggingCol) liveWidths.current = columnWidths

  const onPointerDown = useCallback((colKey: ColKey, e: React.PointerEvent) => {
    e.preventDefault()
    e.currentTarget.setPointerCapture(e.pointerId)
    setDraggingCol(colKey)
    dragStart.current = {
      col: colKey,
      startX: e.clientX,
      startPct: liveWidths.current[colKey],
    }
  }, [])

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragStart.current || !tableRef.current) return
    const { col, startX, startPct } = dragStart.current
    const tableWidth = tableRef.current.offsetWidth
    if (!tableWidth) return
    const deltaPct = ((e.clientX - startX) / tableWidth) * 100
    const newPct = Math.round(Math.max(MIN_PCT, Math.min(MAX_PCT, startPct + deltaPct)))
    liveWidths.current = { ...liveWidths.current, [col]: newPct }
    const colEl = colRefs[col].current
    if (colEl) colEl.style.width = `${newPct}%`
  }, [])

  const onPointerUp = useCallback(() => {
    if (!dragStart.current) return
    setColumnWidths(liveWidths.current)
    setDraggingCol(null)
    dragStart.current = null
  }, [])

  const isDragging = draggingCol !== null

  return {
    tableRef,
    columnWidths,
    draggingCol,
    isDragging,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    idColRef,
    urlColRef,
    methodColRef,
    statusColRef,
    durationColRef,
    timeColRef,
    editedColRef,
  }
}

const Grip = ({
  className,
  onPointerDown,
  onPointerMove,
  onPointerUp,
}: {
  className?: string
  onPointerDown?: (e: React.PointerEvent) => void
  onPointerMove?: (e: React.PointerEvent) => void
  onPointerUp?: (e: React.PointerEvent) => void
}) => (
  <span
    className={cn('shrink-0 cursor-col-resize p-[2px] -mr-[2px] relative z-10', className)}
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

interface Props {
  entries: ListEntry[]
  selectedId: string | null
  onSelectEntry: (id: string) => void
  sortOrder: SortOrder
  onSortOrderChange: (order: SortOrder) => void
}

export default function RequestList({
  entries,
  selectedId,
  onSelectEntry,
  sortOrder,
  onSortOrderChange,
}: Props) {
  const { t } = useTranslation()

  const {
    tableRef,
    columnWidths,
    draggingCol,
    isDragging,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    idColRef,
    urlColRef,
    methodColRef,
    statusColRef,
    durationColRef,
    timeColRef,
    editedColRef,
  } = useColumnResize()

  return (
    <div
      className={`overflow-auto h-full bg-background ${isDragging ? 'select-none' : ''}`}
      style={{ cursor: isDragging ? 'col-resize' : '' }}>
      <table ref={tableRef} className="border-collapse table-fixed w-full">
        <colgroup>
          <col ref={idColRef} style={{ width: `${columnWidths.id}%` }} />
          <col ref={urlColRef} style={{ width: `${columnWidths.url}%` }} />
          <col ref={methodColRef} style={{ width: `${columnWidths.method}%` }} />
          <col ref={statusColRef} style={{ width: `${columnWidths.status}%` }} />
          <col ref={durationColRef} style={{ width: `${columnWidths.duration}%` }} />
          <col ref={timeColRef} style={{ width: `${columnWidths.time}%` }} />
          <col ref={editedColRef} style={{ width: `${columnWidths.edited}%` }} />
        </colgroup>

        {/* ---- header ---- */}
        <thead className="sticky top-0 z-10 bg-muted/30 text-[11px] font-bold text-muted-foreground uppercase tracking-wide">
          <tr className="border-b border-border">
            <th className="px-1.5 py-1.5 font-normal normal-case text-left">
              {t('requestList.id')}
            </th>

            <th className="px-1.5 py-1.5 text-left">
              <div className="flex items-center gap-1">
                <Grip
                  className={
                    draggingCol === 'url'
                      ? 'text-primary'
                      : 'text-muted-foreground/25 hover:text-muted-foreground/70'
                  }
                  onPointerMove={onPointerMove}
                  onPointerUp={onPointerUp}
                  onPointerDown={e => onPointerDown('id', e)}
                />
                <span className="truncate">{t('requestList.url')}</span>
              </div>
            </th>

            <th className="px-1.5 py-1.5 text-left">
              <div className="flex items-center gap-1">
                <Grip
                  className={
                    draggingCol === 'method'
                      ? 'text-primary'
                      : 'text-muted-foreground/25 hover:text-muted-foreground/70'
                  }
                  onPointerMove={onPointerMove}
                  onPointerUp={onPointerUp}
                  onPointerDown={e => onPointerDown('url', e)}
                />
                <span className="truncate">{t('requestList.method')}</span>
              </div>
            </th>

            <th className="px-1.5 py-1.5 text-left">
              <div className="flex items-center gap-1">
                <Grip
                  className={
                    draggingCol === 'status'
                      ? 'text-primary'
                      : 'text-muted-foreground/25 hover:text-muted-foreground/70'
                  }
                  onPointerMove={onPointerMove}
                  onPointerUp={onPointerUp}
                  onPointerDown={e => onPointerDown('method', e)}
                />
                <span className="truncate">{t('requestList.status')}</span>
              </div>
            </th>

            <th className="px-1.5 py-1.5 text-left">
              <div className="flex items-center gap-1">
                <Grip
                  className={
                    draggingCol === 'duration'
                      ? 'text-primary'
                      : 'text-muted-foreground/25 hover:text-muted-foreground/70'
                  }
                  onPointerMove={onPointerMove}
                  onPointerUp={onPointerUp}
                  onPointerDown={e => onPointerDown('status', e)}
                />
                <span className="truncate">{t('requestList.duration')}</span>
              </div>
            </th>

            <th className="px-1.5 py-1.5 text-right">
              <div className="flex items-center justify-end gap-1">
                <Grip
                  className={
                    draggingCol === 'time'
                      ? 'text-primary'
                      : 'text-muted-foreground/25 hover:text-muted-foreground/70'
                  }
                  onPointerMove={onPointerMove}
                  onPointerUp={onPointerUp}
                  onPointerDown={e => onPointerDown('duration', e)}
                />
                <button
                  type="button"
                  className="cursor-pointer hover:text-foreground transition-colors whitespace-nowrap"
                  onClick={() => onSortOrderChange(sortOrder === 'desc' ? 'asc' : 'desc')}>
                  {sortOrder === 'desc' ? (
                    <ArrowDownIcon className="inline size-3 mr-0.5" />
                  ) : (
                    <ArrowUpIcon className="inline size-3 mr-0.5" />
                  )}
                  {t('requestList.time')}
                </button>
              </div>
            </th>

            <th className="px-1.5 py-1.5 text-left">
              <div className="flex items-center gap-1">
                <Grip
                  className={
                    draggingCol === 'edited'
                      ? 'text-primary'
                      : 'text-muted-foreground/25 hover:text-muted-foreground/70'
                  }
                  onPointerMove={onPointerMove}
                  onPointerUp={onPointerUp}
                  onPointerDown={e => onPointerDown('time', e)}
                />
                <span className="truncate">{t('requestList.edited')}</span>
              </div>
            </th>
          </tr>
        </thead>

        {/* ---- body ---- */}
        <tbody>
          {entries.length === 0 ? (
            <tr>
              <td colSpan={7} className="py-16 text-center text-muted-foreground text-sm">
                <p>{t('requestList.emptyTitle')}</p>
                <p className="mt-1 text-xs">{t('requestList.emptyHint')}</p>
              </td>
            </tr>
          ) : (
            entries.map(entry => (
              <tr
                key={entry.id}
                className={cn(
                  'text-xs border-b border-border/50 cursor-pointer transition-colors',
                  entry.id === selectedId && 'bg-accent'
                )}
                onClick={() => onSelectEntry(entry.id)}>
                <td className="px-1 py-2 tabular-nums text-[10px] text-muted-foreground text-left">
                  {entry.listIndex}
                </td>

                <td className="px-1 py-2 text-foreground/80" title={entry.uri}>
                  <span className="block truncate">{shortenUri(entry.uri)}</span>
                </td>

                <td className="px-1 py-2">
                  <span
                    className="badge-method"
                    style={badgeStyle(`--badge-${entry.method.toLowerCase()}`)}>
                    {entry.method}
                  </span>
                </td>

                <td className="px-1 py-2">
                  <span
                    className="badge-status"
                    style={badgeStyle(`--badge-${statusCategory(entry.status)}`)}>
                    {entry.status ?? t('requestList.pending')}
                  </span>
                </td>

                <td className="px-1 py-2 text-muted-foreground">
                  {formatDuration(entry.durationMs)}
                </td>

                <td className="px-1 py-2 text-muted-foreground/60 text-right">
                  {formatTime(entry.requestTimestamp)}
                </td>

                <td className="px-1 py-2 text-center">
                  {entry.edited ? (
                    <span className="text-[10px] text-amber-400 font-medium">
                      {t('requestList.edited')}
                    </span>
                  ) : null}
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  )
}
