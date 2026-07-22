import type { AiConversation, AiUsage, TimelineEntry } from '@/types/ai'

export interface RequestEvent {
  id: number
  method: string
  uri: string
  timestamp: number
  headers: Record<string, string>
  query_params?: Record<string, string>
  decrypted: boolean
  content_type?: string
}

export interface ResponseEvent {
  id: number
  status: number
  timestamp: number
  duration_ms: number
  headers: Record<string, string>
  content_type?: string
}

export interface ResponseChunkEvent {
  id: number
  chunk: string
}

export interface ErrorEvent {
  id: number
  error: string
}

export interface RequestBodyChunkEvent {
  id: number
  chunk: string
}

export type ProxyEvent =
  | {
      type: 'request'
      id: number
      method: string
      uri: string
      timestamp: number
      headers: Record<string, string>
      decrypted: boolean
      content_type?: string
    }
  | { type: 'request_chunk'; id: number; chunk: string }
  | {
      type: 'response'
      id: number
      status: number
      timestamp: number
      duration_ms: number
      headers: Record<string, string>
      content_type?: string
    }
  | { type: 'response_chunk'; id: number; chunk: string }
  | { type: 'error'; id: number; error: string }
  | {
      type: 'ai_normalized'
      id: number
      session_id: string
      provider: string
      conversation: AiConversation
      streaming: boolean
    }
  | {
      type: 'ai_timeline_delta'
      session_id: string
      entries: TimelineEntry[]
    }
  | {
      type: 'ai_session'
      session_id: string
      scope_host: string
      request_ids: number[]
      usage_total: AiUsage
      match_reason: string
      /** 会话标题：来自首请求响应的 {"title": "..."}，无则缺省 */
      title?: string
      /** 来源归属：规则内 (来源, 合并头) 对的头命中时为对应来源名，无则缺省 */
      source?: string
    }

/** 从 AI 视图跳转到代理视图时下发的指令。nonce 自增确保重复跳同一 id 也能重触发。 */
export interface ProxyJumpTarget {
  id: number
  nonce: number
}

export interface TrafficEntry {
  id: number
  method: string
  uri: string
  requestNumber: number
  requestTimestamp: number
  requestHeaders: Record<string, string>
  requestBody: string | null
  requestQuery?: Record<string, string>
  requestContentType?: string
  status: number | null
  responseTimestamp: number | null
  durationMs: number | null
  responseHeaders: Record<string, string> | null
  /** 响应体 chunks（字符串数组）。非流式为单元素数组。body = chunks.join('') */
  responseChunks: string[]
  responseContentType?: string
  error: string | null
  decrypted?: boolean
}

/** 后端 SessionStore 内存统计（get_backend_memory_stats 返回） */
export interface BackendMemoryStats {
  sessionCount: number
  maxSessions: number
  /** 所有 session 的 timeline 条目总数 */
  timelineEntryCount: number
  /** timeline 中 AiTurn JSON 序列化字节估算 */
  timelineContentBytes: number
  /** id / scope / title / source / match_reason 字符串字节估算 */
  metadataBytes: number
  /** last_fingerprints + request_ids Vec<u64> 堆字节估算 */
  fingerprintBytes: number
  /** HashMap + Vec 堆 + SessionEntry 结构体开销 */
  structBytes: number
  totalEstBytes: number
}
