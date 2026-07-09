import type { RequestHandler } from 'msw'

// 默认空:mock 仅作为脱离后端的纯 UI 迭代层(VITE_ENABLE_MOCKS=true 启用)。
// 测试用例内通过 server.use(...) 注入各自的 handler。
export const handlers: RequestHandler[] = []
