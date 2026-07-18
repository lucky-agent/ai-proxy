import type { AiHint, AiConversation, AiUsage, AiTurn } from '@/types/ai'

export interface RequestEvent {
  id: string
  method: string
  uri: string
  timestamp: number
  headers: Record<string, string>
  query_params?: Record<string, string>
  decrypted: boolean
  content_type?: string
  content_length?: number
  ai_hint?: AiHint
}

export interface ResponseEvent {
  id: string
  status: number
  timestamp: number
  duration_ms: number
  headers: Record<string, string>
  content_type?: string
  content_length?: number
}

export interface ResponseChunkEvent {
  id: string
  chunk: string
}

export interface ErrorEvent {
  id: string
  error: string
}

export interface RequestBodyChunkEvent {
  id: string
  chunk: string
}

export type ProxyEvent =
  | {
      type: 'request'
      id: string
      method: string
      uri: string
      timestamp: number
      headers: Record<string, string>
      decrypted: boolean
      content_type?: string
      content_length?: number
      ai_hint?: AiHint
    }
  | { type: 'request_chunk'; id: string; chunk: string }
  | {
      type: 'response'
      id: string
      status: number
      timestamp: number
      duration_ms: number
      headers: Record<string, string>
      content_type?: string
      content_length?: number
    }
  | { type: 'response_chunk'; id: string; chunk: string }
  | { type: 'error'; id: string; error: string }
  | {
      type: 'ai_normalized'
      id: string
      session_id: string
      provider: string
      request_turns: AiTurn[]
      conversation: AiConversation
      streaming: boolean
    }
  | {
      type: 'ai_session'
      session_id: string
      scope_host: string
      request_ids: string[]
      usage_total: AiUsage
      turn_count: number
      match_reason: string
      /** 会话标题：来自首请求响应的 {"title": "..."}，无则缺省 */
      title?: string
      /** 来源归属：规则内 (来源, 合并头) 对的头命中时为对应来源名，无则缺省 */
      source?: string
    }

export interface ChunkRecord {
  data: string
}

/** 从 AI 视图跳转到代理视图时下发的指令。nonce 自增确保重复跳同一 id 也能重触发。 */
export interface ProxyJumpTarget {
  id: string
  nonce: number
}

export interface TrafficEntry {
  id: string
  method: string
  uri: string
  requestNumber: number
  requestTimestamp: number
  requestHeaders: Record<string, string>
  requestBody: string | null
  requestQuery?: Record<string, string>
  requestContentType?: string
  requestContentLength?: number
  status: number | null
  responseTimestamp: number | null
  durationMs: number | null
  responseHeaders: Record<string, string> | null
  responseBody: string | null
  responseChunks: ChunkRecord[]
  responseContentType?: string
  responseContentLength?: number
  error: string | null
  decrypted?: boolean
  aiHint: AiHint
}
