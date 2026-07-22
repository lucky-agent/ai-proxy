import { useState, useRef, useCallback, useEffect } from 'react'
import { Channel, invoke } from '@tauri-apps/api/core'
import type { ProxyEvent, TrafficEntry } from '@/types/proxy'
import { publishAiEvent } from './aiEventBus'
import { MemAccum, estHeadersSize, estStrBytes } from '@/lib/memoryStats'

const MAX_CHUNKS = 2000
const MAX_BODY_ACCUMULATE = 2 * 1024 * 1024
/** entries 窗口上限：超出后 FIFO 删除最旧条目 */
const MAX_ENTRIES = 5000
/** 到达此阈值时对最早一批条目做瘦身（清 body 降水印，保留元信息在列表可见） */
const SLIM_AT = 500
/** 每次瘦身处理的条目数 */
const SLIM_BATCH = 300

export function useProxyEvents() {
  const counterRef = useRef(0)
  const entriesRef = useRef<Map<number, TrafficEntry>>(new Map())
  /** FIFO 淘汰：记录插入顺序，用于定位最旧条目 */
  const insertionOrderRef = useRef<number[]>([])
  const forceUpdateRef = useRef(0)
  const [, setTick] = useState(0)

  /** rAF 合批：同帧内多个事件只触发一次渲染 */
  const rafRef = useRef<number | null>(null)
  const scheduleUpdate = useCallback(() => {
    if (rafRef.current !== null) return  // 已有待刷新帧
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = null
      forceUpdateRef.current += 1
      setTick(forceUpdateRef.current)
    })
  }, [])

  /** 立即触发（clear 等用户操作需要即时反馈） */
  const triggerUpdate = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current)
      rafRef.current = null
    }
    forceUpdateRef.current += 1
    setTick(forceUpdateRef.current)
  }, [])

  /** 增量内存追踪：每个事件处理时加减，零遍历。 */
  const accumRef = useRef(new MemAccum())

  /** 从 entry 计算当前追踪的所有开销（用于淘汰/瘦身时反算扣减） */
  function entryTrackedCost(entry: TrafficEntry) {
    const hdrSz = estHeadersSize(entry.requestHeaders) + estHeadersSize(entry.responseHeaders)
    const chunkBytes = entry.responseChunks.reduce((s, c) => s + c.length * 2, 0)
    const bodySz = estStrBytes(entry.requestBody) + chunkBytes
    return { hdrSz, bodySz, chunkCount: entry.responseChunks.length }
  }

  /** 瘦身最早 SLIM_BATCH 条条目：清空 body/chunks/requestBody 释放内存，元信息保留。 */
  const slimOldest = useCallback(() => {
    const toSlim = insertionOrderRef.current.slice(0, SLIM_BATCH)
    const accum = accumRef.current
    for (const id of toSlim) {
      const entry = entriesRef.current.get(id)
      if (entry) {
        const { hdrSz, bodySz, chunkCount } = entryTrackedCost(entry)
        accum.slimEntry(hdrSz + bodySz, chunkCount)
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
          const query_params = 'query_params' in event ? (event as any).query_params : undefined
          counterRef.current += 1

          // FIFO 淘汰：超出上限时删最旧条目
          if (entriesRef.current.size >= MAX_ENTRIES) {
            const oldest = insertionOrderRef.current.shift()
            if (oldest !== undefined) {
              const dead = entriesRef.current.get(oldest)
              if (dead) {
                const { hdrSz, bodySz, chunkCount } = entryTrackedCost(dead)
                accumRef.current.slimEntry(hdrSz + bodySz, chunkCount)
                accumRef.current.removeEntry(0)
              } else {
                accumRef.current.removeEntry(0)
              }
              entriesRef.current.delete(oldest)
            }
          }
          // 瘦身：达到 SLIM_AT 时清空最早一批的 body 释放内存
          if (entriesRef.current.size >= SLIM_AT) {
            slimOldest()
          }
          insertionOrderRef.current.push(id)

          const hdrSz = estHeadersSize(headers)
          accumRef.current.addEntry(hdrSz)

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
            status: null,
            responseTimestamp: null,
            durationMs: null,
            responseHeaders: null,
            responseChunks: [],
            responseContentType: undefined,
            error: null,
          })
          scheduleUpdate()
          break
        }
        case 'request_chunk': {
          const { id, chunk } = event
          const entry = entriesRef.current.get(id)
          if (entry) {
            if (entry.requestBody === null) entry.requestBody = ''
            if (entry.requestBody.length < MAX_BODY_ACCUMULATE) {
              entry.requestBody += chunk
              accumRef.current.addReqChunk(chunk)
              entriesRef.current.set(id, entry)
            }
          }
          scheduleUpdate()
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
            accumRef.current.addRespHeaders(estHeadersSize(headers))
            entriesRef.current.set(id, entry)
          }
          scheduleUpdate()
          break
        }
        case 'response_chunk': {
          const { id, chunk } = event
          const entry = entriesRef.current.get(id)
          if (entry) {
            // 上限检查：总字节 ≤ MAX_BODY_ACCUMULATE，条数 ≤ MAX_CHUNKS
            const totalBytes = entry.responseChunks.reduce((s, c) => s + c.length, 0)
            if (totalBytes < MAX_BODY_ACCUMULATE && entry.responseChunks.length < MAX_CHUNKS) {
              entry.responseChunks.push(chunk)
              accumRef.current.addRespChunk(chunk)
            }
            entriesRef.current.set(id, entry)
          }
          scheduleUpdate()
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
        case 'ai_timeline_delta':
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
  }, [scheduleUpdate, slimOldest])

  const entries = Array.from(entriesRef.current.values())

  const clear = useCallback(() => {
    entriesRef.current.clear()
    insertionOrderRef.current = []
    counterRef.current = 0
    accumRef.current.clear()
    triggerUpdate()
  }, [triggerUpdate])

  return { entries, clear, accum: accumRef.current }
}
