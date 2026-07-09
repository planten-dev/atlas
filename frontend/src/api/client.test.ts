import { http, HttpResponse } from 'msw'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { server } from '@/mocks/server'
import { client, setUnauthorizedHandler, unwrap, SKIP_AUTH_REDIRECT_HEADER } from '@/api/client'
import { ApiError } from '@/api/error'

afterEach(() => {
  setUnauthorizedHandler(() => {})
})

describe('api client', () => {
  it('成功路径返回类型化数据', async () => {
    server.use(
      http.get('/api/v1/auth/me', () =>
        HttpResponse.json({
          id: 'u1',
          dingtalk_user_id: 'd1',
          status: 'active',
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
          last_login_at: null,
        }),
      ),
    )
    const data = unwrap(await client.GET('/api/v1/auth/me'))
    expect(data.id).toBe('u1')
  })

  it('错误体转为 ApiError(code + message)', async () => {
    server.use(
      http.get('/api/v1/events/list', () =>
        HttpResponse.json({ error: 'permission_denied', message: 'no' }, { status: 403 }),
      ),
    )
    await expect(client.GET('/api/v1/events/list')).rejects.toMatchObject({
      name: 'ApiError',
      status: 403,
      code: 'permission_denied',
    })
  })

  it('非 JSON 错误体使用默认文案', async () => {
    server.use(
      http.get('/api/v1/events/list', () => new HttpResponse('boom', { status: 500 })),
    )
    try {
      await client.GET('/api/v1/events/list')
      expect.unreachable()
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError)
      expect((error as ApiError).code).toBe('unknown_error')
      expect((error as ApiError).message).toContain('500')
    }
  })

  it('401 触发 onUnauthorized 一次', async () => {
    const handler = vi.fn()
    setUnauthorizedHandler(handler)
    server.use(
      http.get('/api/v1/events/list', () =>
        HttpResponse.json({ error: 'unauthorized', message: 'expired' }, { status: 401 }),
      ),
    )
    await expect(client.GET('/api/v1/events/list')).rejects.toBeInstanceOf(ApiError)
    expect(handler).toHaveBeenCalledTimes(1)
  })

  it('带 skip header 的 401 不触发跳转(启动探测)', async () => {
    const handler = vi.fn()
    setUnauthorizedHandler(handler)
    server.use(
      http.get('/api/v1/auth/me', () =>
        HttpResponse.json({ error: 'unauthorized', message: 'expired' }, { status: 401 }),
      ),
    )
    await expect(
      client.GET('/api/v1/auth/me', { headers: { [SKIP_AUTH_REDIRECT_HEADER]: '1' } }),
    ).rejects.toBeInstanceOf(ApiError)
    expect(handler).not.toHaveBeenCalled()
  })
})
