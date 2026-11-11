# 测试与性能

本页介绍 Redux/RTK Query 应用的测试策略和性能优化技术。

## 测试 Redux Slice

### 基础 Reducer 测试

```tsx
// user.slice.test.ts
import reducer, { userLoggedIn, userLoggedOut } from './user.slice';

describe('用户 slice', () => {
  const initialState = {
    account: null,
    isAuthenticated: false,
  };

  it('应该处理 userLoggedIn', () => {
    const account = '0x123...';
    const actual = reducer(initialState, userLoggedIn({ account }));
    
    expect(actual.account).toBe(account);
    expect(actual.isAuthenticated).toBe(true);
  });

  it('应该处理 userLoggedOut', () => {
    const loggedInState = {
      account: '0x123...',
      isAuthenticated: true,
    };
    
    const actual = reducer(loggedInState, userLoggedOut());
    
    expect(actual.account).toBeNull();
    expect(actual.isAuthenticated).toBe(false);
  });
});
```

### 测试 Entity Adapter

```tsx
// credits.slice.test.ts
import reducer, { txAdded, txsCleared, creditsSelectors } from './credits.slice';

describe('带有 entity adapter 的 credits slice', () => {
  it('应该添加一笔交易', () => {
    const initialState = reducer(undefined, { type: 'unknown' });
    const tx = { id: '1', address: '0x123', amount: 100, type: 'grant', timestamp: Date.now() };
    
    const actual = reducer(initialState, txAdded(tx));
    
    expect(creditsSelectors.selectById({ credits: actual }, '1')).toEqual(tx);
    expect(creditsSelectors.selectTotal({ credits: actual })).toBe(1);
  });

  it('应该清除所有交易', () => {
    const initialState = reducer(undefined, { type: 'unknown' });
    const withTx = reducer(initialState, txAdded({ id: '1', /* ... */ }));
    
    const actual = reducer(withTx, txsCleared());
    
    expect(creditsSelectors.selectTotal({ credits: actual })).toBe(0);
  });
});
```

## 测试选择器

### 简单选择器

```tsx
// user.selectors.test.ts
import { selectAccount, selectIsAuthenticated } from './user.slice';

describe('用户选择器', () => {
  const mockState = {
    user: {
      account: '0x123...',
      isAuthenticated: true,
    },
    // ... 其他 slice
  };

  it('应该选择 account', () => {
    expect(selectAccount(mockState)).toBe('0x123...');
  });

  it('应该选择认证状态', () => {
    expect(selectIsAuthenticated(mockState)).toBe(true);
  });
});
```

### 记忆化选择器

```tsx
// land.selectors.test.ts
import { selectFilteredParcels, selectActiveFiltersCount } from './land.selectors';

describe('land 选择器', () => {
  const mockState = {
    land: {
      filters: { owner: '0x123', minPrice: 100 },
      parcels: [
        { id: '1', owner: '0x123', price: 150 },
        { id: '2', owner: '0x456', price: 200 },
      ],
    },
  };

  it('应该统计活动过滤器数量', () => {
    expect(selectActiveFiltersCount(mockState)).toBe(2);
  });

  it('应该筛选地块', () => {
    const result = selectFilteredParcels(mockState);
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('1');
  });

  it('应该记忆化结果', () => {
    const result1 = selectFilteredParcels(mockState);
    const result2 = selectFilteredParcels(mockState);
    
    // 相同引用 = 已记忆化
    expect(result1).toBe(result2);
  });
});
```

## 使用 MSW 测试 RTK Query

### 设置 MSW

```tsx
// src/test/server.ts
import { setupServer } from 'msw/node';
import { http, HttpResponse } from 'msw';

const handlers = [
  http.get('/api/v1/parcels/:id', ({ params }) => {
    return HttpResponse.json({
      id: params.id,
      x: 10,
      y: 20,
      owner: '0x123',
      name: '测试地块',
    });
  }),

  http.post('/api/v1/credits/grant', async ({ request }) => {
    const body = await request.json();
    return HttpResponse.json({
      ok: true,
      newBalance: 1000,
    });
  }),
];

export const server = setupServer(...handlers);
```

```tsx
// src/test/setup.ts
import { server } from './server';

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());
```

### 测试查询

