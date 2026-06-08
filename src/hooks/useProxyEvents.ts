import { useState, useRef, useCallback, useEffect } from 'react'
import { Channel, invoke } from '@tauri-apps/api/core'
import type { ProxyEvent, TrafficEntry } from '@/types/proxy'
const MAX_CHUNKS = 2000
const MAX_BODY_ACCUMULATE = 2 * 1024 * 1024
 export function useProxyEvents() {
   const counterRef = useRef(0)
   const entriesRef = useRef<Map<string, TrafficEntry>>(new Map())
  const forceUpdateRef = useRef(0)
  const [, setTick] = useState(0)
  const triggerUpdate = useCallback(() => {
    forceUpdateRef.current += 1
    setTick(forceUpdateRef.current)
  }, [])
  useEffect(() => {
    const channel = new Channel<ProxyEvent>()
    channel.onmessage = (event: ProxyEvent) => {
      switch (event.type) {
        case 'request': {
         const { id, method, uri, timestamp, headers } = event
         const query_params = 'query_params' in event ? (event as any).query_params : undefined
          counterRef.current += 1
          entriesRef.current.set(id, {
            id,
            method,
            uri,
            requestNumber: counterRef.current,
            requestTimestamp: timestamp,
            requestHeaders: headers,
            requestBody: null,
            requestQuery: query_params,
            status: null,
            responseTimestamp: null,
            durationMs: null,
            responseHeaders: null,
            responseBody: '',
            responseChunks: [],
            error: null,
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
      }
    }
    invoke('subscribe_proxy_events', { channel })
    return () => {
      // Channel is dropped when component unmounts;
      // backend channel.send() returns Err and the pipeline stops.
    }
  }, [triggerUpdate])
  const entries = Array.from(entriesRef.current.values())
  const clear = useCallback(() => {
    entriesRef.current.clear()
    counterRef.current = 0
    triggerUpdate()
  }, [triggerUpdate])
  return { entries, clear }
}
