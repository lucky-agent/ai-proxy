import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { BackendMemoryStats } from '@/types/proxy'

const EMPTY: BackendMemoryStats = {
  sessionCount: 0,
  maxSessions: 0,
  timelineEntryCount: 0,
  timelineContentBytes: 0,
  metadataBytes: 0,
  fingerprintBytes: 0,
  structBytes: 0,
  totalEstBytes: 0,
}

/**
 * Polls `get_backend_memory_stats` on a 30s interval.
 * Also exposes a `refresh()` for event-driven immediate queries.
 */
export function useBackendMemoryStats(): [BackendMemoryStats, () => void] {
  const [stats, setStats] = useState<BackendMemoryStats>(EMPTY)

  const refresh = useCallback(() => {
    invoke<BackendMemoryStats>('get_backend_memory_stats')
      .then(setStats)
      .catch(() => setStats(EMPTY))
  }, [])

  useEffect(() => {
    refresh()
    const id = setInterval(refresh, 30_000)
    return () => clearInterval(id)
  }, [refresh])

  return [stats, refresh]
}
