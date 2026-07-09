import js from '@eslint/js'
import globals from 'globals'
import tseslint from 'typescript-eslint'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'

export default tseslint.config(
  {
    ignores: ['dist', 'src/routeTree.gen.ts', 'src/api/types.gen.ts'],
  },
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.recommended,
      reactHooks.configs.flat['recommended-latest'],
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
    },
    rules: {
      // 设计规范 §3:components/ui 之外禁止裸 fetch,统一走 api/client
      'no-restricted-globals': [
        'error',
        {
          name: 'fetch',
          message: '请使用 @/api/client 的封装,不要直接调用 fetch。',
        },
      ],
    },
  },
  {
    // 设计规范 §3:页面代码只 import hooks/ 与 components/,不直接摸 api/client
    files: ['src/routes/**/*.{ts,tsx}', 'src/layouts/**/*.{ts,tsx}', 'src/components/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: '@/api/client',
              message: '页面/组件不直接访问 api/client,请经由 hooks/ 封装。',
            },
          ],
        },
      ],
    },
  },
  {
    files: ['src/api/**/*.ts', 'src/mocks/**/*.ts', 'src/test/**/*.{ts,tsx}', '**/*.test.{ts,tsx}'],
    rules: {
      'no-restricted-globals': 'off',
      'no-restricted-imports': 'off',
    },
  },
  {
    // shadcn 生成组件会同文件导出 variants 常量
    files: ['src/components/ui/**/*.tsx'],
    rules: {
      'react-refresh/only-export-components': 'off',
    },
  },
  {
    // TanStack Router 文件式路由必须同文件导出 Route;
    // Provider/工具组件同文件导出 hook 或纯函数(供测试)也是既定模式。
    files: [
      'src/routes/**/*.tsx',
      'src/auth/PermissionProvider.tsx',
      'src/components/diff/DiffView.tsx',
      'src/components/customers/CustomerForm.tsx',
    ],
    rules: {
      'react-refresh/only-export-components': 'off',
    },
  },
)
