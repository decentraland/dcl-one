# 状态管理

本页介绍如何使用 Redux Toolkit 的为 UI 和本地状态管理创建 Redux slice `createSlice` 以及 `createEntityAdapter`.

## 何时使用 Slice，何时使用 RTK Query

为你的状态选择合适的工具：

| 状态类型               | 工具                  | 示例                |
| ------------------ | ------------------- | ----------------- |
| **远程数据** （由服务器拥有）  | RTK Query           | 用户资料、NFT、目录项、订单   |
| **UI 状态** （由客户端拥有） | createSlice         | 筛选器、模态框、视图偏好、表单状态 |
| **规范化集合**          | createEntityAdapter | 已排序/筛选的列表、乐观更新    |


> [!WARNING]
> **不要在 slice 和 RTK Query 中** 重复相同的数据。请选择单一事实来源。


## 创建基础 Slice

为简单的 UI 状态使用 `createSlice` ：

```tsx
// src/features/ui/ui.slice.ts
import { createSlice, PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';

interface UIState {
  sidebarOpen: boolean;
  modalOpen: boolean;
  viewMode: 'grid' | 'list';
  theme: 'light' | 'dark';
}

const initialState: UIState = {
  sidebarOpen: true,
  modalOpen: false,
  viewMode: 'grid',
  theme: 'light',
};

const uiSlice = createSlice({
  name: 'ui',
  initialState,
  reducers: {
    // 布尔切换
    sidebarToggled(state) {
      state.sidebarOpen = !state.sidebarOpen;
    },
    
    // 设置特定值
    modalOpened(state) {
      state.modalOpen = true;
    },
    
    modalClosed(state) {
      state.modalOpen = false;
    },
    
    // 负载动作
    viewModeChanged(state, action: PayloadAction<'grid' | 'list'>) {
      state.viewMode = action.payload;
    },
    
    themeChanged(state, action: PayloadAction<'light' | 'dark'>) {
      state.theme = action.payload;
    },
    
    // 多个属性
    uiReset() {
      return initialState;
    },
  },
});

// 导出动作
export const {
  sidebarToggled,
  modalOpened,
  modalClosed,
  viewModeChanged,
  themeChanged,
  uiReset,
} = uiSlice.actions;

// 导出 reducer
export default uiSlice.reducer;

// 导出选择器
export const selectSidebarOpen = (state: RootState) => state.ui.sidebarOpen;
export const selectModalOpen = (state: RootState) => state.ui.modalOpen;
export const selectViewMode = (state: RootState) => state.ui.viewMode;
export const selectTheme = (state: RootState) => state.ui.theme;
```

## 使用 Entity Adapter

对于规范化集合（带 ID 的列表），请使用 `createEntityAdapter`:

```tsx
// src/features/credits/credits.slice.ts
import { createSlice, createEntityAdapter, PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';

export type CreditTransaction = {
  id: string;
  address: string;
  amount: number;
  type: 'grant' | 'spend';
  timestamp: number;
  description?: string;
};

// 创建实体适配器
const txAdapter = createEntityAdapter<CreditTransaction>({
  selectId: (tx) => tx.id,
  sortComparer: (a, b) => b.timestamp - a.timestamp, // 最新优先
});

// 使用适配器的初始状态创建 slice
const creditsSlice = createSlice({
  name: 'credits',
  initialState: txAdapter.getInitialState({
    sending: false,
    error: null as string | null,
  }),
  reducers: {
    // 添加单个交易
    txAdded: txAdapter.addOne,
    
    // 添加多个交易
    txsAdded: txAdapter.addMany,
    
    // 更新交易
    txUpdated: txAdapter.updateOne,
    
    // 删除交易
    txRemoved: txAdapter.removeOne,
    
    // 清除所有交易
    txsCleared: txAdapter.removeAll,
    
    // 带额外状态的自定义 reducer
    sendingStarted(state) {
      state.sending = true;
      state.error = null;
    },
    
    sendingSucceeded(state, action: PayloadAction<CreditTransaction>) {
      state.sending = false;
      txAdapter.addOne(state, action.payload);
    },
    
    sendingFailed(state, action: PayloadAction<string>) {
      state.sending = false;
      state.error = action.payload;
    },
  },
});

// 导出动作
export const {
  txAdded,
  txsAdded,
  txUpdated,
  txRemoved,
  txsCleared,
  sendingStarted,
  sendingSucceeded,
  sendingFailed,
} = creditsSlice.actions;

// 导出 reducer
export default creditsSlice.reducer;

// 创建选择器
const selectCreditsState = (state: RootState) => state.credits;

export const creditsSelectors = txAdapter.getSelectors(selectCreditsState);

// 额外的自定义选择器
export const selectIsSending = (state: RootState) => state.credits.sending;
export const selectError = (state: RootState) => state.credits.error;

// 记忆化选择器
export const selectTotalCredits = (state: RootState) => {
  const txs = creditsSelectors.selectAll(state);
  return txs.reduce((total, tx) => {
    return total + (tx.type === 'grant' ? tx.amount : -tx.amount);
  }, 0);
};
```

