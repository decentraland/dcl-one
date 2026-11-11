# Store 设置

本页介绍如何配置 Redux store、设置类型化 hooks，以及组织你的项目结构。

## 文件夹结构

项目必须遵循以下结构以确保一致性和可维护性：

```
src/
  app/
    store.ts              # Store 配置
    hooks.ts              # 类型化 hooks（useAppDispatch/useAppSelector）
  shared/
    types/                # DTO（API）、领域模型、映射器
    utils/                # 共享工具
  services/
    client.ts             # RTK Query 基础客户端
    baseQuery.ts          # 带认证/chainId/重试的基础查询
  features/
    user/
      user.client.ts      # RTK Query 端点
      user.slice.ts       # UI 状态切片
      user.selectors.ts   # 记忆化选择器
      __tests__/          # 测试
    land/
      land.client.ts
      land.slice.ts
      land.selectors.ts
    credits/
      credits.client.ts
      credits.slice.ts
      credits.selectors.ts
```

### 文件夹组织原则

* **基于功能** - 按业务域分组，而不是按技术角色分组
* **同位置放置** - 将相关代码放在一起
* **清晰分离** - 区分远程数据（`.client.ts`）和本地状态（`.slice.ts`)

## Store 配置

store 必须使用 RTK 的 `configureStore` 进行配置，并包含所有必要的中间件。

### 基础 Store 设置

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';
import { client } from '@/services/client';
import userReducer from '@/features/user/user.slice';
import landReducer from '@/features/land/land.slice';
import creditsReducer from '@/features/credits/credits.slice';

export const store = configureStore({
  reducer: {
    // RTK Query 客户端 reducer（必须包含）
    [client.reducerPath]: client.reducer,
    
    // 功能切片
    user: userReducer,
    land: landReducer,
    credits: creditsReducer,
  },
  
  middleware: (getDefaultMiddleware) =>
    getDefaultMiddleware({
      serializableCheck: {
        // 忽略 redux-persist 或其他已知的不可序列化路径
        ignoredActions: ['persist/PERSIST', 'persist/REHYDRATE'],
        ignoredPaths: ['register'],
      },
    }).concat(client.middleware), // 必须包含 RTK Query 中间件
    
  devTools: process.env.NODE_ENV !== 'production',
});

// 导出应用各处使用的类型
export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
```

### 配置选项

#### 可序列化检查

该 `serializableCheck` middleware 会验证所有状态是否可序列化。将其配置为忽略已知例外：

```tsx
serializableCheck: {
  // 要忽略的 actions
  ignoredActions: [
    'persist/PERSIST',
    'persist/REHYDRATE',
  ],
  // 要忽略的状态路径
  ignoredPaths: ['register', 'socket.connection'],
}
```


> [!WARNING]
> **切勿 `serializableCheck` 完全禁用。** 应改为配置例外，并将不可序列化的数据放在 Redux 之外。


#### 开发工具

在开发环境中启用 Redux DevTools 以便调试：

```tsx
devTools: process.env.NODE_ENV !== 'production'
```

如需在生产环境中调试，请使用特定配置：

```tsx
devTools: process.env.NODE_ENV !== 'production' ? true : {
  name: 'Decentraland App',
  trace: false,
  traceLimit: 25,
}
```

## 类型化 Hooks

创建以下的类型化版本： `useDispatch` 和 `useSelector` 以获得更好的类型推断和开发体验。

### Hook 设置

```tsx
// src/app/hooks.ts
import { TypedUseSelectorHook, useDispatch, useSelector } from 'react-redux';
import type { RootState, AppDispatch } from './store';

// 类型化 dispatch hook
export const useAppDispatch = () => useDispatch<AppDispatch>();

// 类型化 selector hook
export const useAppSelector: TypedUseSelectorHook<RootState> = useSelector;
```

### 组件中的用法

```tsx
// ❌ 不佳：使用标准 hooks
import { useDispatch, useSelector } from 'react-redux';
import { RootState } from '@/app/store';

function Component() {
  const dispatch = useDispatch(); // 无类型推断
  const user = useSelector((state: RootState) => state.user); // 手动类型标注
}

// ✅ 优秀：使用类型化 hooks
import { useAppDispatch, useAppSelector } from '@/app/hooks';

function Component() {
  const dispatch = useAppDispatch(); // 自动类型化
  const user = useAppSelector((state) => state.user); // 推断出 RootState
}
```

### 类型化 Hooks 的好处

1. **自动补全** - 针对状态结构提供完整 IntelliSense
2. **类型安全** - 在编译时捕获错误
3. **重构** - 可以放心重命名状态属性
4. **更少样板代码** - 无需重复指定 `RootState` 重复

## Provider 设置

用 Redux Provider 包裹你的应用：

### Next.js App Router

```tsx
// app/providers.tsx
'use client';

