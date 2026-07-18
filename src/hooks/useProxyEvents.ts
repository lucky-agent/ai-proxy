import { useState, useRef, useCallback, useEffect } from 'react'
import { Channel, invoke } from '@tauri-apps/api/core'
import type { ProxyEvent, TrafficEntry } from '@/types/proxy'
import { publishAiEvent } from './aiEventBus'

const MAX_CHUNKS = 2000
const MAX_BODY_ACCUMULATE = 2 * 1024 * 1024
/** entries 窗口上限：超出后 FIFO 删除最旧条目 */
const MAX_ENTRIES = 5000
/** 到达此阈值时对最早一批条目做瘦身（清 body 降水印，保留元信息在列表可见） */
const SLIM_AT = 4000
/** 每次瘦身处理的条目数 */
const SLIM_BATCH = 2000

export function useProxyEvents() {
  const counterRef = useRef(0)
  const entriesRef = useRef<Map<number, TrafficEntry>>(new Map())
  /** FIFO 淘汰：记录插入顺序，用于定位最旧条目 */
  const insertionOrderRef = useRef<number[]>([])
  const forceUpdateRef = useRef(0)
  const [, setTick] = useState(0)
  const triggerUpdate = useCallback(() => {
    forceUpdateRef.current += 1
    setTick(forceUpdateRef.current)
  }, [])

  /** 瘦身最早 SLIM_BATCH 条条目：清空 body/chunks/requestBody 释放内存，元信息保留。 */
  const slimOldest = useCallback(() => {
    const toSlim = insertionOrderRef.current.slice(0, SLIM_BATCH)
    for (const id of toSlim) {
      const entry = entriesRef.current.get(id)
      if (entry) {
        entry.responseBody = ''
        entry.responseChunks = []
        entry.requestBody = null
      }
    }
  }, [])

  useEffect(() => {
    const channel = new Channel<ProxyEvent>()
    channel.onmessage = (event: ProxyEvent) => {
      switch (event.type) {
        case 'request': {
          const { id, method, uri, timestamp, headers, decrypted } = event
          console.log('[ProxyEvent] request', { id, method, uri, timestamp, decrypted })
          const query_params = 'query_params' in event ? (event as any).query_params : undefined
          counterRef.current += 1

          // FIFO 淘汰：超出上限时删最旧条目
          if (entriesRef.current.size >= MAX_ENTRIES) {
            const oldest = insertionOrderRef.current.shift()
            if (oldest !== undefined) entriesRef.current.delete(oldest)
          }
          // 瘦身：达到 SLIM_AT 时清空最早一批的 body 释放内存
          if (entriesRef.current.size >= SLIM_AT) {
            slimOldest()
          }
          insertionOrderRef.current.push(id)

          entriesRef.current.set(id, {
            id,
            method,
            uri,
            requestNumber: counterRef.current,
            requestTimestamp: timestamp,
            requestHeaders: headers,
            requestBody: null,
            requestQuery: query_params,
            decrypted,
            requestContentType: event.content_type,
            requestContentLength: event.content_length,
            status: null,
            responseTimestamp: null,
            durationMs: null,
            responseHeaders: null,
            responseBody: '',
            responseChunks: [],
            responseContentType: undefined,
            responseContentLength: undefined,
            error: null,
            aiHint: event.ai_hint ?? 'none',
          })
          triggerUpdate()
          break
        }
        case 'request_chunk': {
          const { id, chunk } = event
          const entry = entriesRef.current.get(id)
          if (entry) {
            if (entry.requestBody === null) entry.requestBody = ''
            if (entry.requestBody.length < MAX_BODY_ACCUMULATE) {
              entry.requestBody += chunk
              entriesRef.current.set(id, entry)
            }
          }
          triggerUpdate()
          break
        }
        case 'response': {
          const { id, status, timestamp, duration_ms, headers } = event
          const entry = entriesRef.current.get(id)
          if (entry) {
            entry.status = status
            entry.responseTimestamp = timestamp
            entry.durationMs = duration_ms
            entry.responseHeaders = headers
            entry.responseContentType = event.content_type
            entry.responseContentLength = event.content_length
            entriesRef.current.set(id, entry)
          }
          triggerUpdate()
          break
        }
        case 'response_chunk': {
          const { id, chunk } = event
          const entry = entriesRef.current.get(id)
          if (entry) {
            if (entry.responseBody !== null && entry.responseBody.length < MAX_BODY_ACCUMULATE) {
              entry.responseBody += chunk
            }
            if (entry.responseChunks.length < MAX_CHUNKS) {
              entry.responseChunks.push({ data: chunk })
            }
            entriesRef.current.set(id, entry)
          }
          triggerUpdate()
          break
        }
        case 'error': {
          const { id, error } = event
          const entry = entriesRef.current.get(id)
          if (entry) {
            entry.error = error
            entriesRef.current.set(id, entry)
          }
          triggerUpdate()
          break
        }
        case 'ai_normalized':
        case 'ai_session': {
          // AI 事件转发到独立总线，由 useAiSessions 消费；不污染 entries。
          publishAiEvent(event)
          break
        }
      }
    }

    invoke('subscribe_proxy_events', { channel })
    return () => {
      // Channel is dropped when component unmounts;
      // backend channel.send() returns Err and the pipeline stops.
    }
  }, [triggerUpdate, slimOldest])

  const entries = Array.from(entriesRef.current.values())

  const clear = useCallback(() => {
    entriesRef.current.clear()
    insertionOrderRef.current = []
    counterRef.current = 0
    triggerUpdate()
  }, [triggerUpdate])

  return { entries, clear }
}
