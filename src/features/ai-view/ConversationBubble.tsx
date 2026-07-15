import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ChevronRight, ChevronDown, WrenchIcon, FileTextIcon, ExternalLinkIcon, CopyIcon, CheckIcon } from 'lucide-react'
import type { AiTurn, AiContentBlock } from '@/types/ai'

/** 把单个 content block 拼成可复制的纯文本 */
function blockToText(block: AiContentBlock): string {
  if (block.type === 'text') return block.text
  if (block.type === 'tool_use') {
    let input = ''
    try { input = JSON.stringify(block.input, null, 2) } catch { input = String(block.input) }
    return `[tool_use] ${block.name}\n${input}`
  }
  if (block.type === 'tool_result') {
    return `[tool_result]\n${block.content.map(blockToText).join('\n')}`
  }
  return ''
}

/** 把整个 turn 拼成可复制的纯文本（tools_def 输出美化后的 JSON） */
function turnToText(turn: AiTurn): string {
  if (turn.role === 'tools_def') {
    const raw = turn.content[0]?.type === 'text' ? turn.content[0].text : ''
    try { return JSON.stringify(JSON.parse(raw), null, 2) } catch { return raw }
  }
  return turn.content.map(blockToText).join('\n')
}

interface ConversationBubbleProps {
  turn: AiTurn
  isStreaming: boolean
  /** 可选：标注该轮来自哪次请求（合并时间线用，如 "#2"） */
  reqLabel?: string
  /** 可选：点击跳转到代理视图定位该请求 */
  onJump?: () => void
}

/** 单个 content block 的渲染 */
function ContentBlock({ block }: { block: AiContentBlock }) {
  const [expanded, setExpanded] = useState(false)

  if (block.type === 'text') {
    return <span>{block.text}</span>
  }

  if (block.type === 'tool_use') {
    return (
      <div className="mt-1.5 rounded-lg border border-amber-500/20 bg-amber-500/5 overflow-hidden">
        <button
          className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-left text-[11px] font-medium text-amber-600 dark:text-amber-400 hover:bg-amber-500/10 transition-colors"
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? <ChevronDown className="size-3 flex-shrink-0" /> : <ChevronRight className="size-3 flex-shrink-0" />}
          <WrenchIcon className="size-3 flex-shrink-0" />
          <span className="truncate">{block.name}</span>
        </button>
        {expanded && (
          <pre className="max-h-48 overflow-y-auto border-t border-amber-500/15 px-3 py-2 text-[11px] font-mono text-foreground/80 whitespace-pre-wrap break-all">
            {JSON.stringify(block.input, null, 2)}
          </pre>
        )}
      </div>
    )
  }

  if (block.type === 'tool_result') {
    return (
      <div className="mt-1.5 rounded-lg border border-emerald-500/20 bg-emerald-500/5 overflow-hidden">
        <button
          className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-left text-[11px] font-medium text-emerald-600 dark:text-emerald-400 hover:bg-emerald-500/10 transition-colors"
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? <ChevronDown className="size-3 flex-shrink-0" /> : <ChevronRight className="size-3 flex-shrink-0" />}
          <FileTextIcon className="size-3 flex-shrink-0" />
          <span className="truncate">tool result</span>
        </button>
        {expanded && (
          <div className="max-h-48 overflow-y-auto border-t border-emerald-500/15 px-3 py-2 text-xs">
            {block.content.map((innerBlock, i) => (
              <ContentBlock key={i} block={innerBlock} />
            ))}
          </div>
        )}
      </div>
    )
  }

  return null
}

