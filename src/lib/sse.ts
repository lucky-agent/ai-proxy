export interface SseEvent {
  event: string
  id?: string
  retry?: number
  data: string
}

export function parseSse(body: string): SseEvent[] {
  const normalized = body.replace(/\r\n/g, '\n').replace(/\r/g, '\n')
  const lines = normalized.split('\n')
  const events: SseEvent[] = []
  let current: Partial<SseEvent> = {}

  for (const line of lines) {
    if (line === '') {
      if (current.data !== undefined) {
        events.push({
          event: current.event || 'message',
          id: current.id,
          retry: current.retry,
          data: current.data.replace(/\n$/, ''),
        })
      }
      current = {}
      continue
    }

    if (line.startsWith(':')) continue

    const colonIdx = line.indexOf(':')
    if (colonIdx === -1) {
      if (line.trim() === 'data') {
        current.data = (current.data || '') + '\n'
      }
      continue
    }

    const fieldName = line.slice(0, colonIdx)
    const fieldValue = line.slice(colonIdx + 1)
    const value = fieldValue.startsWith(' ') ? fieldValue.slice(1) : fieldValue

    switch (fieldName) {
      case 'event':
        current.event = value
        break
      case 'data':
        current.data = (current.data || '') + value + '\n'
        break
      case 'id':
        current.id = value
        break
      case 'retry':
        current.retry = parseInt(value, 10)
        break
    }
  }

  if (current.data !== undefined) {
    events.push({
      event: current.event || 'message',
      id: current.id,
      retry: current.retry,
      data: current.data.replace(/\n$/, ''),
    })
  }

  return events
}

export function isStreamingContentType(headers: Record<string, string> | null | undefined): boolean {
  if (!headers) return false
  for (const [k, v] of Object.entries(headers)) {
    if (k.toLowerCase() === 'content-type' && v.toLowerCase().includes('text/event-stream')) {
      return true
    }
  }
  return false
}

// ---------------------------------------------------------------------------
// 合并消息 — 将多个 SSE 事件聚合成一条完整内容
// ---------------------------------------------------------------------------

export type MergeFormat = 'openai'

export interface MergeResult {
  /** 合并后的纯文本内容 */
  content: string
  /** 可用于展示的格式化内容（如 JSON） */
  formatted: string
  /** 合并涉及的 SSE 事件总数 */
  eventCount: number
}

// ---------------------------------------------------------------------------
// Token 用量 — 从响应中解析
// ---------------------------------------------------------------------------

export interface TokenUsage {
  /** 提示 tokens */
  promptTokens?: number
  /** 补全 tokens */
  completionTokens?: number
  /** 总 tokens */
  totalTokens?: number
  /** 缓存命中 tokens（prompt_tokens_details.cached_tokens） */
  cachedTokens?: number
}

/**
 * 从 SSE 事件列表中提取 token 用量信息。
 * OpenAI 的最后一个 chunk 通常包含 usage 字段：
 *   {"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150,"prompt_tokens_details":{"cached_tokens":30}}}
 * 遍历 events 从后向前查找带 usage 的 chunk。
 */
export function extractTokenUsage(events: SseEvent[]): TokenUsage | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const data = events[i].data.trim()
    if (data === '[DONE]') continue
    try {
      const parsed = JSON.parse(data)
      if (parsed && typeof parsed === 'object' && parsed.usage) {
        const usage = parsed.usage
        const details = usage.prompt_tokens_details
        return {
          promptTokens: usage.prompt_tokens,
          completionTokens: usage.completion_tokens,
          totalTokens: usage.total_tokens,
          cachedTokens: details?.cached_tokens ?? undefined,
        }
      }
    } catch {
      continue
    }
  }
  return null
}

/**
 * 尝试解析一条 SSE event.data 为 OpenAI chat completion chunk，
 * 返回 delta.content；解析失败返回 null。
 *
 * OpenAI SSE chunk 示例：
 *   {"id":"...","object":"chat.completion.chunk","choices":[{"delta":{"content":"Hello"},"index":0}]}
 *   {"id":"...","object":"chat.completion.chunk","choices":[{"delta":{},"index":0}]}
 *   {"id":"...","object":"chat.completion.chunk","choices":[{"delta":{},"index":0,"finish_reason":"stop"}]}
 */
function extractOpenAIContent(data: string): string | null {
  // 跳过 [DONE] 标记
  if (data.trim() === '[DONE]') return null

  try {
    const parsed = JSON.parse(data)
    if (!parsed || typeof parsed !== 'object') return null

    const choices = parsed.choices
    if (!Array.isArray(choices) || choices.length === 0) return null

    const delta = choices[0]?.delta
    if (!delta || typeof delta !== 'object') return null

    const content = delta.content
    if (typeof content !== 'string') return null

    return content
  } catch {
    return null
  }
}

/**
 * 将 SSE 事件列表按指定格式合并为一条消息。
 *
 * @param events  SSE 事件列表
 * @param format  目标格式
 * @returns       合并结果，events 为空时返回 null
 */
export function mergeSseMessages(
  events: SseEvent[],
  format: MergeFormat,
): MergeResult | null {
  if (events.length === 0) return null

  if (format === 'openai') {
    const parts: string[] = []
    for (const evt of events) {
      const content = extractOpenAIContent(evt.data)
      if (content !== null) parts.push(content)
    }
    const content = parts.join('')
    return { content, formatted: content, eventCount: events.length }
  }

  return null
}

/**
 * 粗略估算文本的 token 数（OpenAI cl100k_base 近似）。
 * CJK 字符约 1 token / 1.5 字符，ASCII 约 1 token / 4 字符。
 * 仅用于展示参考，非精确计数。
 */
export function estimateTokens(text: string): number {
  let cjkCount = 0
  let asciiCount = 0
  for (const char of text) {
    const code = char.charCodeAt(0)
    if (
      (code >= 0x4E00 && code <= 0x9FFF) || // CJK Unified Ideographs
      (code >= 0x3040 && code <= 0x30FF) || // Hiragana/Katakana
      (code >= 0xAC00 && code <= 0xD7AF)    // Hangul
    ) {
      cjkCount++
    } else {
      asciiCount++
    }
  }
  return Math.max(1, Math.ceil(cjkCount / 1.5 + asciiCount / 4))
}
