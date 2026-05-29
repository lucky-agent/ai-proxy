import { useState, useRef, useCallback, useMemo } from 'react'
import type { TrafficEntry } from '@/types/proxy'
import { extractHost } from '@/lib/format'
import DomainSidebar from './DomainSidebar'
import RequestList from './RequestList'
import type { SortOrder } from './RequestList'
import DetailPanel from './DetailPanel'

const MIN_DOMAIN_RATIO = 0.08
const MAX_DOMAIN_RATIO = 0.4
const MIN_SPLIT_RATIO = 0.15
const MAX_SPLIT_RATIO = 0.7

interface Props {
  entries: TrafficEntry[]
}

export default function TrafficLog({ entries }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selectedDomain, setSelectedDomain] = useState<string | null>(null)
  const [domainRatio, setDomainRatio] = useState(0.15)
  const [splitRatio, setSplitRatio] = useState(0.4)
  const [draggingDomain, setDraggingDomain] = useState(false)
  const [draggingSplit, setDraggingSplit] = useState(false)
  const [sortOrder, setSortOrder] = useState<SortOrder>('desc')

  const containerRef = useRef<HTMLDivElement>(null)
  const mainAreaRef = useRef<HTMLDivElement>(null)
  const domainRef = useRef<HTMLDivElement>(null)
  const requestListRef = useRef<HTMLDivElement>(null)

  // Live ratios for direct DOM manipulation during drag — avoids React re-renders
  const liveDomainRatio = useRef(domainRatio)
  const liveSplitRatio = useRef(splitRatio)
  // Sync live refs with state only when NOT dragging (prevents snap-back on mid-drag re-renders)
  if (!draggingDomain) liveDomainRatio.current = domainRatio
  if (!draggingSplit) liveSplitRatio.current = splitRatio

  const domains = useMemo(() => {
    const map = new Map<string, number>()
    for (const e of entries) {
      const host = extractHost(e.uri)
      map.set(host, (map.get(host) ?? 0) + 1)
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [entries])

  const filtered = useMemo(() => {
    if (!selectedDomain) return entries
    return entries.filter((e) => extractHost(e.uri) === selectedDomain)
  }, [entries, selectedDomain])

  const sorted = useMemo(() => {
    const copy = [...filtered]
    copy.sort((a, b) =>
      sortOrder === 'desc'
        ? b.requestTimestamp - a.requestTimestamp
        : a.requestTimestamp - b.requestTimestamp
    )
    return copy.map((entry, i) => ({ ...entry, listIndex: i + 1 }))
  }, [filtered, sortOrder])

  const selected = entries.find((e) => e.id === selectedId)

  const onDomainPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setDraggingDomain(true)
    e.currentTarget.setPointerCapture(e.pointerId)
  }, [])

  const onDomainPointerMove = useCallback((e: React.PointerEvent) => {
    if (!draggingDomain || !containerRef.current) return
    const rect = containerRef.current.getBoundingClientRect()
    const ratio = Math.max(MIN_DOMAIN_RATIO, Math.min(MAX_DOMAIN_RATIO, (e.clientX - rect.left) / rect.width))
    liveDomainRatio.current = ratio
    if (domainRef.current) domainRef.current.style.width = `${ratio * 100}%`
  }, [draggingDomain])

  const onDomainPointerUp = useCallback(() => {
    setDomainRatio(liveDomainRatio.current)
    setDraggingDomain(false)
  }, [])

  const onSplitPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setDraggingSplit(true)
    e.currentTarget.setPointerCapture(e.pointerId)
  }, [])

  const onSplitPointerMove = useCallback((e: React.PointerEvent) => {
    if (!draggingSplit || !mainAreaRef.current) return
    const rect = mainAreaRef.current.getBoundingClientRect()
    const ratio = Math.max(MIN_SPLIT_RATIO, Math.min(MAX_SPLIT_RATIO, (e.clientY - rect.top) / rect.height))
    liveSplitRatio.current = ratio
    if (requestListRef.current) requestListRef.current.style.height = `${ratio * 100}%`
  }, [draggingSplit])

  const onSplitPointerUp = useCallback(() => {
    setSplitRatio(liveSplitRatio.current)
    setDraggingSplit(false)
  }, [])

  const isDragging = draggingDomain || draggingSplit

  return (
    <div
      ref={containerRef}
      className={`flex min-h-0 flex-1 overflow-hidden ${isDragging ? 'select-none' : ''}`}
      style={{ cursor: draggingDomain ? 'col-resize' : draggingSplit ? 'row-resize' : '' }}
    >
        <div
          ref={domainRef}
          className="h-full min-h-0 shrink-0 overflow-hidden"
          style={{ width: `${liveDomainRatio.current * 100}%` }}>
          <DomainSidebar
            domains={domains}
            totalEntries={entries.length}
            selectedDomain={selectedDomain}
            onSelectDomain={setSelectedDomain}
          />
        </div>

        {/* Domain Resize Handle */}
        <div
          onPointerDown={onDomainPointerDown}
          onPointerMove={onDomainPointerMove}
          onPointerUp={onDomainPointerUp}
          className="group relative w-[1px] shrink-0 cursor-col-resize bg-border transition-colors hover:bg-primary/50 active:bg-primary/70"
        >
          <div className="absolute inset-y-0 -left-2 -right-2" />
          <div className="absolute inset-y-0 left-1/2 -translate-x-1/2 flex flex-col items-center justify-center gap-[3px] opacity-0 group-hover:opacity-100 transition-opacity">
            <span className="block size-[3px] rounded-full bg-muted-foreground" />
            <span className="block size-[3px] rounded-full bg-muted-foreground" />
            <span className="block size-[3px] rounded-full bg-muted-foreground" />
          </div>
        </div>

        {/* Main Area: top-bottom split */}
        <div ref={mainAreaRef} className="flex min-h-0 flex-col flex-1 overflow-hidden">
          <div
            ref={requestListRef}
            className="min-h-0 shrink-0 overflow-hidden"
            style={{ height: `${liveSplitRatio.current * 100}%` }}>
            <RequestList
              entries={sorted}
              selectedId={selectedId}
              onSelectEntry={setSelectedId}
              sortOrder={sortOrder}
              onSortOrderChange={setSortOrder}
            />
          </div>

          {/* Split Resize Handle (horizontal) */}
          <div
            onPointerDown={onSplitPointerDown}
            onPointerMove={onSplitPointerMove}
            onPointerUp={onSplitPointerUp}
            className="group relative h-[1px] shrink-0 cursor-row-resize bg-border transition-colors hover:bg-primary/50 active:bg-primary/70"
          >
            <div className="absolute inset-x-0 -top-2 -bottom-2" />
            <div className="absolute left-1/2 -translate-x-1/2 top-1/2 -translate-y-1/2 flex items-center gap-[3px] opacity-0 group-hover:opacity-100 transition-opacity">
              <span className="block size-[3px] rounded-full bg-muted-foreground" />
              <span className="block size-[3px] rounded-full bg-muted-foreground" />
              <span className="block size-[3px] rounded-full bg-muted-foreground" />
            </div>
          </div>

          <DetailPanel entry={selected} />
        </div>
    </div>
  )
}