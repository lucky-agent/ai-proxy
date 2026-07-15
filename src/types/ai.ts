/** AI 对话统一中间表示（IR），镜像后端 `src-tauri/src/proxy/ai/normalize.rs`。
 *  前端不再本地解析，仅消费后端 `AiNormalized` / `AiSession` 事件后展示。 */

/** 内容块；text / tool_use / tool_result */
export type AiContentBlock =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: unknown }
  | { type: 'tool_result'; tool_use_id: string; content: AiContentBlock[] }

export interface AiTurn {
  role: 'system' | 'user' | 'assistant' | 'tool' | 'tools_def'
  content: AiContentBlock[]
}

export interface AiUsage {
  promptTokens?: number
  completionTokens?: number
  totalTokens?: number
  cachedTokens?: number
}

export interface AiConversation {
  provider: 'openai' | 'anthropic'
  turns: AiTurn[]
  streaming: boolean
  model?: string
  usage?: AiUsage
  finishReason?: string
}

export type AiProvider = 'openai' | 'anthropic'

/** 镜像后端 AiHint 枚举（`proxy.ts` 仍依赖） */
export type AiHint = 'none' | 'candidate' | { provider: string }

export function isAiProvider(s: string): s is AiProvider {
  return s === 'openai' || s === 'anthropic'
}

/** 前端会话状态：由 useAiSessions 从 AiNormalized / AiSession 事件累积。 */
export interface AiSessionState {
  sessionId: string
  scopeHost: string
  /** 组内请求 id，有序 */
  requestIds: string[]
  usageTotal: AiUsage
  turnCount: number
  /** 归组依据：`header:<name>` / `prefix` / `new` / `usage` */
  matchReason: string
  /** 每个请求 id → 该次归一化对话（流式期间被 AiNormalized 不断覆盖更新） */
  conversations: Record<string, AiConversation>
}