## 实体适配器方法

### 状态变更

```tsx
// 添加
txAdapter.addOne(state, entity)
txAdapter.addMany(state, entities)

// 更新
txAdapter.updateOne(state, { id, changes })
txAdapter.updateMany(state, updates)

// Upsert（添加或更新）
txAdapter.upsertOne(state, entity)
txAdapter.upsertMany(state, entities)

// 删除
txAdapter.removeOne(state, id)
txAdapter.removeMany(state, ids)
txAdapter.removeAll(state)

// 设置（替换全部）
txAdapter.setAll(state, entities)
txAdapter.setOne(state, entity)
txAdapter.setMany(state, entities)
```

### 生成的选择器

```tsx
const selectors = txAdapter.getSelectors(selectState);

// 将所有实体选为数组
selectors.selectAll(state)

// 将实体选为 { [id]: entity }
selectors.selectEntities(state)

// 将所有 ID 选为数组
selectors.selectIds(state)

// 选择总数
selectors.selectTotal(state)

// 通过 ID 选择单个实体
selectors.selectById(state, id)
```

## 复杂状态示例

在一个 slice 中组合多个关注点：

```tsx
// src/features/land/land.slice.ts
import { createSlice, createEntityAdapter, PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';

export type LandFilter = {
  owner?: string;
  minPrice?: number;
  maxPrice?: number;
  types?: ('parcel' | 'estate')[];
};

export type SelectedParcel = {
  x: number;
  y: number;
  id?: string;
};

const selectedParcelsAdapter = createEntityAdapter<SelectedParcel>({
  selectId: (p) => `${p.x},${p.y}`,
});

interface LandState {
  // 视图状态
  mapCenter: { x: number; y: number };
  mapZoom: number;
  
  // 筛选状态
  filters: LandFilter;
  
  // 选择状态（使用适配器）
  selectedParcels: ReturnType<typeof selectedParcelsAdapter.getInitialState>;
  
  // UI 状态
  showGrid: boolean;
  highlightOwned: boolean;
}

const initialState: LandState = {
  mapCenter: { x: 0, y: 0 },
  mapZoom: 1,
  filters: {},
  selectedParcels: selectedParcelsAdapter.getInitialState(),
  showGrid: true,
  highlightOwned: false,
};

const landSlice = createSlice({
  name: 'land',
  initialState,
  reducers: {
    // 地图控制
    mapCenterChanged(state, action: PayloadAction<{ x: number; y: number }>) {
      state.mapCenter = action.payload;
    },
    
    mapZoomed(state, action: PayloadAction<number>) {
      state.mapZoom = action.payload;
    },
    
    // 筛选器
    filtersUpdated(state, action: PayloadAction<Partial<LandFilter>>) {
      state.filters = { ...state.filters, ...action.payload };
    },
    
    filtersCleared(state) {
      state.filters = {};
    },
    
    // 选择
    parcelSelected(state, action: PayloadAction<SelectedParcel>) {
      selectedParcelsAdapter.addOne(state.selectedParcels, action.payload);
    },
    
    parcelDeselected(state, action: PayloadAction<string>) {
      selectedParcelsAdapter.removeOne(state.selectedParcels, action.payload);
    },
    
    selectionCleared(state) {
      selectedParcelsAdapter.removeAll(state.selectedParcels);
    },
    
    // UI 切换
    gridToggled(state) {
      state.showGrid = !state.showGrid;
    },
    
    ownedHighlightToggled(state) {
      state.highlightOwned = !state.highlightOwned;
    },
  },
});

export const {
  mapCenterChanged,
  mapZoomed,
  filtersUpdated,
  filtersCleared,
  parcelSelected,
  parcelDeselected,
  selectionCleared,
  gridToggled,
  ownedHighlightToggled,
} = landSlice.actions;

export default landSlice.reducer;

// 选择器
export const selectMapCenter = (state: RootState) => state.land.mapCenter;
export const selectMapZoom = (state: RootState) => state.land.mapZoom;
export const selectFilters = (state: RootState) => state.land.filters;
export const selectShowGrid = (state: RootState) => state.land.showGrid;
export const selectHighlightOwned = (state: RootState) => state.land.highlightOwned;

// 选择器
const selectSelectedParcelsState = (state: RootState) => state.land.selectedParcels;
export const selectedParcelsSelectors = selectedParcelsAdapter.getSelectors(
  selectSelectedParcelsState
);
```

