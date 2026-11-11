# RTK Query

RTK Query 是 Redux Toolkit 强大的数据获取和缓存层。它免去了编写异步 action 创建器的需要，并会自动管理加载状态、缓存和数据同步。

## 基础客户端配置

所有 RTK Query 端点都从单一的基础客户端实例扩展而来。

### 基础查询设置

```tsx
// src/services/baseQuery.ts
import { fetchBaseQuery } from '@reduxjs/toolkit/query/react';
import type { RootState } from '@/app/store';

export const baseQuery = fetchBaseQuery({
  baseUrl: process.env.NEXT_PUBLIC_API_URL, // 例如：https://api.decentraland.org
  
  prepareHeaders: (headers, { getState }) => {
    const state = getState() as RootState;
    
    // 添加身份验证令牌
    const token = state.user.session?.authToken;
    if (token) {
      headers.set('authorization', `Bearer ${token}`);
    }
    
    // 为 web3 上下文添加链 ID
    const chainId = state.user.chainId;
    if (chainId) {
      headers.set('x-chain-id', String(chainId));
    }
    
    // 标准请求头
    headers.set('accept', 'application/json');
    headers.set('content-type', 'application/json');
    
    return headers;
  },
  
  credentials: 'omit', // 如果后端需要 cookie，则可改为 'include'
});
```

### 客户端实例

```tsx
// src/services/client.ts
import { createApi } from '@reduxjs/toolkit/query/react';
import { baseQuery } from './baseQuery';

export const client = createApi({
  reducerPath: 'client',
  baseQuery,
  
  // 定义所有可能用于缓存失效的标签类型
  tagTypes: [
    'User',
    'Profile',
    'Parcels',
    'Estates',
    'Credits',
    'Orders',
    'Sales',
    'NFTs',
  ],
  
  // 缓存配置
  keepUnusedDataFor: 60,         // 将未使用的数据保留 60 秒
  refetchOnFocus: true,           // 窗口重新获得焦点时重新获取
  refetchOnReconnect: true,       // 重新连接时重新获取
  refetchOnMountOrArgChange: 30,  // 如果数据超过 30 秒则重新获取
  
  // 端点会在功能文件中注入
  endpoints: () => ({}),
});
```

## 标签约定

标签用于缓存失效和同步。请遵循以下约定：

### 标签命名

| 资源类型        | 查询标签                                 | 突变失效标签                         |
| ----------- | ------------------------------------ | ------------------------------ |
| **集合**      | `'Parcels'` （复数）                     | `'Parcels'`                    |
| **单个实体**    | `{type: 'Parcels', id: '123'}`       | `{type: 'Parcels', id: '123'}` |
| **列表 + 详情** | `['Parcels', {type: 'Parcels', id}]` | `['Parcels']` 或特定 id           |

### 标签示例

```tsx
// 集合：提供列表标签
providesTags: ['Parcels']

// 单个实体：提供特定标签 + 列表标签
providesTags: (result) => 
  result 
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// 突变：使列表和特定实体都失效
invalidatesTags: (result, error, arg) => [
  { type: 'Parcels', id: arg.id },
  'Parcels'
]
```

## 创建端点

端点应与其功能放在同一位置，位于 `feature.client.ts` 文件中。

### 查询端点（读取）

```tsx
// src/features/land/land.client.ts
import { client } from '@/services/client';

export type Tile = {
  x: number;
  y: number;
  type: 'parcel' | 'road' | 'plaza';
  owner?: string;
};

export type Parcel = {
  id: string;
  x: number;
  y: number;
  owner: string;
  name?: string;
  description?: string;
};

export const landClient = client.injectEndpoints({
  endpoints: (build) => ({
    // 获取所有地块
    getTiles: build.query<Record<string, Tile>, void>({
      query: () => '/v1/tiles',
      providesTags: ['Parcels'],
    }),
    
    // 根据坐标获取地块
    getParcelByCoords: build.query<Parcel, { x: number; y: number }>({
      query: ({ x, y }) => `/v1/lands/${x}/${y}`,
      providesTags: (result, error, arg) =>
        result
          ? [{ type: 'Parcels', id: result.id }, 'Parcels']
          : ['Parcels'],
    }),
    
    // 根据所有者获取地块
    getParcelsByOwner: build.query<Parcel[], { owner: string }>({
      query: ({ owner }) => `/v1/lands/owner/${owner}`,
      providesTags: (result) =>
        result
          ? [
              ...result.map(({ id }) => ({ type: 'Parcels' as const, id })),
              'Parcels',
            ]
          : ['Parcels'],
    }),
  }),
  overrideExisting: false,
});

// 导出 hooks
export const {
  useGetTilesQuery,
  useGetParcelByCoordsQuery,
  useGetParcelsByOwnerQuery,
} = landClient;
```