```tsx
// land.client.test.tsx
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { setupStore } from '@/app/store';
import { useGetParcelQuery } from './land.client';

function TestComponent({ id }: { id: string }) {
  const { data, isLoading, isError } = useGetParcelQuery({ id });

  if (isLoading) return <div>加载中...</div>;
  if (isError) return <div>错误</div>;
  if (!data) return null;

  return <div>{data.name}</div>;
}

describe('land 客户端', () => {
  it('应该获取并显示地块数据', async () => {
    const store = setupStore();

    render(
      <Provider store={store}>
        <TestComponent id="1" />
      </Provider>
    );

    expect(screen.getByText('加载中...')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('测试地块')).toBeInTheDocument();
    });
  });
});
```

### 测试 Mutation

```tsx
// credits.client.test.tsx
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { setupStore } from '@/app/store';
import { useGrantCreditsMutation } from './credits.client';

function TestComponent() {
  const [grant, { isLoading, isSuccess }] = useGrantCreditsMutation();

  return (
    <div>
      <button onClick={() => grant({ address: '0x123', amount: 100 })}>
        授予
      </button>
      {isLoading && <div>加载中...</div>}
      {isSuccess && <div>成功！</div>}
    </div>
  );
}

describe('credits 客户端 mutations', () => {
  it('应该成功授予 credits', async () => {
    const store = setupStore();

    render(
      <Provider store={store}>
        <TestComponent />
      </Provider>
    );

    fireEvent.click(screen.getByText('授予'));

    await waitFor(() => {
      expect(screen.getByText('成功！')).toBeInTheDocument();
    });
  });
});
```

### 测试乐观更新

```tsx
// credits.client.test.ts
import { server } from '@/test/server';
import { http, HttpResponse } from 'msw';
import { setupStore } from '@/app/store';
import { creditsClient } from './credits.client';

describe('乐观更新', () => {
  it('应该乐观更新缓存并在出错时回滚', async () => {
    const store = setupStore();
    const address = '0x123';

    // 预取初始余额
    await store.dispatch(
      creditsClient.endpoints.getBalance.initiate({ address })
    );

    const initialBalance = creditsClient.endpoints.getBalance.select({ address })(
      store.getState()
    ).data?.amount;

    expect(initialBalance).toBe(100); // 来自 MSW 处理器

    // 模拟失败
    server.use(
      http.post('/api/v1/credits/grant', () => {
        return HttpResponse.json({ error: 'Failed' }, { status: 500 });
      })
    );

    // 触发 mutation
    const mutation = store.dispatch(
      creditsClient.endpoints.grantCredits.initiate({
        address,
        amount: 50,
      })
    );

    // 检查乐观更新
    const optimisticBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(optimisticBalance).toBe(150); // 100 + 50

    // 等待 mutation 失败
    await expect(mutation).rejects.toThrow();

    // 检查回滚
    const rolledBackBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(rolledBackBalance).toBe(100); // 回到原始值
  });
});
```

## 性能优化

### 为简单的 UI 状态使用 `selectFromResult` 以防止重新渲染

```tsx
// ✅ 好：仅订阅特定字段
const { owner } = useGetParcelQuery(
  { id },
  {
    selectFromResult: ({ data }) => ({
      owner: data?.owner,
    }),
  }
);

// 组件只会在 owner 变化时重新渲染
```

### 避免选择整个 state

```tsx
// ✅ 好：选择特定值
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ 不好：选择整个 slice
const ui = useAppSelector((state) => state.ui);
```

### 记忆化昂贵的选择器

```tsx
import { createSelector } from '@reduxjs/toolkit';

// ✅ 好：已记忆化的选择器
export const selectFilteredParcels = createSelector(
  [selectAllParcels, selectFilters],
  (parcels, filters) => {
    // 昂贵的过滤逻辑
    return parcels.filter(/* ... */);
  }
);

// ❌ 不好：在组件中计算
function Component() {
  const parcels = useAppSelector(selectAllParcels);
  const filters = useAppSelector(selectFilters);
  
  // 每次渲染都会重新计算！
  const filtered = parcels.filter(/* ... */);
}
```

### 对规范化数据使用 Entity Adapter

```tsx
// ✅ 好：使用 entity adapter 规范化
const adapter = createEntityAdapter<Parcel>();

// 按 ID 高效查找
const parcel = adapter.getSelectors().selectById(state, id);

// ❌ 不好：数组查找
const parcel = state.parcels.find((p) => p.id === id);
```

