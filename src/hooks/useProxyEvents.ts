import { useState, useEffect, useRef, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import type { RequestEvent, ResponseEvent, ResponseChunkEvent, ErrorEvent, TrafficEntry, RequestBodyChunkEvent } from '@/types/proxy'

const MAX_BODY_ACCUMULATE = 2 * 1024 * 1024

export function useProxyEvents() {
  const entriesRef = useRef<Map<string, TrafficEntry>>(new Map())
  const forceUpdateRef = useRef(0)
  const [, setTick] = useState(0)

  const triggerUpdate = useCallback(() => {
    forceUpdateRef.current += 1
    setTick(forceUpdateRef.current)
  }, [])

  useEffect(() => {
    const unsubs: (() => void)[] = []

    listen<RequestEvent>('proxy:request', (event) => {
      const { id, method, uri, timestamp, headers } = event.payload
      entriesRef.current.set(id, {
        id,
        method,
        uri,
        requestTimestamp: timestamp,
        requestHeaders: headers,
        requestBody: null,
        status: null,
        responseTimestamp: null,
        durationMs: null,
        responseHeaders: null,
        responseBody: '',
        error: null,
      })
      triggerUpdate()
    }).then((unsub) => unsubs.push(unsub))

    listen<ResponseEvent>('proxy:response', (event) => {
      const { id, status, timestamp, duration_ms, headers } = event.payload
      const entry = entriesRef.current.get(id)
      if (entry) {
        entry.status = status
        entry.responseTimestamp = timestamp
        entry.durationMs = duration_ms
        entry.responseHeaders = headers
        entriesRef.current.set(id, entry)
      }
      triggerUpdate()
    }).then((unsub) => unsubs.push(unsub))

    listen<ResponseChunkEvent>('proxy:response-chunk', (event) => {
      const { id, chunk } = event.payload
      const entry = entriesRef.current.get(id)
      if (entry && entry.responseBody !== null) {
        if (entry.responseBody.length < MAX_BODY_ACCUMULATE) {
          entry.responseBody += chunk
          entriesRef.current.set(id, entry)
        }
      }
      triggerUpdate()
    }).then((unsub) => unsubs.push(unsub))

    listen<RequestBodyChunkEvent>('proxy:request-chunk', (event) => {
      const { id, chunk } = event.payload
      const entry = entriesRef.current.get(id)
      if (entry) {
        if (entry.requestBody === null) {
          entry.requestBody = ''
        }
        if (entry.requestBody.length < MAX_BODY_ACCUMULATE) {
          entry.requestBody += chunk
          entriesRef.current.set(id, entry)
        }
      }
      triggerUpdate()
    }).then((unsub) => unsubs.push(unsub))

    listen<ErrorEvent>('proxy:error', (event) => {
      const { id, error } = event.payload
      const entry = entriesRef.current.get(id)
      if (entry) {
        entry.error = error
        entriesRef.current.set(id, entry)
      }
      triggerUpdate()
    }).then((unsub) => unsubs.push(unsub))

    return () => {
      unsubs.forEach((unsub) => unsub())
    }
  }, [triggerUpdate])

  const entries = Array.from(entriesRef.current.values())
  const clear = useCallback(() => {
    entriesRef.current.clear()
    triggerUpdate()
  }, [triggerUpdate])

  return { entries, clear }
}