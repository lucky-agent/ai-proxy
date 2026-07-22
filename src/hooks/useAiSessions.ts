import { useState, useRef, useCallback, useEffect } from 'react'
import type { AiConversation, AiSessionState, AiUsage, TimelineEntry } from '@/types/ai'
import { subscribeAiEvents, type AiEvent } from './aiEventBus'

/** 空 usage */
const EMPTY_USAGE: AiUsage = {}

/** sessions 窗口上限：超出后 LRU 淘汰最久未访问的会话 */
const MAX_SESSIONS = 50

/** 将 sid 移到 LRU 数组末尾（标记为最近访问）。 */
function touchLru(order: string[], sid: string) {
  const idx = order.indexOf(sid)
  if (idx !== -1) {
    order.splice(idx, 1)
  }
  order.push(sid)
}

/**
 * 消费后端 `ai_normalized` / `ai_timeline_delta` / `ai_session` 事件，
 * 维护 `Map<sessionId, AiSessionState>`。
 *
 * 时间线由后端维护：每条 AiTimelineDelta 是去重后的增量，
 * 前端直接 append 到会话的 timeline 数组即可渲染。
 * 单次请求视图仍用 `conversations[requestId]`（纯响应侧数据）。
 */
export function useAiSessions() {
  const sessionsRef = useRef<Map<string, AiSessionState>>(new Map())
  /** requestId → sessionId，便于 AiNormalized 快速定位所属会话 */
  const reqToSessionRef = useRef<Map<number, string>>(new Map())
  /** LRU 淘汰：记录 sessionId 访问顺序，越靠后越新 */
  const lruOrderRef = useRef<string[]>([])
  const [, setTick] = useState(0)

  // rAF 合批：流式高频 AiNormalized 落同一帧刷新
  const rafRef = useRef<number | null>(null)
  const scheduleUpdate = useCallback(() => {
    if (rafRef.current !== null) return
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = null
      setTick((t) => t + 1)
    })
  }, [])

  /** 淘汰最久未访问的会话，同步清理关联索引。 */
  const evictIfNeeded = useCallback(() => {
    if (sessionsRef.current.size <= MAX_SESSIONS) return
    const evictId = lruOrderRef.current.shift()
    if (!evictId) return
    const sess = sessionsRef.current.get(evictId)
    if (sess) {
      for (const rid of sess.requestIds) {
        reqToSessionRef.current.delete(rid)
      }
    }
    sessionsRef.current.delete(evictId)
  }, [])

  useEffect(() => {
    const handle = (event: AiEvent) => {
      const map = sessionsRef.current

      if (event.type === 'ai_session') {
        const prev = map.get(event.session_id)
        map.set(event.session_id, {
          sessionId: event.session_id,
          scopeHost: event.scope_host,
          title: event.title ?? prev?.title,
          requestIds: event.request_ids,
          usageTotal: event.usage_total ?? EMPTY_USAGE,
          matchReason: event.match_reason,
          source: event.source ?? prev?.source,
          conversations: prev?.conversations ?? {},
          timeline: prev?.timeline ?? [],
        })
        for (const rid of event.request_ids) {
          reqToSessionRef.current.set(rid, event.session_id)
        }
        touchLru(lruOrderRef.current, event.session_id)
        evictIfNeeded()
      } else if (event.type === 'ai_timeline_delta') {
        // 时间线增量：直接 append
        let sess = map.get(event.session_id)
        if (!sess) {
          // AiTimelineDelta 先于 AiSession 到达时的占位
          sess = {
            sessionId: event.session_id,
            scopeHost: '',
            requestIds: [],
            usageTotal: EMPTY_USAGE,
            matchReason: '',
            conversations: {},
            timeline: [],
          }
          map.set(event.session_id, sess)
        }
        if (event.entries.length > 0) {
          sess.timeline = [...sess.timeline, ...event.entries]
        }
        touchLru(lruOrderRef.current, event.session_id)
        evictIfNeeded()
      } else {
        // ai_normalized：存储该请求的纯响应侧数据（assistant turns + 元信息）
        const sid = event.session_id
        reqToSessionRef.current.set(event.id, sid)
        let sess = map.get(sid)
        if (!sess) {
          sess = {
            sessionId: sid,
            scopeHost: '',
            requestIds: [event.id],
            usageTotal: EMPTY_USAGE,
            matchReason: '',
            conversations: {},
            timeline: [],
          }
          map.set(sid, sess)
        }
        // 直接存储 conversation（纯响应侧数据，无需拼接 request_turns）
        sess.conversations = { ...sess.conversations, [event.id]: event.conversation }
        if (!sess.requestIds.includes(event.id)) {
          sess.requestIds = [...sess.requestIds, event.id]
        }
        touchLru(lruOrderRef.current, sid)
        evictIfNeeded()
      }
      scheduleUpdate()
    }
    return subscribeAiEvents(handle)
  }, [scheduleUpdate, evictIfNeeded])

  const sessions = Array.from(sessionsRef.current.values())

  /** 前端移除整个会话（不通知后端；同会话后续新流量会让其重新出现） */
  const removeSession = useCallback((sessionId: string) => {
    const sess = sessionsRef.current.get(sessionId)
    if (!sess) return
    for (const rid of sess.requestIds) {
      reqToSessionRef.current.delete(rid)
    }
    sessionsRef.current.delete(sessionId)
    const lruIdx = lruOrderRef.current.indexOf(sessionId)
    if (lruIdx !== -1) lruOrderRef.current.splice(lruIdx, 1)
    scheduleUpdate()
  }, [scheduleUpdate])

  /** 前端移除会话内单次请求；删空后连同会话一起移除 */
  const removeRequest = useCallback((sessionId: string, requestId: number) => {
    const sess = sessionsRef.current.get(sessionId)
    if (!sess) return
    reqToSessionRef.current.delete(requestId)
    const rest = sess.requestIds.filter((rid) => rid !== requestId)
    if (rest.length === 0) {
      sessionsRef.current.delete(sessionId)
      const lruIdx = lruOrderRef.current.indexOf(sessionId)
      if (lruIdx !== -1) lruOrderRef.current.splice(lruIdx, 1)
    } else {
      sess.requestIds = rest
      const conversations = { ...sess.conversations }
      delete conversations[requestId]
      sess.conversations = conversations
      // 移除 timeline 中属于该 requestId 的条目
      sess.timeline = sess.timeline.filter((e) => e.requestId !== requestId)
    }
    scheduleUpdate()
  }, [scheduleUpdate])

  /** 某请求的归一化对话（纯响应侧数据） */
  const conversationOf = useCallback((requestId: number): AiConversation | undefined => {
    const sid = reqToSessionRef.current.get(requestId)
    if (!sid) return undefined
    return sessionsRef.current.get(sid)?.conversations[requestId]
  }, [])

  /**
   * 合并时间线：直接从后端维护的 timeline 返回，无需前端 LCP 去重。
   * 每条 AiTimelineDelta 的 entries 是已去重的增量，前端只需 append。
   */
  const mergedTimeline = useCallback((sessionId: string): TimelineEntry[] => {
    return sessionsRef.current.get(sessionId)?.timeline ?? []
  }, [])

  /** 清空所有 AI 会话（前端状态；不通知后端，同 clear 流量一致） */
  const clearAll = useCallback(() => {
    sessionsRef.current.clear()
    reqToSessionRef.current.clear()
    lruOrderRef.current = []
    scheduleUpdate()
  }, [scheduleUpdate])

  return { sessions, conversationOf, mergedTimeline, removeSession, removeRequest, clearAll }
}
