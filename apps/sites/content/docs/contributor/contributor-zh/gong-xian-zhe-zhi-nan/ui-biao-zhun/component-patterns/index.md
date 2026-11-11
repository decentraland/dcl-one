# 组件模式

本页介绍如何在 React 组件中有效使用 Redux 和 RTK Query，包括 hooks、优化模式和最佳实践。

## 基础查询用法

使用 RTK Query 端点生成的 hooks：

```tsx
import { useGetParcelByCoordsQuery } from '@/features/land/land.client';

function ParcelCard({ x, y }: { x: number; y: number }) {
  const { data, isLoading, isError, error } = useGetParcelByCoordsQuery({ x, y });

  if (isLoading) return <div>加载中...</div>;
  if (isError) return <div>错误: {error.toString()}</div>;
  if (!data) return null;

  return (
    <div>
      <h2>{data.name || `地块 ${x},${y}`}</h2>
      <p>所有者: {data.owner}</p>
    </div>
  );
}
```

## 使用以下方式优化重新渲染 `selectFromResult`

将结果缩小到只包含你需要的字段：

```tsx
function ParcelOwner({ x, y }: { x: number; y: number }) {
  // ✅ 好做法：只订阅所有者字段的变化
  const { owner, isFetching } = useGetParcelByCoordsQuery(
    { x, y },
    {
      selectFromResult: ({ data, isFetching }) => ({
        owner: data?.owner,
        isFetching,
      }),
    }
  );

  if (isFetching) return <span>加载中...</span>;
  return <span>所有者: {owner}</span>;
}

// ❌ 不佳做法：任何数据变化都会重新渲染
function ParcelOwnerBad({ x, y }: { x: number; y: number }) {
  const { data } = useGetParcelByCoordsQuery({ x, y });
  return <span>所有者: {data?.owner}</span>;
}
```

## 使用 skip 的条件查询 `skip`

当参数尚未就绪时跳过查询：

```tsx
function UserProfile() {
  const { data: session } = useGetSessionQuery();
  const userId = session?.userId;

  // 在有 userId 之前跳过个人资料查询
  const { data: profile } = useGetProfileQuery(
    { id: userId! },
    {
      skip: !userId, // 如果 userId 未定义则不获取
    }
  );

  if (!userId) return <div>请登录</div>;
  if (!profile) return <div>正在加载个人资料...</div>;

  return <div>{profile.name}</div>;
}
```

## 用于实时更新的轮询

```tsx
function LiveBalance({ address }: { address: string }) {
  const { data } = useGetBalanceQuery(
    { address },
    {
      pollingInterval: 10000, // 每 10 秒轮询一次
      skipPollingIfUnfocused: true, // 当标签页未聚焦时暂停
    }
  );

  return <div>余额: {data?.amount ?? 0}</div>;
}
```

## 延迟查询

在组件挂载时不要自动触发，改为手动触发查询：

```tsx
function SearchParcels() {
  const [trigger, result] = useLazyGetParcelsByOwnerQuery();
  const [owner, setOwner] = useState('');

  const handleSearch = () => {
    if (owner) {
      trigger({ owner });
    }
  };

  return (
    <div>
      <input
        value={owner}
        onChange={(e) => setOwner(e.target.value)}
        placeholder="输入所有者地址"
      />
      <button onClick={handleSearch}>搜索</button>
      
      {result.isLoading && <div>搜索中...</div>}
      {result.data && (
        <ul>
          {result.data.map((parcel) => (
            <li key={parcel.id}>{parcel.name}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

## Mutation 用法

### 基本 Mutation

```tsx
function GrantCreditsButton({ address }: { address: string }) {
  const [grantCredits, { isLoading, isError, error }] = useGrantCreditsMutation();

  const handleGrant = async () => {
    try {
      await grantCredits({ address, amount: 100 }).unwrap();
      toast.success('积分授予成功！');
    } catch (err) {
      toast.error('授予积分失败');
      console.error('授予失败：', err);
    }
  };

  return (
    <button onClick={handleGrant} disabled={isLoading}>
      {isLoading ? '正在授予...' : '授予 100 积分'}
    </button>
  );
}
```

### 带有反馈的 Mutation

```tsx
import { isFetchBaseQueryError } from '@/services/client';