/** 对话气泡：role 决定对齐与配色，内部渲染 content blocks */
export function ConversationBubble({ turn, isStreaming, reqLabel, onJump }: ConversationBubbleProps) {
  const { t } = useTranslation()
  const [toolsExpanded, setToolsExpanded] = useState(false)
  const [systemExpanded, setSystemExpanded] = useState(false)
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(turnToText(turn))
      setCopied(true)
      setTimeout(() => setCopied(false), 1200)
    } catch {
      /* 剪贴板不可用时静默忽略 */
    }
  }

  // role-based style
  const isUser = turn.role === 'user'
  const isSystem = turn.role === 'system'
  const isToolsDef = turn.role === 'tools_def'
  const isTool = turn.role === 'tool'

  // 除 assistant（模型响应）外，system / tools_def / tool 都是客户端随请求发出的，统一靠右
  const alignClass = turn.role === 'assistant' ? 'justify-start' : 'justify-end'

  let bubbleClass: string
  if (isUser) {
    bubbleClass = 'bg-blue-500 text-white'
  } else if (isSystem) {
    bubbleClass = 'bg-surface-base/50 text-muted-foreground text-xs'
  } else if (isToolsDef) {
    bubbleClass = 'bg-violet-500/5 border border-violet-500/15 text-foreground'
  } else if (isTool) {
    bubbleClass = 'bg-emerald-500/5 border border-emerald-500/15 text-foreground'
  } else {
    bubbleClass = 'bg-surface-base text-foreground'
  }

  /** 获取 system turn 纯文本（用于折叠标题） */
  const systemPreview = isSystem
    ? turn.content[0]?.type === 'text'
      ? turn.content[0].text.slice(0, 60) + (turn.content[0].text.length > 60 ? '...' : '')
      : 'system'
    : ''

  return (
    <div className={`group flex ${alignClass}`}>
      <div className={`relative max-w-[80%] rounded-xl px-4 py-2.5 text-sm ${bubbleClass}`}>
        <CopyButton copied={copied} onCopy={handleCopy} isUser={isUser} />
        {(reqLabel || onJump) && (
          <div className={`mb-1 flex items-center gap-1 text-[9px] font-mono ${isUser ? 'text-white/60' : 'text-muted-foreground/50'}`}>
            {reqLabel && <span>{reqLabel}</span>}
            {onJump && (
              <button
                type="button"
                title={t('aiView.jumpToProxy', '在代理中查看')}
                onClick={(e) => { e.stopPropagation(); onJump() }}
                className="inline-flex items-center opacity-60 hover:opacity-100 transition-opacity cursor-pointer"
              >
                <ExternalLinkIcon className="size-3" />
              </button>
            )}
          </div>
        )}
        {/* system 特殊渲染：可折叠 */}
        {isSystem ? (
          <div>
            <button
              className="flex w-full items-center gap-1.5 text-left text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors"
              onClick={() => setSystemExpanded(!systemExpanded)}
            >
              {systemExpanded ? <ChevronDown className="size-3 flex-shrink-0" /> : <ChevronRight className="size-3 flex-shrink-0" />}
              <span className="italic">💬 {systemPreview}</span>
            </button>
            {systemExpanded && (
              <div className="mt-1.5 border-t border-border/30 pt-1.5 text-xs text-muted-foreground whitespace-pre-wrap break-words">
                {turn.content.map((block, j) => (
                  block.type === 'text' ? <span key={j}>{block.text}</span> : <ContentBlock key={j} block={block} />
                ))}
              </div>
            )}
          </div>
        ) : isToolsDef ? (
          /* tools_def 特殊渲染：可折叠的工具列表 */
          <div>
            {(() => {
              const toolsText = turn.content[0]?.type === 'text' ? turn.content[0].text : ''
              let toolCount = 0
              let parsed: any[] | null = null
              if (toolsText) {
                try { parsed = JSON.parse(toolsText); toolCount = Array.isArray(parsed) ? parsed.length : 0 } catch {}
              }
              return (
                <button
                  className="flex w-full items-center gap-1.5 text-left text-[11px] font-medium text-violet-600 dark:text-violet-400 hover:text-violet-700 transition-colors"
                  onClick={() => setToolsExpanded(!toolsExpanded)}
                >
                  {toolsExpanded ? <ChevronDown className="size-3 flex-shrink-0" /> : <ChevronRight className="size-3 flex-shrink-0" />}
                  <span>🔩 Tools{toolCount > 0 ? ` (${toolCount})` : ''}</span>
                </button>
              )
            })()}
            {toolsExpanded && (() => {
              const toolsText = turn.content[0]?.type === 'text' ? turn.content[0].text : ''
              let formatted = toolsText
              try { formatted = JSON.stringify(JSON.parse(toolsText), null, 2) } catch {}
              return (
                <pre className="mt-1.5 max-h-64 overflow-y-auto border-t border-violet-500/15 pt-1.5 text-[10px] font-mono text-foreground/70 whitespace-pre-wrap break-all">
                  {formatted}
                </pre>
              )
            })()}
          </div>
        ) : (
          <>
            {turn.content.map((block, j) => (
              <ContentBlock key={j} block={block} />
            ))}
          </>
        )}
        {isStreaming && turn.role === 'assistant' && (
          <span className="animate-pulse">▌</span>
        )}
      </div>
    </div>
  )
}

/** 气泡内右上角的复制按钮：默认淡出，hover 气泡时显示，复制后短暂变对勾 */
function CopyButton({ copied, onCopy, isUser }: { copied: boolean; onCopy: () => void; isUser?: boolean }) {
  return (
    <button
      type="button"
      onClick={(e) => { e.stopPropagation(); onCopy() }}
      className={`absolute top-1.5 right-1.5 z-10 rounded-md p-1 opacity-0 group-hover:opacity-70 hover:!opacity-100 transition-opacity cursor-pointer ${
        isUser
          ? 'text-white/80 hover:bg-white/15'
          : 'text-muted-foreground bg-surface-base/40 hover:bg-surface-base/80'
      }`}
    >
      {copied ? <CheckIcon className="size-3.5 text-emerald-500" /> : <CopyIcon className="size-3.5" />}
    </button>
  )
}
