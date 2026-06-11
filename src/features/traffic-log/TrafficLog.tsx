import { useState, useRef, useCallback, useMemo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { TrafficEntry } from '@/types/proxy'
import { extractHost } from '@/lib/format'
import DomainSidebar from './DomainSidebar'
import RequestList from './RequestList'
import type { SortOrder, SortColumn } from './RequestList'
import DetailPanel from './DetailPanel'
import EditRequestDialog from './EditRequestDialog'

const MIN_DOMAIN_RATIO = 0.08
const MAX_DOMAIN_RATIO = 0.4
const MIN_SPLIT_RATIO = 0.15
const MAX_SPLIT_RATIO = 0.7

interface Props {
  entries: TrafficEntry[]
}

/** 补全完整 URL：代理存储的 URI 可能只是路径（如 /v1/chat/completions） */
function buildFullUrl(entry: TrafficEntry): string {
  if (entry.uri.startsWith('http://') || entry.uri.startsWith('https://')) {
    return entry.uri
  }
  const host = entry.requestHeaders?.['host'] ?? entry.requestHeaders?.['Host'] ?? ''
  if (host) {
    return 'https://' + host + entry.uri
  }
  return entry.uri
}

export default function TrafficLog({ entries }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selectedDomain, setSelectedDomain] = useState<string | null>(null)
  const [domainRatio, setDomainRatio] = useState(0.15)
  const [splitRatio, setSplitRatio] = useState(0.4)
  const [draggingDomain, setDraggingDomain] = useState(false)
  const [draggingSplit, setDraggingSplit] = useState(false)
  const [sortColumn, setSortColumn] = useState<SortColumn>(null)
  const [sortOrder, setSortOrder] = useState<SortOrder>('desc')
  const [detailOpen, setDetailOpen] = useState(false)
  const [editEntry, setEditEntry] = useState<TrafficEntry | null>(null)
  const [domainCollapsed, setDomainCollapsed] = useState(false)
  const [pinnedDomains, setPinnedDomains] = useState<Set<string>>(() => {
    try {
      const stored = localStorage.getItem('ai-proxy-pinned-domains')
      if (stored) return new Set(JSON.parse(stored))
    } catch {}
    return new Set()
  })

  const handleTogglePin = useCallback((domain: string) => {
    setPinnedDomains(prev => {
      const next = new Set(prev)
      if (next.has(domain)) {
        next.delete(domain)
      } else {
        next.add(domain)
      }
      localStorage.setItem('ai-proxy-pinned-domains', JSON.stringify([...next]))
      return next
    })
  }, [])

  const containerRef = useRef<HTMLDivElement>(null)
  const mainAreaRef = useRef<HTMLDivElement>(null)
  const domainRef = useRef<HTMLDivElement>(null)
  const requestListRef = useRef<HTMLDivElement>(null)

  const liveDomainRatio = useRef(domainRatio)
  const liveSplitRatio = useRef(splitRatio)
  if (!draggingDomain) liveDomainRatio.current = domainRatio
  if (!draggingSplit) liveSplitRatio.current = splitRatio

  const domains = useMemo(() => {
    const map = new Map<string, number>()
    for (const e of entries) {
      const host = extractHost(e.uri)
      if (host === '(unknown)') {
        const hostHeader = e.requestHeaders?.['host'] ?? e.requestHeaders?.['Host'] ?? ''
        if (hostHeader) {
          const fallbackHost = hostHeader.split(':')[0]
          map.set(fallbackHost, (map.get(fallbackHost) ?? 0) + 1)
          continue
        }
      }
      map.set(host, (map.get(host) ?? 0) + 1)
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1])
  }, [entries])

  const filtered = useMemo(() => {
    if (!selectedDomain) return entries
    return entries.filter(e => {
      const host = extractHost(e.uri)
      if (host !== '(unknown)') return host === selectedDomain
      const hostHeader = e.requestHeaders?.['host'] ?? e.requestHeaders?.['Host'] ?? ''
      return hostHeader.startsWith(selectedDomain)
    })
  }, [entries, selectedDomain])

  const sorted = useMemo(() => {
    const copy = [...filtered]
    if (!sortColumn) return copy
    copy.sort((a, b) => {
      let cmp: number
      switch (sortColumn) {
        case 'id':
          cmp = a.requestNumber - b.requestNumber
          break
        case 'url':
          cmp = a.uri.localeCompare(b.uri)
          break
        case 'method':
          cmp = a.method.localeCompare(b.method)
          break
        case 'status':
          cmp = (a.status ?? -1) - (b.status ?? -1)
          break
        case 'duration':
          cmp = (a.durationMs ?? -1) - (b.durationMs ?? -1)
          break
        case 'time':
          cmp = a.requestTimestamp - b.requestTimestamp
          break
       case 'edited':
         cmp = (a.edited ? 1 : 0) - (b.edited ? 1 : 0)
         break
        case 'ssl':
          cmp = (a.decrypted ? 1 : 0) - (b.decrypted ? 1 : 0)
          break
       default:
          return 0
      }
      return sortOrder === 'desc' ? -cmp : cmp
    })
    return copy
  }, [filtered, sortColumn, sortOrder])

  const selected = entries.find(e => e.id === selectedId)

  const handleSelectEntry = useCallback(
    (id: string) => {
      if (detailOpen && selectedId === id) {
        setDetailOpen(false)
        setSelectedId(null)
      } else {
        setSelectedId(id)
        setDetailOpen(true)
      }
    },
    [detailOpen, selectedId]
  )

  const handleCloseDetail = useCallback(() => {
    setDetailOpen(false)
    setSelectedId(null)
  }, [])

  const handleEditRequest = useCallback((entry: TrafficEntry) => {
    setEditEntry(entry)
  }, [])

  const handleResendEdited = useCallback(async (
    method: string,
    url: string,
    headers: Record<string, string>,
    body: string | null,
  ) => {
    try {
      const entryId = await invoke<string>('resend_request', {
        method,
        url,
        headers,
        body,
      })
      setSelectedId(entryId)
      setDetailOpen(true)
      setEditEntry(null)
    } catch (err) {
      console.error('resend invoke failed:', err)
    }
  }, [])

  const handleResendRequest = useCallback(async (entry: TrafficEntry) => {
    const headers: Record<string, string> = {}
    for (const [k, v] of Object.entries(entry.requestHeaders)) {
      const lk = k.toLowerCase()
      if (lk === 'host' || lk === 'content-length' || lk === 'transfer-encoding') continue
      headers[k] = v
    }

    try {
      const entryId = await invoke<string>('resend_request', {
        method: entry.method,
        url: buildFullUrl(entry),
        headers,
        body: entry.requestBody,
      })
      setSelectedId(entryId)
      setDetailOpen(true)
    } catch (err) {
      console.error('resend invoke failed:', err)
    }
  }, [])

  const handleSortChange = useCallback((column: SortColumn, order: SortOrder) => {
    setSortColumn(column)
    setSortOrder(order)
  }, [])

  // --- drag handlers ---
  const onDomainPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setDraggingDomain(true)
    e.currentTarget.setPointerCapture(e.pointerId)
  }, [])

  const onDomainPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!draggingDomain || !containerRef.current) return
      const rect = containerRef.current.getBoundingClientRect()
      const ratio = Math.max(MIN_DOMAIN_RATIO, Math.min(MAX_DOMAIN_RATIO, (e.clientX - rect.left) / rect.width))
      liveDomainRatio.current = ratio
      if (domainRef.current) domainRef.current.style.width = `${ratio * 100}%`
    },
    [draggingDomain]
  )

  const onDomainPointerUp = useCallback(() => {
    setDomainRatio(liveDomainRatio.current)
    setDraggingDomain(false)
  }, [])

  const onSplitPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    setDraggingSplit(true)
    e.currentTarget.setPointerCapture(e.pointerId)
  }, [])

  const onSplitPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!draggingSplit || !mainAreaRef.current) return
      const rect = mainAreaRef.current.getBoundingClientRect()
      const ratio = Math.max(MIN_SPLIT_RATIO, Math.min(MAX_SPLIT_RATIO, (e.clientY - rect.top) / rect.height))
      liveSplitRatio.current = ratio
      if (requestListRef.current) requestListRef.current.style.height = `${ratio * 100}%`
    },
    [draggingSplit]
  )

  const onSplitPointerUp = useCallback(() => {
    setSplitRatio(liveSplitRatio.current)
    setDraggingSplit(false)
  }, [])

  const isDragging = draggingDomain || draggingSplit

