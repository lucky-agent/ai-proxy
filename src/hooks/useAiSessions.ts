import { useState, useRef, useCallback, useEffect } from 'react'
import type { AiContentBlock, AiConversation, AiSessionState, AiTurn, AiUsage } from '@/types/ai'
import { subscribeAiEvents, type AiEvent } from './aiEventBus'

/** 空 usage */
const EMPTY_USAGE: AiUsage = {}

/** sessions 窗口上限：超出后 LRU 淘汰最久未访问的会话 */
const MAX_SESSIONS = 50

/**
 * 单个 content block 是否相等——利用协议自带的唯一标识做 O(1) 判定：
 * tool_use.id / tool_result.tool_use_id 由服务端生成且全局唯一，客户端回放时
 * 原样带回，id 相同即同一次调用，无需深比较 input/content 大对象。
 */
function sameBlock(a: AiContentBlock, b: AiContentBlock): boolean {
  if (a.type !== b.type) return false
  switch (a.type) {
    case 'text':
      return a.text === (b as Extract<AiContentBlock, { type: 'text' }>).text
    case 'thinking':
      return a.text === (b as Extract<AiContentBlock, { type: 'thinking' }>).text
    case 'tool_use': {
      const bb = b as Extract<AiContentBlock, { type: 'tool_use' }>
      // 极少数解析不到 id 时（空串）退化为 input 比对
      if (!a.id) return a.name === bb.name && JSON.stringify(a.input) === JSON.stringify(bb.input)
      return a.id === bb.id && a.name === bb.name
    }
    case 'tool_result':
      return a.tool_use_id === (b as Extract<AiContentBlock, { type: 'tool_result' }>).tool_use_id
  }
}

/** LCP 比较忽略 thinking：客户端回放历史时可能剥掉思考内容（如 reasoning_content
 *  不回放），计入会让同一 turn 前后形状不同、时间线在该处断链重复。 */
function comparableBlocks(turn: AiTurn): AiContentBlock[] {
  return turn.content.filter((b) => b.type !== 'thinking')
}

/** 两个 turn 是否相等：role + 逐 block 比较（忽略 thinking），任一不匹配即短路。 */
function sameTurn(a: AiTurn, b: AiTurn): boolean {
  if (a === b) return true
  if (a.role !== b.role) return false
  const ca = comparableBlocks(a)
  const cb = comparableBlocks(b)
  if (ca.length !== cb.length) return false
  return ca.every((blk, i) => sameBlock(blk, cb[i]))
}

/** 将 sid 移到 LRU 数组末尾（标记为最近访问）。 */
function touchLru(order: string[], sid: string) {
  const idx = order.indexOf(sid)
  if (idx !== -1) {
    order.splice(idx, 1)
  }
  order.push(sid)
}

/**
 * 消费后端 `ai_normalized` / `ai_session` 事件，维护 `Map<sessionId, AiSessionState>`。
 * 与 useProxyEvents 并列独立，不依赖/污染 TrafficEntry。
 */