## 记忆化选择器

为简单的 UI 状态使用 `createSelector` 来自 Reselect，用于计算/派生状态：

```tsx
// src/features/land/land.selectors.ts
import { createSelector } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';
import { selectFilters } from './land.slice';

// 代价高昂的筛选逻辑 - 记忆化
export const selectActiveFiltersCount = createSelector(
  [selectFilters],
  (filters) => {
    return Object.values(filters).filter(Boolean).length;
  }
);

// 组合多个选择器
export const selectHasActiveFilters = createSelector(
  [selectActiveFiltersCount],
  (count) => count > 0
);

// 多个输入
export const selectFilteredParcels = createSelector(
  [
    (state: RootState) => state.land.allParcels, // 假设它存在
    selectFilters,
  ],
  (parcels, filters) => {
    return parcels.filter((parcel) => {
      if (filters.owner && parcel.owner !== filters.owner) return false;
      if (filters.minPrice && parcel.price < filters.minPrice) return false;
      if (filters.maxPrice && parcel.price > filters.maxPrice) return false;
      if (filters.types && !filters.types.includes(parcel.type)) return false;
      return true;
    });
  }
);
```

## 使用 extraReducers 处理异步逻辑

在你的 slice 中处理 RTK Query 或异步 thunk 的响应：

```tsx
import { createSlice } from '@reduxjs/toolkit';
import { creditsClient } from './credits.client';

const slice = createSlice({
  name: 'credits',
  initialState: { lastGranted: null as number | null },
  reducers: {},
  extraReducers: (builder) => {
    builder
      .addMatcher(
        creditsClient.endpoints.grantCredits.matchFulfilled,
        (state, action) => {
          state.lastGranted = action.payload.newBalance;
        }
      )
      .addMatcher(
        creditsClient.endpoints.grantCredits.matchRejected,
        (state) => {
          state.lastGranted = null;
        }
      );
  },
});
```

## 最佳实践

### 1. 保持状态最小化

```tsx
// ✅ 好：只存储你需要的内容
interface State {
  userId: string | null;
  isAuthenticated: boolean;
}

// ❌ 坏：存储派生/计算值
interface State {
  userId: string | null;
  isAuthenticated: boolean;
  hasUserId: boolean; // 可以计算得出
  userIdLength: number; // 可以计算得出
}
```

### 2. 使用适合 Immer 的变更方式

```tsx
// ✅ 好：直接修改（Immer 会处理）
reducers: {
  itemAdded(state, action) {
    state.items.push(action.payload);
    state.count += 1;
  }
}

// ❌ 坏：手动展开（不必要）
reducers: {
  itemAdded(state, action) {
    return {
      ...state,
      items: [...state.items, action.payload],
      count: state.count + 1,
    };
  }
}
```

### 3. 逻辑地组织 Reducer

```tsx
// ✅ 好：按功能分组
reducers: {
  // 模态框控制
  modalOpened(state) { ... },
  modalClosed(state) { ... },
  
  // 筛选器控制
  filterApplied(state, action) { ... },
  filterCleared(state) { ... },
  
  // 重置
  stateReset() { return initialState; },
}
```

### 4. 正确为 Action 添加类型

```tsx
// ✅ 好：显式的负载类型
userUpdated(state, action: PayloadAction<{ id: string; name: string }>) {
  state.user = action.payload;
}

// ❌ 差：未指定类型的负载
userUpdated(state, action) {
  state.user = action.payload; // 没有类型安全
}
```

## 测试 Slice

```tsx
// land.slice.test.ts
import reducer, { mapCenterChanged, mapZoomed } from './land.slice';

describe('land slice', () => {
  const initialState = {
    mapCenter: { x: 0, y: 0 },
    mapZoom: 1,
    // ... 其他状态
  };

  it('应该处理 mapCenterChanged', () => {
    const newCenter = { x: 10, y: 20 };
    const actual = reducer(initialState, mapCenterChanged(newCenter));
    expect(actual.mapCenter).toEqual(newCenter);
  });

  it('应该处理 mapZoomed', () => {
    const actual = reducer(initialState, mapZoomed(2));
    expect(actual.mapZoom).toBe(2);
  });
});
```

## 下一步

* 回顾 [组件模式](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/component-patterns/) 用于在组件中使用 slice
* 了解 [Web3 集成](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/web3-integration/) 用于区块链状态
* 查看 [测试与性能](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/testing-and-performance/) 获取优化技巧


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/state-management.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
