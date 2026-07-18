import { SparklesIcon } from 'lucide-react'
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui/tooltip'
import { useTranslation } from 'react-i18next'
import { useState, useMemo, useEffect, useRef, useCallback, type ReactNode } from 'react'
import { AiSidebar, type AiSelection } from './AiSidebar'
import { ConversationBubble } from './ConversationBubble'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { usePanelRef } from 'react-resizable-panels'
import type { AiConversation, AiSessionState, AiTurn } from '@/types/ai'
import type { DetailPosition } from '@/features/bottom-bar'

/** 会话元信息面板，在右侧/下方布局中复用 */
function MetaPanel({ session }: { session: AiSessionState }) {
  return (
    <div className="text-xs text-muted-foreground space-y-2 min-w-0">
      <div>
        <span className="font-semibold text-foreground/70">Session</span><br />
        <span className="break-all">{session.scopeHost || '—'}</span>
      </div>
      <div>
        <span className="font-semibold text-foreground/70">分组依据</span><br />
        <Tooltip>
          <TooltipTrigger className="break-all text-left">
            <span>{session.matchReason || '—'}</span>
          </TooltipTrigger>
          <TooltipContent side="top" className="max-w-[360px] bg-popover text-popover-foreground text-[11px]">
            {session.matchReason || '—'}
          </TooltipContent>
        </Tooltip>
      </div>
      <div>
        <span className="font-semibold text-foreground/70">轮次</span><br />
        {session.requestIds.length} 次请求
      </div>
      <div>
        <span className="font-semibold text-foreground/70">Token（累加）</span><br />
        Prompt: {session.usageTotal.promptTokens ?? '-'}<br />
        Completion: {session.usageTotal.completionTokens ?? '-'}<br />
        Total: {session.usageTotal.totalTokens ?? '-'}
        {session.usageTotal.cachedTokens != null && (
          <><br />Cached: {session.usageTotal.cachedTokens}</>
        )}
      </div>
    </div>
  )
}

function renderConversation(
  rendered: { turn: AiTurn; requestId: string }[],
  selection: AiSelection,
  reqIndex: Map<string, number>,
  isStreamingReq: (requestId: string) => boolean,
  mdSessions: Record<string, boolean>,
  onJumpToProxy: ((requestId: string) => void) | undefined,
  t: ReturnType<typeof useTranslation>['t'],
): ReactNode {
  if (rendered.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {t('aiView.waiting', '等待归一化数据…')}
      </div>
    )
  }
  return rendered.map(({ turn, requestId }, i) => {
    const idx = reqIndex.get(requestId)
    const showLabel = !selection.requestId && idx != null
    const isLast = i === rendered.length - 1
    return (
      <ConversationBubble
        key={`${selection.sessionId}:${selection.requestId ?? 'all'}:${i}`}
        turn={turn}
        isStreaming={turn.role === 'assistant' && isLast && isStreamingReq(requestId)}
        reqLabel={showLabel ? `#${idx}` : undefined}
        onJump={onJumpToProxy ? () => onJumpToProxy(requestId) : undefined}
        defaultView={mdSessions[selection.sessionId] ? 'md' : 'raw'}
      />
    )
  })
}

interface AiViewProps {
  sessions: AiSessionState[]
  mergedTimeline: (sessionId: string) => { turn: AiTurn; requestId: string }[]
  conversationOf: (requestId: string) => AiConversation | undefined
  showSidebar: boolean
  detailPosition: DetailPosition
  /** 点击气泡的跳转钮 → 切到代理视图并定位该请求。 */
  onJumpToProxy?: (requestId: string) => void
  /** 右键删除整个会话（仅前端移除） */
  onDeleteSession: (sessionId: string) => void
  /** 右键删除会话内单次请求（仅前端移除） */
  onDeleteRequest: (sessionId: string, requestId: string) => void
}

export function AiView({ sessions, mergedTimeline, conversationOf, showSidebar, detailPosition, onJumpToProxy, onDeleteSession, onDeleteRequest }: AiViewProps) {
  const { t } = useTranslation()
  const [selection, setSelection] = useState<AiSelection | null>(null)

  // 会话级 md 渲染开关：sessionId → 是否 md 渲染（默认 raw，不持久化）
  const [mdSessions, setMdSessions] = useState<Record<string, boolean>>({})
  const toggleMdSession = useCallback((sessionId: string) => {
    setMdSessions((prev) => ({ ...prev, [sessionId]: !prev[sessionId] }))
  }, [])

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
          <AiSidebar sessions={sessions} selection={selection} onSelect={setSelection} onDeleteSession={handleDeleteSession} onDeleteRequest={handleDeleteRequest} mdSessions={mdSessions} onToggleMd={toggleMdSession} />
        </div>
      </ResizablePanel>

      <ResizableHandle withHandle />

      <ResizablePanel id="ai-main" defaultSize="78%" minSize="60%">
        {selection && selectedSession ? (
          <>
            {/* 右侧元信息组件，三种布局复用 */}
            {detailPosition === 'right' && (
              <div className="flex h-full gap-2 p-4">
                <div ref={scrollRef} onScroll={handleScroll} className="flex-1 overflow-y-auto space-y-3">
                  {renderConversation(rendered, selection, reqIndex, isStreamingReq, mdSessions, onJumpToProxy, t)}
                </div>
                <div className="w-56 flex-shrink-0 border-l border-border/40 pl-3">
                  <MetaPanel session={selectedSession} />
                </div>
              </div>
            )}
            {detailPosition === 'bottom' && (
              <ResizablePanelGroup orientation="vertical" id="ai-main-vertical" className="h-full">
                <ResizablePanel id="conversation" defaultSize="65%" minSize="30%">
                  <div ref={scrollRef} onScroll={handleScroll} className="h-full overflow-y-auto space-y-3 p-4">
                    {renderConversation(rendered, selection, reqIndex, isStreamingReq, mdSessions, onJumpToProxy, t)}
                  </div>
                </ResizablePanel>
                <ResizableHandle withHandle />
                <ResizablePanel id="meta" defaultSize="35%" minSize="10%" collapsible collapsedSize={0}>
                  <div className="h-full overflow-y-auto p-4">
                    <MetaPanel session={selectedSession} />
                  </div>
                </ResizablePanel>
              </ResizablePanelGroup>
            )}
            {detailPosition === 'hidden' && (
              <div ref={scrollRef} onScroll={handleScroll} className="h-full overflow-y-auto space-y-3 p-4">
                {renderConversation(rendered, selection, reqIndex, isStreamingReq, mdSessions, onJumpToProxy, t)}
              </div>
            )}
          </>
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