import { Provider } from 'react-redux';
import { store } from '@/app/store';

export function Providers({ children }: { children: React.ReactNode }) {
  return <Provider store={store}>{children}</Provider>;
}
```

```tsx
// app/layout.tsx
import { Providers } from './providers';

export default function RootLayout({ children }) {
  return (
    <html>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
```

### Next.js Pages Router

```tsx
// pages/_app.tsx
import { Provider } from 'react-redux';
import { store } from '@/app/store';
import type { AppProps } from 'next/app';

export default function App({ Component, pageProps }: AppProps) {
  return (
    <Provider store={store}>
      <Component {...pageProps} />
    </Provider>
  );
}
```

### React（Vite/CRA）

```tsx
// main.tsx / index.tsx
import { Provider } from 'react-redux';
import { store } from '@/app/store';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Provider store={store}>
      <App />
    </Provider>
  </React.StrictMode>
);
```

## 环境配置

配置特定环境的设置：

```tsx
// src/config/constants.ts
export const API_URL = process.env.NEXT_PUBLIC_API_URL || 'https://api.decentraland.org';
export const WS_URL = process.env.NEXT_PUBLIC_WS_URL || 'wss://api.decentraland.org';
export const CHAIN_ID = process.env.NEXT_PUBLIC_CHAIN_ID || '1';

// RTK Query 设置
export const RTK_QUERY_CONFIG = {
  keepUnusedDataFor: 60, // 秒
  refetchOnFocus: true,
  refetchOnReconnect: true,
  refetchOnMountOrArgChange: 30, // 秒
} as const;
```

## 类型定义

创建共享类型定义以保持一致性：

```tsx
// src/shared/types/api.types.ts
/** API 响应包装器 */
export interface ApiResponse<T> {
  ok: boolean;
  data: T;
  error?: string;
}

/** 分页响应 */
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  limit: number;
}

/** 常见 API 错误 */
export interface ApiError {
  message: string;
  code: string;
  details?: Record<string, any>;
}
```

```tsx
// src/shared/types/domain.types.ts
/** 用户领域模型 */
export interface User {
  id: string;
  address: string;
  name?: string;
  avatar?: string;
  createdAt: string;
}

/** 地块领域模型 */
export interface Parcel {
  id: string;
  x: number;
  y: number;
  owner: string;
  name?: string;
}
```

## 多个 Store 实例

用于测试或微前端时，你可能需要多个 store 实例：

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';

export function createStore(preloadedState?: Partial<RootState>) {
  return configureStore({
    reducer: {
      // ... reducers
    },
    preloadedState,
    // ... 其他配置
  });
}

// 默认 store 实例
export const store = createStore();

export type AppStore = ReturnType<typeof createStore>;
export type RootState = ReturnType<AppStore['getState']>;
export type AppDispatch = AppStore['dispatch'];
```

## 最佳实践

### 1. 单一 Store

始终为每个应用只使用一个 store 实例：

```tsx
// ✅ 好：一个 store
export const store = configureStore({ ... });

// ❌ 不佳：多个 store
export const userStore = configureStore({ ... });
export const cartStore = configureStore({ ... });
```

### 2. 懒加载 Reducer

为进行代码拆分，可动态注入 reducers：

```tsx
import { combineReducers } from '@reduxjs/toolkit';

const staticReducers = {
  user: userReducer,
};

export function createReducer(asyncReducers = {}) {
  return combineReducers({
    ...staticReducers,
    ...asyncReducers,
  });
}

// 在 store 中
let currentReducers = createReducer();

export function injectReducer(key: string, reducer: Reducer) {
  currentReducers = createReducer({ [key]: reducer });
  store.replaceReducer(currentReducers);
}
```

### 3. 热模块替换

在开发环境中为 reducers 启用 HMR：

```tsx
if (process.env.NODE_ENV === 'development' && module.hot) {
  module.hot.accept('./reducer', () => {
    const newRootReducer = require('./reducer').default;
    store.replaceReducer(newRootReducer);
  });
}
```

### 4. 状态持久化

使用 redux-persist 时，请谨慎配置：

```tsx
import { persistStore, persistReducer } from 'redux-persist';
import storage from 'redux-persist/lib/storage';

const persistConfig = {
  key: 'root',
  storage,
  // 仅持久化特定切片
  whitelist: ['user', 'preferences'],
  // 绝不持久化 RTK Query 缓存
  blacklist: ['client'],
};

const persistedReducer = persistReducer(persistConfig, rootReducer);
```

## 下一步

* 了解 [RTK Query](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/rtk-query/) 用于数据获取
* 理解 [状态管理](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/state-management/) 用于本地状态
* 回顾 [组件模式](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/component-patterns/) 用于使用示例


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/store-setup.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
