/** AI 对话统一中间表示（IR），镜像后端 `src-tauri/src/proxy/ai/normalize.rs`。
 *  前端不再本地解析，仅消费后端 `AiNormalized` / `AiSession` 事件后展示。 */

/** 内容块；text / thinking / tool_use / tool_result */
export type AiContentBlock =
  | { type: 'text'; text: string }
  | { type: 'thinking'; text: string }
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
  /** 缓存命中（cache read）token 数 */
  cachedTokens?: number
  /** 缓存写入（cache creation）token 数 */
  cacheCreationTokens?: number
}

export interface AiConversation {
  provider: 'openai' | 'anthropic'
  turns: AiTurn[]
  streaming: boolean
  model?: string
  usage?: AiUsage
  /** 停止原因，provider 原生值透传（stop / end_turn / STOP 等），可区分正常结束、截断、工具调用 */
  finishReason?: string
  /** 首字用时 ms（请求发出 → 首个流式 chunk），仅流式请求有值 */
  firstChunkMs?: number
  /** 总耗时 ms（请求发出 → 流结束），定稿后有值 */
  durationMs?: number
  /** 请求开始时刻 Unix ms（代理收到请求），气泡时间戳用；每次快照恒有 */
  startMs?: number
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
  /** 会话标题：后端从首请求响应的 {"title": "..."} 提取，无则回退 scopeHost */
  title?: string
  /** 组内请求 id，有序 */
  requestIds: number[]
  usageTotal: AiUsage
  turnCount: number
  /** 归组依据：`header:<name>` / `prefix` / `new` / `usage` */
  matchReason: string
  /** 来源归属（客户端名）：后端按命中的合并头确认，无则缺省 */
  source?: string
  /** 每个请求 id → 该次归一化对话（流式期间被 AiNormalized 不断覆盖更新） */
  conversations: Record<number, AiConversation>
}
