export interface RequestEvent {
  id: string
  method: string
  uri: string
  timestamp: number
  headers: Record<string, string>
}

export interface ResponseEvent {
  id: string
  status: number
  timestamp: number
  duration_ms: number
  headers: Record<string, string>
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

export interface TrafficEntry {
  id: string
  method: string
  uri: string
  requestTimestamp: number
  requestHeaders: Record<string, string>
  requestBody: string | null
  status: number | null
  responseTimestamp: number | null
  durationMs: number | null
  responseHeaders: Record<string, string> | null
  responseBody: string | null
  error: string | null
  edited?: boolean
}