function UpdateParcelName({ parcelId }: { parcelId: string }) {
  const [name, setName] = useState('');
  const [updateName, { isLoading, isSuccess, isError, error }] =
    useUpdateParcelNameMutation();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    try {
      const result = await updateName({ id: parcelId, name }).unwrap();
      toast.success(`地块已重命名为 "${result.name}"`);
      setName(''); // 清空表单
    } catch (err) {
      if (isFetchBaseQueryError(err)) {
        toast.error(`错误: ${JSON.stringify(err.data)}`);
      } else {
        toast.error('发生了意外错误');
      }
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="新名称"
        disabled={isLoading}
      />
      <button type="submit" disabled={isLoading || !name}>
        {isLoading ? '正在更新...' : '更新名称'}
      </button>
      
      {isSuccess && <p className="success">名称已更新！</p>}
      {isError && <p className="error">更新失败</p>}
    </form>
  );
}
```

## 使用切片状态

使用以下方式访问切片状态： `useAppSelector`:

```tsx
import { useAppSelector, useAppDispatch } from '@/app/hooks';
import { selectViewMode, viewModeChanged } from '@/features/ui/ui.slice';

function ViewModeToggle() {
  const dispatch = useAppDispatch();
  const viewMode = useAppSelector(selectViewMode);

  const toggleMode = () => {
    dispatch(viewModeChanged(viewMode === 'grid' ? 'list' : 'grid'));
  };

  return (
    <button onClick={toggleMode}>
      View: {viewMode === 'grid' ? '⊞ 网格' : '☰ 列表'}
    </button>
  );
}
```

## 组合多个查询

```tsx
function ParcelDetails({ id }: { id: string }) {
  const { data: parcel, isLoading: parcelLoading } = useGetParcelQuery({ id });
  const { data: owner, isLoading: ownerLoading } = useGetProfileQuery(
    { id: parcel?.owner! },
    { skip: !parcel?.owner }
  );

  const isLoading = parcelLoading || ownerLoading;

  if (isLoading) return <div>加载中...</div>;
  if (!parcel) return <div>未找到地块</div>;

  return (
    <div>
      <h1>{parcel.name}</h1>
      <p>坐标: {parcel.x}, {parcel.y}</p>
      {owner && <p>所有者: {owner.name}</p>}
    </div>
  );
}
```

## 使用选择器的派生状态

```tsx
import { useAppSelector } from '@/app/hooks';
import { selectFilteredParcels, selectActiveFiltersCount } from '@/features/land/land.selectors';

function ParcelList() {
  const parcels = useAppSelector(selectFilteredParcels);
  const filterCount = useAppSelector(selectActiveFiltersCount);

  return (
    <div>
      <h2>
        地块（{parcels.length}）
        {filterCount > 0 && ` - ${filterCount} 个筛选器已启用`}
      </h2>
      <ul>
        {parcels.map((parcel) => (
          <li key={parcel.id}>{parcel.name}</li>
        ))}
      </ul>
    </div>
  );
}
```

## 分发操作

```tsx
import { useAppDispatch } from '@/app/hooks';
import { modalOpened, modalClosed } from '@/features/ui/ui.slice';

function TransferButton({ parcelId }: { parcelId: string }) {
  const dispatch = useAppDispatch();

  const handleClick = () => {
    // 分发操作以打开模态框
    dispatch(modalOpened());
  };

  return <button onClick={handleClick}>转移地块</button>;
}
```

## 实体适配器选择器

```tsx
import { useAppSelector } from '@/app/hooks';
import { creditsSelectors, selectTotalCredits } from '@/features/credits/credits.slice';

function CreditsSummary() {
  // 获取所有交易
  const allTxs = useAppSelector(creditsSelectors.selectAll);
  
  // 获取特定交易
  const txId = 'tx-123';
  const tx = useAppSelector((state) => 
    creditsSelectors.selectById(state, txId)
  );
  
  // 获取总数
  const count = useAppSelector(creditsSelectors.selectTotal);
  
  // 获取派生值
  const total = useAppSelector(selectTotalCredits);

  return (
    <div>
      <p>总积分: {total}</p>
      <p>交易数量: {count}</p>
      <ul>
        {allTxs.map((tx) => (
          <li key={tx.id}>
            {tx.type === 'grant' ? '+' : '-'}
            {tx.amount} - {tx.description}
          </li>
        ))}
      </ul>
    </div>
  );
}
```

## 预取数据

在导航前预取数据，以获得更快的用户体验：

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';
import { Link } from 'react-router-dom';

function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // 悬停时预取地块详情
    dispatch(
      client.util.prefetch('getParcel', { id: parcel.id }, { force: false })
    );
  };

  return (
    <Link
      to={`/parcels/${parcel.id}`}
      onMouseEnter={handleMouseEnter}
    >
      {parcel.name}
    </Link>
  );
}
```