### 调整 RTK Query 缓存设置

```tsx
export const client = createApi({
  // ...
  keepUnusedDataFor: 60, // 保留数据 60 秒
  refetchOnMountOrArgChange: 30, // 如果数据超过 30 秒则重新获取
  refetchOnFocus: true, // 当窗口重新获得焦点时重新获取
  refetchOnReconnect: true, // 重新连接时重新获取
});
```

### 通过预取改善用户体验

```tsx
function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // 悬停时预取
    dispatch(
      client.util.prefetch('getParcel', { id: parcel.id }, { force: false })
    );
  };

  return (
    <Link to={`/parcels/${parcel.id}`} onMouseEnter={handleMouseEnter}>
      {parcel.name}
    </Link>
  );
}
```

### 轮询策略

```tsx
// 仅在需要时轮询
const { data } = useGetBalanceQuery(
  { address },
  {
    pollingInterval: isActive ? 10000 : 0, // 仅在激活时轮询
    skipPollingIfUnfocused: true, // 标签页未聚焦时暂停
  }
);
```

## Redux DevTools

### 在开发环境中启用

```tsx
export const store = configureStore({
  // ...
  devTools: process.env.NODE_ENV !== 'production',
});
```

### Action 清理器

在 DevTools 中清理敏感数据：

```tsx
const actionSanitizer = (action: any) => {
  if (action.type === 'user/loggedIn') {
    return {
      ...action,
      payload: {
        ...action.payload,
        authToken: '***REDACTED***',
      },
    };
  }
  return action;
};

export const store = configureStore({
  // ...
  devTools: {
    actionSanitizer,
  },
});
```

## 最佳实践清单

### 性能

* [ ] 为简单的 UI 状态使用 `selectFromResult` 适用于较大的查询结果
* [ ] 使用以下方式记忆化昂贵的选择器 `createSelector`
* [ ] 对规范化集合使用 entity adapter
* [ ] 避免在组件中选择整个 slice
* [ ] 调整 `keepUnusedDataFor` 以符合你的使用场景
* [ ] 在导航前预取数据
* [ ] 有策略地使用轮询（仅在需要时）

### 测试

* [ ] 为所有 reducer 和 action 编写单元测试
* [ ] 测试记忆化选择器的正确性和性能
* [ ] 使用 MSW 测试 RTK Query 端点
* [ ] 测试乐观更新和回滚逻辑
* [ ] 测试组件中的错误处理
* [ ] 为关键流程编写集成测试

### 代码质量

* [ ] 使用类型化的 hooks（`useAppSelector`, `useAppDispatch`)
* [ ] 处理所有查询状态（加载中、错误、成功）
* [ ] 为简单的 UI 状态使用 `.unwrap()` 用于 mutation 错误处理
* [ ] 在 mutation 后使缓存失效或更新缓存
* [ ] 让不可序列化的数据远离 Redux
* [ ] 记录复杂的选择器和逻辑

## 要避免的反模式


> [!CAUTION]
> **不要这样做：**
> 
> 1. 在 Redux 中存储不可序列化对象（providers、signers）
> 2. 在 slices 和 RTK Query 中重复存储数据
> 3. 在渲染期间派发 action
> 4. 创建不进行记忆化、却返回新对象的选择器
> 5. 忽略加载和错误状态
> 6. 不使用 RTK Query，在多个组件中获取相同数据
> 7. 过度轮询或轮询时不使用 `skipPollingIfUnfocused`


## 监控性能

### 跟踪选择器调用

```tsx
import { createSelector } from '@reduxjs/toolkit';

const selectExpensiveData = createSelector(
  [selectData],
  (data) => {
    console.log('选择器被调用'); // 只有在数据变化时才应输出日志
    return expensiveOperation(data);
  }
);
```

### 监控重新渲染

```tsx
import { useEffect, useRef } from 'react';

function useRenderCount() {
  const renderCount = useRef(0);
  
  useEffect(() => {
    renderCount.current += 1;
    console.log('渲染次数：', renderCount.current);
  });
}

function Component() {
  useRenderCount(); // 跟踪重新渲染
  // ...
}
```

## 下一步

* 回顾 [组件模式](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/component-patterns/) 用于使用示例
* 查看 [RTK Query](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/rtk-query/) 用于缓存策略
* 理解 [状态管理](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/state-management/) 用于 slice 优化


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/testing-and-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