export function useAiSessions() {
  const sessionsRef = useRef<Map<string, AiSessionState>>(new Map())
  /** requestId → sessionId，便于 AiNormalized 快速定位所属会话 */
  const reqToSessionRef = useRef<Map<number, string>>(new Map())
  /**
   * requestId → 请求侧 turns 缓存。后端仅在请求侧首发与定稿快照携带
   * request_turns（流式节流增量为空数组，省去后端热路径克隆/序列化），
   * 前端在此缓存首发值供增量快照合并复用。
   */
  const reqTurnsRef = useRef<Map<number, AiTurn[]>>(new Map())
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
        reqTurnsRef.current.delete(rid)
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
          // 后端某次事件缺 title 字段时不闪回旧值
          title: event.title ?? prev?.title,
          requestIds: event.request_ids,
          usageTotal: event.usage_total ?? EMPTY_USAGE,
          turnCount: event.turn_count,
          matchReason: event.match_reason,
          // 来源归属同 title：缺省时保留已确认的值
          source: event.source ?? prev?.source,
          conversations: prev?.conversations ?? {},
        })
        for (const rid of event.request_ids) {
          reqToSessionRef.current.set(rid, event.session_id)
        }
        touchLru(lruOrderRef.current, event.session_id)
        evictIfNeeded()
      } else {
        // ai_normalized：写入该请求的完整对话（请求 turns + 响应 assistant turn）
        const sid = event.session_id
        reqToSessionRef.current.set(event.id, sid)
        let sess = map.get(sid)
        if (!sess) {
          // AiNormalized 先于 AiSession 到达时的占位
          sess = {
            sessionId: sid,
            scopeHost: '',
            requestIds: [event.id],
            usageTotal: EMPTY_USAGE,
            turnCount: 0,
            matchReason: '',
            conversations: {},
          }
          map.set(sid, sess)
        }
        // 合并：请求 messages（历史）+ 响应对话（assistant 回复）为一次完整对话
        if (event.request_turns.length > 0) {
          reqTurnsRef.current.set(event.id, event.request_turns)
        }
        const requestTurns = reqTurnsRef.current.get(event.id) ?? event.request_turns
        const fullConv: AiConversation = {
          ...event.conversation,
          turns: [...requestTurns, ...event.conversation.turns],
        }
        sess.conversations = { ...sess.conversations, [event.id]: fullConv }
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
      reqTurnsRef.current.delete(rid)
    }
    sessionsRef.current.delete(sessionId)
    // 从 LRU 列表中移除
    const lruIdx = lruOrderRef.current.indexOf(sessionId)
    if (lruIdx !== -1) lruOrderRef.current.splice(lruIdx, 1)
    scheduleUpdate()
  }, [scheduleUpdate])

  /** 前端移除会话内单次请求；删空后连同会话一起移除 */
  const removeRequest = useCallback((sessionId: string, requestId: number) => {
    const sess = sessionsRef.current.get(sessionId)
    if (!sess) return
    reqToSessionRef.current.delete(requestId)
    reqTurnsRef.current.delete(requestId)
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
    }
    scheduleUpdate()
  }, [scheduleUpdate])

  /** 某请求的归一化对话 */
  const conversationOf = useCallback((requestId: number): AiConversation | undefined => {
    const sid = reqToSessionRef.current.get(requestId)
    if (!sid) return undefined
    return sessionsRef.current.get(sid)?.conversations[requestId]
  }, [])

  /**
   * 增量前缀合并时间线：按请求先后顺序，每次请求的 turns（历史 + 本次响应）
   * 与已合并结果求最长公共前缀（LCP），只追加超出前缀的部分并标注来源 requestId。
   *
   * 依赖后端前缀分组（session.rs::is_prefix）的不变量：同会话内后一次请求的
   * messages 是前一次的严格扩展，因此新增段天然 = 本次新输入 + 本次响应，
   * 时序与归属都精确，无重复。header 分组下的分叉请求（重试/压缩历史）
   * 会在分叉点之后整段追加，按时间顺序保留各分支。
   */
  const mergedTimeline = useCallback((sessionId: string): { turn: AiTurn; requestId: number }[] => {
    const sess = sessionsRef.current.get(sessionId)
    if (!sess) return []

    const result: { turn: AiTurn; requestId: number }[] = []
    for (const rid of sess.requestIds) {
      const turns = sess.conversations[rid]?.turns
      if (!turns) continue
      // 最长公共前缀：逐 turn 深比较（同后端 is_prefix 的相等口径）
      let p = 0
      const max = Math.min(result.length, turns.length)
      while (p < max && sameTurn(result[p].turn, turns[p])) p++
      for (let i = p; i < turns.length; i++) {
        result.push({ turn: turns[i], requestId: rid })
      }
    }
    return result
  }, [])

  return { sessions, conversationOf, mergedTimeline, removeSession, removeRequest }
}