### 突变端点（写入）

```tsx
// src/features/land/land.client.ts（续）
export const landClient = client.injectEndpoints({
  endpoints: (build) => ({
    // ... 查询端点 ...
    
    // 更新地块名称
    updateParcelName: build.mutation<
      Parcel,
      { id: string; name: string }
    >({
      query: ({ id, name }) => ({
        url: `/v1/lands/${id}`,
        method: 'PATCH',
        body: { name },
      }),
      invalidatesTags: (result, error, arg) => [
        { type: 'Parcels', id: arg.id },
        'Parcels',
      ],
    }),
    
    // 转移地块
    transferParcel: build.mutation<
      { ok: boolean },
      { id: string; to: string }
    >({
      query: ({ id, to }) => ({
        url: `/v1/lands/${id}/transfer`,
        method: 'POST',
        body: { to },
      }),
      invalidatesTags: (result, error, arg) => [
        { type: 'Parcels', id: arg.id },
        'Parcels', // 使列表失效以更新所有者筛选
      ],
    }),
  }),
});

export const {
  useUpdateParcelNameMutation,
  useTransferParcelMutation,
} = landClient;
```

## 乐观更新

为简单的 UI 状态使用 `onQueryStarted` 用于乐观 UI 更新，并在失败时自动回滚。

```tsx
// src/features/credits/credits.client.ts
import { client } from '@/services/client';

export type CreditsBalance = {
  address: string;
  amount: number;
  lastUpdated: string;
};

export const creditsClient = client.injectEndpoints({
  endpoints: (build) => ({
    getBalance: build.query<CreditsBalance, { address: string }>({
      query: ({ address }) => `/v1/credits/${address}`,
      providesTags: (result, error, arg) => [
        { type: 'Credits', id: arg.address }
      ],
    }),
    
    grantCredits: build.mutation<
      { ok: true; newBalance: number },
      { address: string; amount: number }
    >({
      query: (body) => ({
        url: `/v1/credits/grant`,
        method: 'POST',
        body,
      }),
      
      // 乐观更新
      async onQueryStarted({ address, amount }, { dispatch, queryFulfilled }) {
        // 乐观地更新缓存
        const patchResult = dispatch(
          client.util.updateQueryData('getBalance', { address }, (draft) => {
            draft.amount += amount;
            draft.lastUpdated = new Date().toISOString();
          })
        );
        
        try {
          // 等待突变完成
          const { data } = await queryFulfilled;
          
          // 使用服务器响应更新
          dispatch(
            client.util.updateQueryData('getBalance', { address }, (draft) => {
              draft.amount = data.newBalance;
            })
          );
        } catch {
          // 失败时回滚
          patchResult.undo();
        }
      },
      
      // 同时失效以确保一致性
      invalidatesTags: (result, error, arg) => [
        { type: 'Credits', id: arg.address }
      ],
    }),
  }),
});

export const { useGetBalanceQuery, useGrantCreditsMutation } = creditsClient;
```

## 高级查询选项

### 轮询

```tsx
// 每 10 秒轮询一次
const { data } = useGetBalanceQuery(
  { address },
  { pollingInterval: 10000 }
);
```

### 跳过查询

```tsx
// 如果地址不可用则跳过查询
const { data } = useGetBalanceQuery(
  { address: address! },
  { skip: !address }
);
```

### 延迟查询

```tsx
const [trigger, result] = useLazyGetParcelByCoordsQuery();

// 手动触发
const handleClick = () => {
  trigger({ x: 10, y: 20 });
};
```

### 转换响应

