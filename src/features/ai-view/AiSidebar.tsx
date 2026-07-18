import { useState } from 'react'
import { MessageSquareIcon, ChevronRight, ChevronDown, Trash2Icon, CodeIcon, TextIcon } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui/tooltip'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { AiSessionState } from '@/types/ai'
import { cn } from '@/lib/utils'

/** 选中项：仅 sessionId = 选中会话头（合并时间线）；带 requestId = 选中单次请求 */
export interface AiSelection {
  sessionId: string
  requestId?: string
}

interface AiSidebarProps {
  sessions: AiSessionState[]
  selection: AiSelection | null
  onSelect: (sel: AiSelection) => void
  onDeleteSession: (sessionId: string) => void
  onDeleteRequest: (sessionId: string, requestId: string) => void
  /** sessionId → 是否 md 渲染 */
  mdSessions: Record<string, boolean>
  onToggleMd: (sessionId: string) => void
}

/** 归组依据 → 短标签 */
function reasonLabel(reason: string): string {
  if (reason.startsWith('header:')) return 'header'
  if (reason === 'prefix') return 'prefix'
  if (reason === 'new') return 'new'
  return reason || '—'
}

function SessionGroup({
  session,
  selection,
  onSelect,
  onDeleteSession,
  onDeleteRequest,
  mdSessions,
  onToggleMd,
}: {
  session: AiSessionState
  selection: AiSelection | null
  onSelect: (sel: AiSelection) => void
  onDeleteSession: (sessionId: string) => void
  onDeleteRequest: (sessionId: string, requestId: string) => void
  mdSessions: Record<string, boolean>
  onToggleMd: (sessionId: string) => void
}) {
  const { t } = useTranslation()
  const [expanded, setExpanded] = useState(true)
  const headSelected = selection?.sessionId === session.sessionId && !selection?.requestId
  const total = session.usageTotal.totalTokens

  return (
    <div className="border-b border-border/40">
      {/* 会话头 */}
      <ContextMenu>
        <ContextMenuTrigger>
          <button
            className={cn(
              'w-full text-left px-3 py-2.5 transition-colors',
              headSelected ? 'bg-accent/40' : 'hover:bg-surface-base/50',
            )}
            onClick={() => onSelect({ sessionId: session.sessionId })}
          >
            <div className="flex items-center gap-1.5 mb-0.5">
              <span
                className="flex items-center"
                onClick={(e) => {
                  e.stopPropagation()
                  setExpanded((v) => !v)
                }}
              >
                {expanded ? <ChevronDown className="size-3 flex-shrink-0" /> : <ChevronRight className="size-3 flex-shrink-0" />}
              </span>
              <span className="text-[9px] font-semibold px-1.5 py-0.5 rounded border bg-violet-500/10 text-violet-400 border-violet-500/20">
                {reasonLabel(session.matchReason)}
              </span>
              {session.source && (
                <Tooltip>
                  <TooltipTrigger className="inline-flex">
                    <span className="max-w-24 truncate text-[9px] px-1.5 py-0.5 rounded border bg-sky-500/10 text-sky-500 border-sky-500/20 dark:text-sky-400">
                      {session.source}
                    </span>
                  </TooltipTrigger>
                  <TooltipContent side="top" className="max-w-[320px] bg-popover text-popover-foreground text-[11px]">
                    {session.source}
                  </TooltipContent>
                </Tooltip>
              )}
              <span className="text-[10px] text-muted-foreground/60">{session.requestIds.length} 轮</span>
              <span
                onClick={(e) => {
                  e.stopPropagation()
                  onToggleMd(session.sessionId)
                }}
                className={cn(
                  'ml-auto rounded p-0.5 transition-colors hover:bg-surface-base cursor-pointer',
                  mdSessions[session.sessionId] ? 'text-violet-400' : 'text-muted-foreground/40 hover:text-muted-foreground',
                )}
              >
                {mdSessions[session.sessionId] ? <CodeIcon className="size-3" /> : <TextIcon className="size-3" />}
              </span>
            </div>
            <p className="text-[11px] text-foreground/80 truncate leading-tight">
              {session.title || session.scopeHost || session.sessionId}
            </p>
            <p className="text-[10px] text-muted-foreground/50 mt-0.5">
              {total != null ? `Σ ${total} tokens` : '— tokens'}
            </p>
          </button>
        </ContextMenuTrigger>
        <ContextMenuContent className="text-xs min-w-36">
          <ContextMenuItem variant="destructive" onClick={() => onDeleteSession(session.sessionId)}>
            <Trash2Icon className="size-3.5" />
            <span>{t('aiSidebar.deleteSession', '删除会话')}</span>
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>

      {/* 展开：组内请求 */}
      {expanded && (
        <div className="bg-surface-deep/40">
          {session.requestIds.map((rid, i) => {
            const sel = selection?.sessionId === session.sessionId && selection?.requestId === rid
            return (
              <ContextMenu key={rid}>
                <ContextMenuTrigger>
                  <button
                    className={cn(
                      'w-full text-left pl-8 pr-3 py-1.5 text-[10px] font-mono transition-colors',
                      sel ? 'bg-accent/40 text-foreground' : 'text-muted-foreground hover:bg-surface-base/40',
                    )}
                    onClick={() => onSelect({ sessionId: session.sessionId, requestId: rid })}
                  >
                    #{i + 1}
                  </button>
                </ContextMenuTrigger>
                <ContextMenuContent className="text-xs min-w-36">
                  <ContextMenuItem variant="destructive" onClick={() => onDeleteRequest(session.sessionId, rid)}>
                    <Trash2Icon className="size-3.5" />
                    <span>{t('aiSidebar.deleteRequest', '删除该请求')}</span>
                  </ContextMenuItem>
                </ContextMenuContent>
              </ContextMenu>
            )
          })}
        </div>
      )}
    </div>
  )
}

export function AiSidebar({ sessions, selection, onSelect, onDeleteSession, onDeleteRequest, mdSessions, onToggleMd }: AiSidebarProps) {
  const { t } = useTranslation()

  return (
    <div className="flex h-full flex-col bg-surface-base/30">
      <div className="flex items-center px-3 py-2">
        <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
          {t('aiSidebar.title', 'AI 会话')}
        </span>
        <span className="ml-auto text-[10px] text-muted-foreground/50">{sessions.length}</span>
      </div>

      <Separator />

      <ScrollArea className="flex-1 min-h-0">
        {sessions.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-2 py-8 px-4 text-muted-foreground">
            <MessageSquareIcon className="size-8 text-muted-foreground/25" />
            <p className="text-xs">{t('aiSidebar.empty', '暂无 AI 会话')}</p>
          </div>
        ) : (
          <div className="flex flex-col">
            {sessions.map((s) => (
              <SessionGroup
                key={s.sessionId}
                session={s}
                selection={selection}
                onSelect={onSelect}
                onDeleteSession={onDeleteSession}
                onDeleteRequest={onDeleteRequest}
                mdSessions={mdSessions}
                onToggleMd={onToggleMd}
              />
            ))}
          </div>
        )}
      </ScrollArea>
    </div>
  )
}
