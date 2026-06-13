import { useState, useCallback, useMemo, useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { TrafficEntry } from '@/types/proxy'
import { extractHost, classifyEntry, type TypeFilter } from '@/lib/format'
import { buildFullUrl } from '@/lib/http-constants'
import DomainSidebar from './DomainSidebar'
import RequestList from './RequestList'
import type { SortOrder, SortColumn } from './RequestList'
import DetailPanel from '@/features/detail-panel/DetailPanel'
import EditRequestDialog from './EditRequestDialog'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'

import type { DetailPosition } from '@/features/bottom-bar'
import type { PanelImperativeHandle } from 'react-resizable-panels'

interface Props {
  entries: TrafficEntry[]
  showDomainSidebar: boolean
  detailPosition: DetailPosition
  onAutoOpenDetail: () => void
  typeFilter: TypeFilter
}

export default function TrafficLog({ entries, showDomainSidebar, detailPosition, onAutoOpenDetail, typeFilter }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selectedDomain, setSelectedDomain] = useState<string | null>(null)
  const [sortColumn, setSortColumn] = useState<SortColumn>(null)
  const [sortOrder, setSortOrder] = useState<SortOrder>('desc')
  const [editEntry, setEditEntry] = useState<TrafficEntry | null>(null)
  const [pinnedDomains, setPinnedDomains] = useState<Set<string>>(() => {
    try {
      const stored = localStorage.getItem('ai-proxy-pinned-domains')
      if (stored) return new Set(JSON.parse(stored))
    } catch {}
    return new Set()
  })

  const domainPanelRef = useRef<PanelImperativeHandle>(null)

  // Sync domain sidebar collapse/expand with showDomainSidebar prop
  useEffect(() => {
    const panel = domainPanelRef.current
    if (!panel) return
    if (showDomainSidebar) {
      panel.resize(18)
    } else {
      panel.collapse()
    }
  }, [showDomainSidebar])

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
    let result = entries
    if (selectedDomain) {
      result = result.filter(e => {
        const host = extractHost(e.uri)
        if (host !== '(unknown)') return host === selectedDomain
        const hostHeader = e.requestHeaders?.['host'] ?? e.requestHeaders?.['Host'] ?? ''
        return hostHeader.startsWith(selectedDomain)
      })
    }
    if (typeFilter !== 'all') {
      result = result.filter(e => classifyEntry(e) === typeFilter)
    }
    return result
  }, [entries, selectedDomain, typeFilter])

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
      setSelectedId(id)
      if (detailPosition === 'hidden') {
        onAutoOpenDetail()
      }
    },
    [detailPosition, onAutoOpenDetail]
  )

  const handleCloseDetail = useCallback(() => {
    setSelectedId(null)
  }, [])

  const handleEditRequest = useCallback((entry: TrafficEntry) => {
    setEditEntry(entry)
  }, [])

  const handleSendSuccess = useCallback((entryId: string) => {
    setSelectedId(entryId)
    setEditEntry(null)
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
    } catch (err) {
      console.error('resend invoke failed:', err)
    }
  }, [])

  const handleSortChange = useCallback((column: SortColumn, order: SortOrder) => {
    setSortColumn(column)
    setSortOrder(order)
  }, [])

  const requestList = (
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
  )

  const detailPanel = <DetailPanel entry={selected} onClose={handleCloseDetail} />

  // Inner content: varies by detailPosition.
  const mainContent = (() => {
    if (detailPosition === 'hidden' || !selected) {
      return (
        <div className="h-full min-h-0 min-w-0">
          {requestList}
        </div>
      )
    }
    if (detailPosition === 'bottom') {
      return (
        <ResizablePanelGroup key="bottom" orientation="vertical" id="main-bottom" className="h-full">
          <ResizablePanel id="list" defaultSize={40} minSize={25} maxSize={75}>
            <div className="h-full min-h-0">
              {requestList}
            </div>
          </ResizablePanel>
          <ResizableHandle withHandle />
          <ResizablePanel id="detail" defaultSize={60} minSize={25} maxSize={75}>
            <div className="h-full min-h-0">
              {detailPanel}
            </div>
          </ResizablePanel>
        </ResizablePanelGroup>
      )
    }
    // right
    return (
      <ResizablePanelGroup key="right" orientation="horizontal" id="main-right" className="h-full">
        <ResizablePanel id="list" defaultSize={55} minSize={30} maxSize={75}>
          <div className="h-full min-h-0 min-w-0">
            {requestList}
          </div>
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel id="detail" defaultSize={45} minSize={25} maxSize={70}>
          <div className="h-full min-h-0 min-w-0">
            {detailPanel}
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    )
  })()

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <ResizablePanelGroup orientation="horizontal" id="trafficlog-outer" className="h-full">
        <ResizablePanel
          id="domain-sidebar"
          defaultSize={showDomainSidebar ? 18 : 0}
          minSize={8}
          maxSize={35}
          collapsible
          collapsedSize={0}
          panelRef={domainPanelRef}>
          <div className="h-full min-h-0 overflow-hidden">
            <DomainSidebar
              domains={domains}
              totalEntries={entries.length}
              selectedDomain={selectedDomain}
              onSelectDomain={setSelectedDomain}
              pinnedDomains={pinnedDomains}
              onTogglePin={handleTogglePin}
            />
          </div>
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel id="main" defaultSize={82} minSize={50}>
          {mainContent}
        </ResizablePanel>
      </ResizablePanelGroup>

      <EditRequestDialog
        open={editEntry !== null}
        onOpenChange={(open) => { if (!open) setEditEntry(null) }}
        entry={editEntry}
        onSendSuccess={handleSendSuccess}
      />
    </div>
  )
}
