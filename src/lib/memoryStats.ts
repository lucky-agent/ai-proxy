/** 增量维护的内存估算器。事件处理时直接加减，零遍历开销。 */

export interface MemoryStats {
  entryCount: number
  chunkCount: number
  sessionCount: number
  /** headers + requestBody + response chunks（UTF-16 估算） */
  bodyBytes: number
  /** TrafficEntry / 数组指针等 V8 结构体开销 */
  structBytes: number
  sessionEstBytes: number
  /** 所有字段合计 */
  totalEstBytes: number
}

const EMPTY: MemoryStats = {
  entryCount: 0,
  chunkCount: 0,
  sessionCount: 0,
  bodyBytes: 0,
  structBytes: 0,
  sessionEstBytes: 0,
  totalEstBytes: 0,
}

// ── V8 结构开销常量 ────────────────────────────────────────────────

/** TrafficEntry 对象基础开销：V8 对象头 + ~20 个属性槽 */
const ENTRY_STRUCT_COST = 200
/** responseChunks 数组每元素指针开销 */
const ARRAY_SLOT_COST = 8

// ── 格式化 ──────────────────────────────────────────────────────────

/** 紧凑数字：>= 1000 用 K */
function compactNum(n: number): string {
  if (n >= 1000) {
    const k = n / 1000
    return k >= 100 ? `${Math.round(k)}K` : `${k.toFixed(1).replace(/\.0$/, '')}K`
  }
  return String(n)
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const kb = bytes / 1024
  if (kb < 1024) return `${kb.toFixed(1).replace(/\.0$/, '')} KB`
  const mb = kb / 1024
  if (mb < 1024) return `${mb.toFixed(1).replace(/\.0$/, '')} MB`
  const gb = mb / 1024
  return `${gb.toFixed(1).replace(/\.0$/, '')} GB`
}

/** tooltip 内每行格式化：始终用 MB，精确到 2 位小数 */
export function formatBytesMB(bytes: number): string {
  if (bytes === 0) return '0 B'
  const mb = bytes / (1024 * 1024)
  if (mb < 0.01) return `${bytes} B`
  return `${mb.toFixed(2)} MB`
}

export function formatMemoryStats(s: MemoryStats): string {
  const parts: string[] = []
  parts.push(`E:${compactNum(s.entryCount)}`)
  if (s.chunkCount > 0) parts.push(`C:${compactNum(s.chunkCount)}`)
  parts.push(`S:${s.sessionCount}`)
  parts.push(formatBytes(s.totalEstBytes))
  return parts.join(' · ')
}

// ── 辅助估算函数 ───────────────────────────────────────────────────

/** 请求头/响应头的估算函数（UTF-16 × 2） */
export function estHeadersSize(headers: Record<string, string> | null | undefined): number {
  if (!headers) return 0
  return JSON.stringify(headers).length * 2
}

/** 字符串的 UTF-16 估计字节数 */
export function estStrBytes(s: string | null | undefined): number {
  return s ? s.length * 2 : 0
}

// ── 增量累加器 ───────────────────────────────────────────────────

/**
 * 增量累加器——只在事件处理时调用 add/sub，不遍历数组。
 *
 * 追踪维度：
 * - bodyBytes: headers(UTF-16×2) + requestBody 字符串 + responseChunks 数据
 * - structBytes: TrafficEntry / 数组等 V8 对象开销
 * - totalEstBytes = bodyBytes + structBytes + sessionEstBytes
 */
export class MemAccum {
  private _v: MemoryStats = { ...EMPTY }

  get snapshot(): MemoryStats {
    return { ...this._v }
  }

  // ── Entry 生命周期 ─────────────────────────────────────────────

  /** 新 entry：headers 大小 + TrafficEntry 结构开销 */
  addEntry(headersSize: number) {
    this._v.entryCount++
    this._v.bodyBytes += headersSize
    this._v.structBytes += ENTRY_STRUCT_COST
    this._recalc()
  }

  /** entry 被淘汰/清除时扣减：headers + 结构开销 */
  removeEntry(headersSize: number) {
    this._v.entryCount = Math.max(0, this._v.entryCount - 1)
    this._v.bodyBytes = Math.max(0, this._v.bodyBytes - headersSize)
    this._v.structBytes = Math.max(0, this._v.structBytes - ENTRY_STRUCT_COST)
    this._recalc()
  }

  // ── 请求体 ─────────────────────────────────────────────────────

  /** 请求体 chunk 到达（追加到 requestBody 字符串） */
  addReqChunk(chunk: string) {
    const sz = chunk.length * 2
    this._v.bodyBytes += sz
    this._recalc()
  }

  // ── 响应 ───────────────────────────────────────────────────────

  /** 响应 headers 到达 */
  addRespHeaders(headersSize: number) {
    this._v.bodyBytes += headersSize
    this._recalc()
  }

  /**
   * 响应体 chunk → responseChunks 数组增长。
   * 只追踪字符串本身（UTF-16）和数组槽位。
   */
  addRespChunk(chunk: string) {
    const sz = chunk.length * 2
    this._v.bodyBytes += sz
    this._v.chunkCount++
    this._v.structBytes += ARRAY_SLOT_COST
    this._recalc()
  }

  // ── 瘦身 / 清除 ────────────────────────────────────────────────

  /**
   * entry 瘦身：清空 all body/chunks 但保留元信息。
   * @param prevBodySize body 字符串大小（含 headers + requestBody + responseChunks）
   * @param prevChunkCount chunk 数量
   */
  slimEntry(prevBodySize: number, prevChunkCount: number) {
    this._v.bodyBytes = Math.max(0, this._v.bodyBytes - prevBodySize)
    this._v.chunkCount = Math.max(0, this._v.chunkCount - prevChunkCount)
    this._v.structBytes = Math.max(0, this._v.structBytes - prevChunkCount * ARRAY_SLOT_COST)
    this._recalc()
  }

  /** clear：全部归零 */
  clear() {
    this._v = { ...EMPTY }
  }

  /** 更新 sessions 统计数据（全量替换——sessions 的增量变化不频繁） */
  setSessions(count: number, estBytes: number) {
    this._v.sessionCount = count
    this._v.sessionEstBytes = estBytes
    this._recalc()
  }

  private _recalc() {
    this._v.totalEstBytes =
      this._v.bodyBytes + this._v.structBytes + this._v.sessionEstBytes
  }
}
