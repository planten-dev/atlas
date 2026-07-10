# Atlas 前端

设计文档:[docs/frontend-design.md](../docs/frontend-design.md)(定稿 v1)。
技术栈:Vite 8 + React 19 + TypeScript(strict) + Tailwind v4 + shadcn/ui(Base UI 原语) + TanStack Router/Query/Table + react-hook-form + zod + openapi-fetch。

## 开发

```bash
# 仓库根目录
just run             # 起后端(sqlite,127.0.0.1:3000)
just frontend-dev    # 起前端(Vite dev server,http://127.0.0.1:5173)
```

Vite proxy 把 `/api` 转发到后端,HttpOnly cookie(`atlas_session`)同源直通。

**钉钉 OAuth 回环(dev)**:登录整页跳转到后端 `/api/v1/auth/login/dingtalk`;钉钉授权后回调直达后端 `127.0.0.1:3000`,cookie 落在主机 `127.0.0.1` 上。因此 **dev 请用 `http://127.0.0.1:5173` 访问前端**(不要用 `localhost:5173` —— 浏览器视其为不同主机,cookie 带不过去,登录后会再次要求登录),后端 `frontend_callback_url` 也已配为 `http://127.0.0.1:5173/`。

**钉钉 H5 免登**:登录页在钉钉容器内(UA 含 DingTalk)且配置了 `VITE_DINGTALK_CORP_ID` 时,自动用 JSAPI `requestAuthCode` 取 authCode 并 POST `/api/v1/auth/login/dingtalk/h5` 完成免登;任何失败回退到 OAuth 按钮。corpId 从 `.env`(见 `.env.example`)读取;sessionStorage 一次性标记避免失败后无限重试。

## 正式环境部署

推荐使用单一公网 HTTPS 域名,例如 `https://atlas.example.com`:前端静态文件放在该域名根路径,反向代理将同域 `/api` 转发到 Atlas 后端。前端 API 客户端和 HttpOnly 会话 cookie 均按此前提设计,不需要跨域 CORS。

- 构建前设置 `VITE_DINGTALK_CORP_ID`,其值必须与钉钉应用所属企业一致。
- 钉钉应用首页地址和安全域名均配置为正式 HTTPS 域名。
- 后端 `dingtalk.redirect_uri` 配置为 `https://atlas.example.com/api/v1/auth/callback/dingtalk`。
- 后端 `auth.frontend_callback_url` 配置为 `https://atlas.example.com/`。
- 正式环境设置 `session.cookie_secure = true`。
- 手机钉钉中的 `127.0.0.1`/`localhost` 指向手机自身,不能用于联调;真机测试必须使用手机可访问且已加入钉钉安全域名的地址。

钉钉 Android Chromium 69 级内核和 iOS 13 会使用 legacy JavaScript/CSS 降级。构建末尾的兼容检查会验证 legacy 入口、cascade layer 拍平、Tailwind 属性默认值和传统主题颜色兜底;检查失败时不会产出可发布构建。

## 常用命令

| 命令 | 作用 |
|---|---|
| `npm run dev` | 开发服务器 |
| `npm run gen:api` | 从 `../docs/openapi.json` 重新生成 `src/api/types.gen.ts`(后端改契约后必跑;生成文件禁止手改) |
| `npm run typecheck` / `lint` / `test` / `build` | 质量门(`just frontend-check` 一次跑全) |

## 架构规则(ESLint 强制)

- 页面/组件只 import `hooks/` 与 `components/`,不直接摸 `@/api/client`。
- `src/api/` 之外禁止裸 `fetch`。
- 金额是字符串(`DECIMAL(12,2)`),运算走 `lib/money.ts`(BigInt 分),禁止 `parseFloat`。
- 列表页筛选一律定义为路由 `validateSearch`(zod),URL 即状态。
- 每个域的 mutation 统一返回 `MutationOutcome`(`applied | submitted`)—— 后端逐域接入审批流时只改对应 hook,页面零改动。

## Mock(可选层,默认关)

`VITE_ENABLE_MOCKS=true npm run dev` 启用 MSW,用于脱离后端的纯 UI 迭代;测试始终经由 `src/mocks/server.ts` 注入各自 handler。

## 业务目录

`src/reports/registry.ts` 登记旧平台 6 组 44 条业务条目(菜单/命令面板/占位页由它驱动)。后端每落地一个统计接口,翻一条 `status: 'reserved' → 'available'` 即点亮菜单。标 `[占位]` 的条目名称需上线前与旧平台目录核对。
