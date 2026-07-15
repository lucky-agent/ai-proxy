import { SparklesIcon } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState, useMemo, useEffect, useRef, useCallback } from 'react'
import { AiSidebar, type AiSelection } from './AiSidebar'
import { ConversationBubble } from './ConversationBubble'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { usePanelRef } from 'react-resizable-panels'
import type { AiConversation, AiSessionState, AiTurn } from '@/types/ai'

interface AiViewProps {
  sessions: AiSessionState[]
  mergedTimeline: (sessionId: string) => { turn: AiTurn; requestId: string }[]
  conversationOf: (requestId: string) => AiConversation | undefined
  showSidebar: boolean
  /** 点击气泡的跳转钮 → 切到代理视图并定位该请求。 */
  onJumpToProxy?: (requestId: string) => void
  /** 右键删除整个会话（仅前端移除） */
  onDeleteSession: (sessionId: string) => void
  /** 右键删除会话内单次请求（仅前端移除） */
  onDeleteRequest: (sessionId: string, requestId: string) => void
}

export function AiView({ sessions, mergedTimeline, conversationOf, showSidebar, onJumpToProxy, onDeleteSession, onDeleteRequest }: AiViewProps) {
  const { t } = useTranslation()
  const [selection, setSelection] = useState<AiSelection | null>(null)

  const aiSidebarPanelRef = usePanelRef()
  useEffect(() => {
    const panel = aiSidebarPanelRef.current
    if (!panel) return
    if (showSidebar) panel.resize('22%')
    else panel.collapse()
  }, [showSidebar])

  const selectedSession = useMemo(
    () => (selection ? sessions.find((s) => s.sessionId === selection.sessionId) ?? null : null),
    [selection, sessions],
  )

  // 删除时修正选中态：删中当前会话 → 清空；删中当前请求 → 回落会话头（删空则清空）
  const handleDeleteSession = useCallback((sessionId: string) => {
    onDeleteSession(sessionId)
    setSelection((sel) => (sel?.sessionId === sessionId ? null : sel))
  }, [onDeleteSession])

  const handleDeleteRequest = useCallback((sessionId: string, requestId: string) => {
    const remaining = sessions.find((s) => s.sessionId === sessionId)?.requestIds.filter((rid) => rid !== requestId).length ?? 0
    onDeleteRequest(sessionId, requestId)
    setSelection((sel) => {
      if (sel?.sessionId !== sessionId) return sel
      if (remaining === 0) return null
      return sel.requestId === requestId ? { sessionId } : sel
    })
  }, [onDeleteRequest, sessions])

  /** requestId → #req 序号（会话内位置） */
  const reqIndex = useMemo(() => {
    const m = new Map<string, number>()
    selectedSession?.requestIds.forEach((rid, i) => m.set(rid, i + 1))
    return m
  }, [selectedSession])

  // 会话头 → 合并时间线；单请求 → 该次完整对话
  const rendered = useMemo<{ turn: AiTurn; requestId: string }[]>(() => {
    if (!selection || !selectedSession) return []
    if (selection.requestId) {
      const conv = conversationOf(selection.requestId)
      return conv ? conv.turns.map((turn) => ({ turn, requestId: selection.requestId! })) : []
    }
    return mergedTimeline(selection.sessionId)
  }, [selection, selectedSession, mergedTimeline, conversationOf])

  const isStreamingReq = (requestId: string): boolean =>
    conversationOf(requestId)?.streaming ?? false

  // 打开会话即滚到底部看最新内容；流式期间吸底跟随，用户主动上滚查看历史时不打扰
  const scrollRef = useRef<HTMLDivElement>(null)
  const stickToBottom = useRef(true)

  useEffect(() => {
    stickToBottom.current = true
  }, [selection])

  useEffect(() => {
    const el = scrollRef.current
    if (el && stickToBottom.current) el.scrollTop = el.scrollHeight
  }, [rendered])

  const handleScroll = () => {
    const el = scrollRef.current
    if (!el) return
    stickToBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40
  }

  return (
    <ResizablePanelGroup orientation="horizontal" id="ai-view" className="h-full bg-surface-deep">
      <ResizablePanel id="ai-sidebar" defaultSize="22%" minSize="15%" maxSize="40%" collapsible collapsedSize={0} panelRef={aiSidebarPanelRef}>
        <div className="h-full overflow-hidden">
          <AiSidebar sessions={sessions} selection={selection} onSelect={setSelection} onDeleteSession={handleDeleteSession} onDeleteRequest={handleDeleteRequest} />
        </div>
      </ResizablePanel>

      <ResizableHandle withHandle />

      <ResizablePanel id="ai-main" defaultSize="78%" minSize="60%">
        {selection && selectedSession ? (
          <div className="flex h-full gap-2 p-4">
            <div ref={scrollRef} onScroll={handleScroll} className="flex-1 overflow-y-auto space-y-3">
              {rendered.length === 0 ? (
                <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                  {t('aiView.waiting', '等待归一化数据…')}
                </div>
              ) : (
                rendered.map(({ turn, requestId }, i) => {
                  const idx = reqIndex.get(requestId)
                  const showLabel = !selection.requestId && idx != null
                  const isLast = i === rendered.length - 1
                  return (
                    <ConversationBubble
                      key={i}
                      turn={turn}
                      isStreaming={turn.role === 'assistant' && isLast && isStreamingReq(requestId)}
                      reqLabel={showLabel ? `#${idx}` : undefined}
                      onJump={onJumpToProxy ? () => onJumpToProxy(requestId) : undefined}
                    />
                  )
                })
              )}
            </div>

            {/* 会话元信息侧栏 */}
            <div className="w-56 flex-shrink-0 border-l border-border/40 pl-3 text-xs text-muted-foreground space-y-2">
              <div>
                <span className="font-semibold text-foreground/70">Session</span><br />
                <span className="break-all">{selectedSession.scopeHost || '—'}</span>
              </div>
              <div title={selectedSession.matchReason}>
                <span className="font-semibold text-foreground/70">分组依据</span><br />
                {selectedSession.matchReason || '—'}
              </div>
              <div>
                <span className="font-semibold text-foreground/70">轮次</span><br />
                {selectedSession.requestIds.length} 次请求
              </div>
              <div>
                <span className="font-semibold text-foreground/70">Token（累加）</span><br />
                Prompt: {selectedSession.usageTotal.promptTokens ?? '-'}<br />
                Completion: {selectedSession.usageTotal.completionTokens ?? '-'}<br />
                Total: {selectedSession.usageTotal.totalTokens ?? '-'}
                {selectedSession.usageTotal.cachedTokens != null && (
                  <><br />Cached: {selectedSession.usageTotal.cachedTokens}</>
                )}
              </div>
            </div>
          </div>
        ) : (
          <div className="flex h-full flex-col items-center justify-center gap-3 bg-surface-deep text-muted-foreground">
            <SparklesIcon className="size-12 text-muted-foreground/30" />
            <p className="text-sm font-medium">{t('view.aiComingSoon', '选中 AI 会话查看对话预览')}</p>
          </div>
        )}
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}
