import { useState, useCallback, useRef, useEffect } from 'react'
import type { TrafficEntry } from '@/types/proxy'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import type { PanelImperativeHandle } from 'react-resizable-panels'
import SummaryBar from './components/SummaryBar'
import RequestPanel from './RequestPanel'
import ResponsePanel from './ResponsePanel'

interface Props {
  entry: TrafficEntry | undefined
  onClose?: () => void
  /** 是否展示左侧请求面板，默认 true。false 时只展示响应面板 */
  showRequest?: boolean
}

export default function DetailPanel({ entry, onClose, showRequest = true }: Props) {
  const requestPanelRef = useRef<PanelImperativeHandle>(null)
  const responsePanelRef = useRef<PanelImperativeHandle>(null)

  // null = split 50/50, 'request' = request full, 'response' = response full
  const [fullPanel, setFullPanel] = useState<'request' | 'response' | null>(null)

  // 切换条目时重置面板状态
  useEffect(() => {
    if (!entry) return
    setFullPanel(null)
    const req = requestPanelRef.current
    const resp = responsePanelRef.current
    if (req?.isCollapsed()) req.resize(50)
    if (resp?.isCollapsed()) resp.resize(50)
  }, [entry?.id])

  const handleRequestTitleClick = useCallback(() => {
    const req = requestPanelRef.current
    const resp = responsePanelRef.current
    if (!req || !resp) return

    if (fullPanel === 'request') {
      // Request 已占满 → 恢复 50/50
      resp.expand()
      req.resize(50)
      setFullPanel(null)
    } else {
      // 50/50 或 Response 占满 → Request 占满
      req.resize(100)
      resp.collapse()
      setFullPanel('request')
    }
  }, [fullPanel])

  const handleResponseTitleClick = useCallback(() => {
    const req = requestPanelRef.current
    const resp = responsePanelRef.current
    if (!req || !resp) return

    if (fullPanel === 'response') {
      // Response 已占满 → 恢复 50/50
      req.expand()
      resp.resize(50)
      setFullPanel(null)
    } else {
      // 50/50 或 Request 占满 → Response 占满
      resp.resize(100)
      req.collapse()
      setFullPanel('response')
    }
  }, [fullPanel])

  return (
    <div className="flex min-h-0 min-w-0 h-full flex-col overflow-hidden bg-surface-base">
      {entry && <SummaryBar entry={entry} onClose={onClose} />}

      <ResizablePanelGroup orientation="horizontal" id="detail-panel" className="min-h-0 flex-1">
        {showRequest && (
          <>
            <ResizablePanel
              id="request"
              defaultSize="50"
              minSize="15"
              collapsible
              collapsedSize={0}
              panelRef={requestPanelRef}>
              <div className="flex flex-col min-h-0 min-w-0 h-full overflow-hidden">
                <RequestPanel entry={entry} onTitleClick={handleRequestTitleClick} />
              </div>
            </ResizablePanel>
            <ResizableHandle withHandle />
          </>
        )}

        <ResizablePanel
          id="response"
          defaultSize={showRequest ? "50" : "100"}
          minSize="15"
          collapsible
          collapsedSize={0}
          panelRef={responsePanelRef}>
          <div className="flex flex-col min-h-0 min-w-0 h-full overflow-hidden">
            <ResponsePanel entry={entry} onTitleClick={handleResponseTitleClick} />
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  )
}
