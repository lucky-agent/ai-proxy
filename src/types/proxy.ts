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

export interface ChunkRecord {
  data: string
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
  edited?: boolean
  decrypted?: boolean
}
