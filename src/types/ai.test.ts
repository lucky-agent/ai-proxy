import { describe, it, expect } from 'vitest'
import { isAiProvider, type AiHint } from './ai'

describe('AiHint 类型 guard', () => {
  it('识别 none / candidate 字符串', () => {
    const a: AiHint = 'none'
    const b: AiHint = 'candidate'
    expect(a).toBe('none')
    expect(b).toBe('candidate')
  })

  it('识别 provider 对象', () => {
    const c: AiHint = { provider: 'openai' }
    expect(c).toEqual({ provider: 'openai' })
  })

  it('isAiProvider 校验合法 provider', () => {
    expect(isAiProvider('openai')).toBe(true)
    expect(isAiProvider('anthropic')).toBe(true)
    expect(isAiProvider('gpt')).toBe(false)
  })
})