return (
    <div
      ref={containerRef}
      className={`flex min-h-0 flex-1 overflow-hidden ${isDragging ? 'select-none' : ''}`}
      style={{ cursor: draggingDomain ? 'col-resize' : draggingSplit ? 'row-resize' : '' }}>
      <div
        ref={domainRef}
        className="h-full min-h-0 shrink-0 overflow-hidden"
        style={{ width: domainCollapsed ? '28px' : `${liveDomainRatio.current * 100}%` }}>
        <DomainSidebar
          domains={domains}
          totalEntries={entries.length}
          selectedDomain={selectedDomain}
          onSelectDomain={setSelectedDomain}
          panelCollapsed={domainCollapsed}
          onTogglePanel={() => setDomainCollapsed(!domainCollapsed)}
          pinnedDomains={pinnedDomains}
          onTogglePin={handleTogglePin}
        />
      </div>

      <div
        onPointerDown={onDomainPointerDown}
        onPointerMove={onDomainPointerMove}
        onPointerUp={onDomainPointerUp}
        className="group relative w-[1px] shrink-0 cursor-col-resize bg-border transition-colors hover:bg-primary/50 active:bg-primary/70">
        <div className="absolute inset-y-0 -left-2 -right-2" />
        <div className="absolute inset-y-0 left-1/2 -translate-x-1/2 flex flex-col items-center justify-center gap-[3px] opacity-0 group-hover:opacity-100 transition-opacity">
          <span className="block size-[3px] rounded-full bg-muted-foreground" />
          <span className="block size-[3px] rounded-full bg-muted-foreground" />
          <span className="block size-[3px] rounded-full bg-muted-foreground" />
        </div>
      </div>

      <div ref={mainAreaRef} className="flex min-h-0 flex-col flex-1 overflow-hidden">
        <div
          ref={requestListRef}
          className="min-h-0"
          style={detailOpen ? { height: `${liveSplitRatio.current * 100}%` } : { flex: 1 }}>
          <RequestList
            entries={sorted}
            selectedId={selectedId}
            onSelectEntry={handleSelectEntry}
            sortColumn={sortColumn}
            sortOrder={sortOrder}
            onSortChange={handleSortChange}
            onResendRequest={handleResendRequest}
            onEditRequest={handleEditRequest}
          />
        </div>

        {detailOpen && (
          <>
            <div
              onPointerDown={onSplitPointerDown}
              onPointerMove={onSplitPointerMove}
              onPointerUp={onSplitPointerUp}
              className="group relative h-[1px] shrink-0 cursor-row-resize bg-border transition-colors hover:bg-primary/50 active:bg-primary/70">
              <div className="absolute inset-x-0 -top-2 -bottom-2" />
              <div className="absolute left-1/2 -translate-x-1/2 top-1/2 -translate-y-1/2 flex items-center gap-[3px] opacity-0 group-hover:opacity-100 transition-opacity">
                <span className="block size-[3px] rounded-full bg-muted-foreground" />
                <span className="block size-[3px] rounded-full bg-muted-foreground" />
                <span className="block size-[3px] rounded-full bg-muted-foreground" />
              </div>
            </div>
            <DetailPanel entry={selected} onClose={handleCloseDetail} />
            <EditRequestDialog
              open={editEntry !== null}
              onOpenChange={(open) => { if (!open) setEditEntry(null) }}
              entry={editEntry}
              onResend={handleResendEdited}
            />
          </>
        )}
      </div>
    </div>
  )
}