```tsx
getParcel: build.query<Parcel, string>({
  query: (id) => `/v1/lands/${id}`,
  transformResponse: (response: ApiResponse<Parcel>) => response.data,
})
```

### 自定义序列化

对于分页或搜索，请自定义缓存键序列化：

```tsx
searchParcels: build.query<Parcel[], { q: string; owner?: string; page?: number }>({
  query: (args) => ({
    url: '/v1/parcels/search',
    params: args,
  }),
  
  // 用于处理可选参数的自定义缓存键
  serializeQueryArgs: ({ endpointName, queryArgs }) => {
    const { q, owner = 'any', page = 1 } = queryArgs;
    return `${endpointName}-${q}-${owner}-${page}`;
  },
  
  // 合并分页结果
  merge(currentCache, newItems, { arg }) {
    if (arg.page === 1) {
      return newItems;
    }
    return [...currentCache, ...newItems];
  },
  
  // 当参数变化时强制重新获取
  forceRefetch({ currentArg, previousArg }) {
    return JSON.stringify(currentArg) !== JSON.stringify(previousArg);
  },
  
  providesTags: ['Parcels'],
})
```

## 错误处理

### 自定义错误处理

```tsx
import { FetchBaseQueryError } from '@reduxjs/toolkit/query';

export function isFetchBaseQueryError(
  error: unknown
): error is FetchBaseQueryError {
  return typeof error === 'object' && error != null && 'status' in error;
}

export function isErrorWithMessage(
  error: unknown
): error is { message: string } {
  return (
    typeof error === 'object' &&
    error != null &&
    'message' in error &&
    typeof (error as any).message === 'string'
  );
}
```

在组件中的用法：

```tsx
const { data, error } = useGetParcelQuery({ id });

if (error) {
  if (isFetchBaseQueryError(error)) {
    const errMsg = 'error' in error ? error.error : JSON.stringify(error.data);
    return <div>错误：{errMsg}</div>;
  } else if (isErrorWithMessage(error)) {
    return <div>错误：{error.message}</div>;
  }
}
```

## 缓存管理

### 手动更新缓存

```tsx
// 直接更新缓存
dispatch(
  client.util.updateQueryData('getBalance', { address }, (draft) => {
    draft.amount = 1000;
  })
);
```

### 使缓存失效

```tsx
// 使所有 Credits 查询失效
dispatch(client.util.invalidateTags(['Credits']));

// 使特定实体失效
dispatch(client.util.invalidateTags([{ type: 'Credits', id: address }]));
```

### 重置客户端状态

```tsx
// 重置整个客户端状态
dispatch(client.util.resetApiState());
```

### 预取数据

```tsx
// 在导航前预取数据
dispatch(
  client.util.prefetch('getParcel', { id: '123' }, { force: false })
);
```

## 最佳实践

### 1. 使用描述性的端点名称

```tsx
// ✅ 好
getParcelByCoords
getParcelsByOwner
updateParcelName

// ❌ 不好
getParcel
fetch
update
```

### 2. 提供全面的标签

```tsx
// ✅ 好：同时提供列表和实体标签
providesTags: (result) =>
  result
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// ❌ 不好：只提供列表标签
providesTags: ['Parcels']
```

### 3. 处理加载和错误状态

```tsx
// ✅ 好：完整的状态处理
const { data, isLoading, isFetching, isError, error } = useGetParcelQuery({ id });

if (isLoading) return <Spinner />;
if (isError) return <Error error={error} />;
if (!data) return null;

// ❌ 不好：不完整的状态处理
const { data } = useGetParcelQuery({ id });
return <div>{data.name}</div>; // 如果 data 为 undefined，可能会崩溃
```

### 4. 使用类型守卫

```tsx
// ✅ 好：类型安全的错误处理
if (isFetchBaseQueryError(error)) {
  // 处理获取错误
} else if (isErrorWithMessage(error)) {
  // 处理带消息的错误
}

// ❌ 不好：不安全的类型转换
const message = (error as any).message;
```

## 下一步

* 了解 [状态管理](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/state-management/) 用于本地 UI 状态
* 回顾 [组件模式](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/component-patterns/) 用于使用示例
* 理解 [Web3 集成](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/web3-integration/) 用于区块链数据


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/rtk-query.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
