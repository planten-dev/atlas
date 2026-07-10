import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  DINGTALK_H5_ATTEMPT_KEY,
  createDingTalkH5AttemptTracker,
  fetchDingTalkAuthCode,
  isDingTalkWebview,
} from './dingtalk'

afterEach(() => vi.useRealTimers())

describe('isDingTalkWebview', () => {
  it('钉钉容器 UA 命中', () => {
    expect(
      isDingTalkWebview(
        'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AliApp(DingTalk/7.5.0)',
      ),
    ).toBe(true)
    expect(isDingTalkWebview('... dingtalk-win ...')).toBe(true)
  })

  it('普通浏览器不命中', () => {
    expect(
      isDingTalkWebview(
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/126.0 Safari/537.36',
      ),
    ).toBe(false)
  })
})

describe('createDingTalkH5AttemptTracker', () => {
  it('写入 sessionStorage 并在后续判定中阻止重试', () => {
    const values = new Map<string, string>()
    const tracker = createDingTalkH5AttemptTracker(() => ({
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => values.set(key, value),
    }))

    expect(tracker.hasAttempted()).toBe(false)
    tracker.markAttempted()
    expect(values.get(DINGTALK_H5_ATTEMPT_KEY)).toBe('1')
    expect(tracker.hasAttempted()).toBe(true)
  })

  it('sessionStorage 抛错时使用内存标记', () => {
    const tracker = createDingTalkH5AttemptTracker(() => {
      throw new Error('storage blocked')
    })

    expect(tracker.hasAttempted()).toBe(false)
    tracker.markAttempted()
    expect(tracker.hasAttempted()).toBe(true)
  })
})

describe('fetchDingTalkAuthCode', () => {
  it('返回 SDK 提供的 authCode', async () => {
    const requestAuthCode = vi.fn().mockResolvedValue({ code: 'auth-code' })

    await expect(
      fetchDingTalkAuthCode('corp-id', { requestAuthCode }),
    ).resolves.toBe('auth-code')
    expect(requestAuthCode).toHaveBeenCalledWith('corp-id')
  })

  it('SDK 未返回 code 时失败', async () => {
    await expect(
      fetchDingTalkAuthCode('corp-id', {
        requestAuthCode: async () => ({}),
      }),
    ).rejects.toThrow('returned no code')
  })

  it('超过时限后失败，不会无限等待', async () => {
    vi.useFakeTimers()
    const result = expect(
      fetchDingTalkAuthCode('corp-id', {
        timeoutMs: 50,
        requestAuthCode: () => new Promise(() => undefined),
      }),
    ).rejects.toThrow('timed out')

    await vi.advanceTimersByTimeAsync(50)
    await result
  })
})
