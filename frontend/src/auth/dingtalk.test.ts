import { describe, expect, it } from 'vitest'
import { isDingTalkWebview } from './dingtalk'

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