## 手动缓存管理

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';

function RefreshButton() {
  const dispatch = useAppDispatch();

  const handleRefresh = () => {
    // 使所有 Parcels 查询失效
    dispatch(client.util.invalidateTags(['Parcels']));
    
    // 或重置整个客户端状态
    // dispatch(client.util.resetApiState());
  };

  return <button onClick={handleRefresh}>刷新数据</button>;
}
```

## 处理查询状态

```tsx
function ComprehensiveExample({ id }: { id: string }) {
  const {
    data,
    isLoading,      // 初始加载
    isFetching,     // 任何获取（包括重新获取）
    isSuccess,      // 查询成功
    isError,        // 查询失败
    error,          // 错误对象
    refetch,        // 手动重新获取函数
  } = useGetParcelQuery({ id });

  // 不同状态
  if (isLoading) {
    return <Spinner />;
  }

  if (isError) {
    return (
      <div>
        <p>错误: {error.toString()}</p>
        <button onClick={() => refetch()}>重试</button>
      </div>
    );
  }

  if (!data) {
    return <div>暂无数据</div>;
  }

  return (
    <div>
      {isFetching && <div className="refetch-indicator">正在更新...</div>}
      <h1>{data.name}</h1>
      <button onClick={() => refetch()}>刷新</button>
    </div>
  );
}
```

## 表单集成

```tsx
import { useState } from 'react';
import { useAppDispatch } from '@/app/hooks';
import { filtersUpdated } from '@/features/land/land.slice';

function FilterForm() {
  const dispatch = useAppDispatch();
  const [owner, setOwner] = useState('');
  const [minPrice, setMinPrice] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    
    dispatch(filtersUpdated({
      owner: owner || undefined,
      minPrice: minPrice ? parseInt(minPrice, 10) : undefined,
    }));
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="text"
        value={owner}
        onChange={(e) => setOwner(e.target.value)}
        placeholder="所有者地址"
      />
      <input
        type="number"
        value={minPrice}
        onChange={(e) => setMinPrice(e.target.value)}
        placeholder="最低价格"
      />
      <button type="submit">应用筛选</button>
    </form>
  );
}
```

## 最佳实践

### 1. 使用类型化的 Hooks

```tsx
// ✅ 好做法：使用类型化的 hooks
import { useAppSelector, useAppDispatch } from '@/app/hooks';

const value = useAppSelector(selectValue);
const dispatch = useAppDispatch();

// ❌ 不佳做法：使用未类型化的 hooks
import { useSelector, useDispatch } from 'react-redux';

const value = useSelector((state: RootState) => state.value);
const dispatch = useDispatch();
```

### 2. 处理所有查询状态

```tsx
// ✅ 好做法：全面处理状态
const { data, isLoading, isError, error } = useQuery(args);
if (isLoading) return <Loading />;
if (isError) return <Error error={error} />;
if (!data) return null;
return <Content data={data} />;

// ❌ 不佳做法：缺少错误处理
const { data } = useQuery(args);
return <Content data={data} />; // 可能会崩溃
```

### 3. 在 Mutation 中使用 `.unwrap()` 用于 Mutation

```tsx
// ✅ 好做法：显式错误处理
try {
  const result = await mutation(args).unwrap();
  toast.success('成功！');
} catch (error) {
  toast.error('失败！');
}

// ❌ 不佳做法：没有错误处理
mutation(args);
```

### 4. 使用...缩小结果 `selectFromResult`

```tsx
// ✅ 好做法：只订阅所需字段
const { name } = useQuery(args, {
  selectFromResult: ({ data }) => ({ name: data?.name }),
});

// ❌ 不佳做法：订阅整个对象
const { data } = useQuery(args);
const name = data?.name;
```

### 5. 避免选择整个切片

```tsx
// ✅ 好：选择特定值
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ 不佳做法：选择整个切片
const ui = useAppSelector((state) => state.ui);
const viewMode = ui.viewMode;
```

## 下一步

* 了解 [Web3 集成](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/web3-integration/) 用于区块链模式
* 回顾 [测试与性能](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/testing-and-performance/) 用于优化
* 查看 [RTK Query](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/rtk-query/) 用于高级端点配置


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/component-patterns.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
