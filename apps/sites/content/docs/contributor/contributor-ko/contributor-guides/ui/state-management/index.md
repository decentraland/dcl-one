# 상태 관리

이 페이지에서는 UI 및 로컬 상태 관리를 위한 Redux 슬라이스를 만드는 방법을 Redux Toolkit의 `createSlice` 및 `createEntityAdapter`.

## 슬라이스를 사용할 시기와 RTK Query

상태에 맞는 올바른 도구를 선택하세요:

| 상태 유형                | 도구                  | 예시                        |
| -------------------- | ------------------- | ------------------------- |
| **원격 데이터** (서버 소유)   | RTK Query           | 사용자 프로필, NFT, 카탈로그 항목, 주문 |
| **UI 상태** (클라이언트 소유) | createSlice         | 필터, 모달, 보기 기본 설정, 폼 상태    |
| **정규화된 컬렉션**         | createEntityAdapter | 정렬/필터링된 목록, 낙관적 업데이트      |


> [!WARNING]
> **같은 데이터를 중복하지 마세요** 슬라이스와 RTK Query 양쪽에. 하나의 단일 진실 소스를 선택하세요.


## 기본 슬라이스 만들기

다음 `createSlice` 간단한 UI 상태에는:

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
    // 불리언 토글
    sidebarToggled(state) {
      state.sidebarOpen = !state.sidebarOpen;
    },
    
    // 특정 값 설정
    modalOpened(state) {
      state.modalOpen = true;
    },
    
    modalClosed(state) {
      state.modalOpen = false;
    },
    
    // 페이로드 액션
    viewModeChanged(state, action: PayloadAction<'grid' | 'list'>) {
      state.viewMode = action.payload;
    },
    
    themeChanged(state, action: PayloadAction<'light' | 'dark'>) {
      state.theme = action.payload;
    },
    
    // 여러 속성
    uiReset() {
      return initialState;
    },
  },
});

// 액션 내보내기
export const {
  sidebarToggled,
  modalOpened,
  modalClosed,
  viewModeChanged,
  themeChanged,
  uiReset,
} = uiSlice.actions;

// 리듀서 내보내기
export default uiSlice.reducer;

// 셀렉터 내보내기
export const selectSidebarOpen = (state: RootState) => state.ui.sidebarOpen;
export const selectModalOpen = (state: RootState) => state.ui.modalOpen;
export const selectViewMode = (state: RootState) => state.ui.viewMode;
export const selectTheme = (state: RootState) => state.ui.theme;
```

## 엔티티 어댑터 사용하기

정규화된 컬렉션(ID가 있는 목록)에는 다음을 사용하세요: `createEntityAdapter`:

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

// 엔티티 어댑터 생성
const txAdapter = createEntityAdapter<CreditTransaction>({
  selectId: (tx) => tx.id,
  sortComparer: (a, b) => b.timestamp - a.timestamp, // 최신순
});

// 어댑터의 초기 상태로 슬라이스 생성
const creditsSlice = createSlice({
  name: 'credits',
  initialState: txAdapter.getInitialState({
    sending: false,
    error: null as string | null,
  }),
  reducers: {
    // 단일 거래 추가
    txAdded: txAdapter.addOne,
    
    // 여러 거래 추가
    txsAdded: txAdapter.addMany,
    
    // 거래 업데이트
    txUpdated: txAdapter.updateOne,
    
    // 거래 제거
    txRemoved: txAdapter.removeOne,
    
    // 모든 거래 지우기
    txsCleared: txAdapter.removeAll,
    
    // 추가 상태가 있는 커스텀 리듀서
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

// 액션 내보내기
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

// 리듀서 내보내기
export default creditsSlice.reducer;

// 셀렉터 생성
const selectCreditsState = (state: RootState) => state.credits;

export const creditsSelectors = txAdapter.getSelectors(selectCreditsState);

// 추가 커스텀 셀렉터
export const selectIsSending = (state: RootState) => state.credits.sending;
export const selectError = (state: RootState) => state.credits.error;

// 메모이제이션된 셀렉터
export const selectTotalCredits = (state: RootState) => {
  const txs = creditsSelectors.selectAll(state);
  return txs.reduce((total, tx) => {
    return total + (tx.type === 'grant' ? tx.amount : -tx.amount);
  }, 0);
};
```

## 엔티티 어댑터 메서드

### 상태 변경

```tsx
// 추가
txAdapter.addOne(state, entity)
txAdapter.addMany(state, entities)

// 업데이트
txAdapter.updateOne(state, { id, changes })
txAdapter.updateMany(state, updates)

// 업서트(추가 또는 업데이트)
txAdapter.upsertOne(state, entity)
txAdapter.upsertMany(state, entities)

// 제거
txAdapter.removeOne(state, id)
txAdapter.removeMany(state, ids)
txAdapter.removeAll(state)

// 설정(모두 교체)
txAdapter.setAll(state, entities)
txAdapter.setOne(state, entity)
txAdapter.setMany(state, entities)
```

### 생성된 셀렉터

```tsx
const selectors = txAdapter.getSelectors(selectState);

// 모든 엔티티를 배열로 선택
selectors.selectAll(state)

// 엔티티를 { [id]: entity }로 선택
selectors.selectEntities(state)

// 모든 ID를 배열로 선택
selectors.selectIds(state)

// 총 개수 선택
selectors.selectTotal(state)

// ID로 단일 엔티티 선택
selectors.selectById(state, id)
```

## 복잡한 상태 예시

여러 관심사를 하나의 슬라이스로 결합:

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
  // 보기 상태
  mapCenter: { x: number; y: number };
  mapZoom: number;
  
  // 필터 상태
  filters: LandFilter;
  
  // 선택 상태(어댑터 사용)
  selectedParcels: ReturnType<typeof selectedParcelsAdapter.getInitialState>;
  
  // UI 상태
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
    // 맵 컨트롤
    mapCenterChanged(state, action: PayloadAction<{ x: number; y: number }>) {
      state.mapCenter = action.payload;
    },
    
    mapZoomed(state, action: PayloadAction<number>) {
      state.mapZoom = action.payload;
    },
    
    // 필터
    filtersUpdated(state, action: PayloadAction<Partial<LandFilter>>) {
      state.filters = { ...state.filters, ...action.payload };
    },
    
    filtersCleared(state) {
      state.filters = {};
    },
    
    // 선택
    parcelSelected(state, action: PayloadAction<SelectedParcel>) {
      selectedParcelsAdapter.addOne(state.selectedParcels, action.payload);
    },
    
    parcelDeselected(state, action: PayloadAction<string>) {
      selectedParcelsAdapter.removeOne(state.selectedParcels, action.payload);
    },
    
    selectionCleared(state) {
      selectedParcelsAdapter.removeAll(state.selectedParcels);
    },
    
    // UI 토글
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

// 셀렉터
export const selectMapCenter = (state: RootState) => state.land.mapCenter;
export const selectMapZoom = (state: RootState) => state.land.mapZoom;
export const selectFilters = (state: RootState) => state.land.filters;
export const selectShowGrid = (state: RootState) => state.land.showGrid;
export const selectHighlightOwned = (state: RootState) => state.land.highlightOwned;

// 선택 셀렉터
const selectSelectedParcelsState = (state: RootState) => state.land.selectedParcels;
export const selectedParcelsSelectors = selectedParcelsAdapter.getSelectors(
  selectSelectedParcelsState
);
```

## 메모이제이션된 셀렉터

다음 `createSelector` 를 사용하여 계산/파생 상태 생성:

```tsx
// src/features/land/land.selectors.ts
import { createSelector } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';
import { selectFilters } from './land.slice';

// 비용이 큰 필터링 로직 - 메모이제이션됨
export const selectActiveFiltersCount = createSelector(
  [selectFilters],
  (filters) => {
    return Object.values(filters).filter(Boolean).length;
  }
);

// 여러 셀렉터 결합
export const selectHasActiveFilters = createSelector(
  [selectActiveFiltersCount],
  (count) => count > 0
);

// 여러 입력
export const selectFilteredParcels = createSelector(
  [
    (state: RootState) => state.land.allParcels, // 이것이 있다고 가정
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

## 추가 리듀서로 비동기 로직 처리

슬라이스에서 RTK Query 또는 비동기 thunk 응답을 처리하세요:

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

## 모범 사례

### 1. 상태를 최소화하세요

```tsx
// ✅ 좋음: 필요한 것만 저장
interface State {
  userId: string | null;
  isAuthenticated: boolean;
}

// ❌ 나쁨: 파생/계산 값을 저장
interface State {
  userId: string | null;
  isAuthenticated: boolean;
  hasUserId: boolean; // 계산 가능
  userIdLength: number; // 계산 가능
}
```

### 2. Immer 친화적 변경 사용

```tsx
// ✅ 좋음: 직접 변경(Immer가 처리함)
reducers: {
  itemAdded(state, action) {
    state.items.push(action.payload);
    state.count += 1;
  }
}

// ❌ 나쁨: 수동 전개(불필요)
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

### 3. 리듀서를 논리적으로 구성하기

```tsx
// ✅ 좋음: 기능별로 그룹화
reducers: {
  // 모달 컨트롤
  modalOpened(state) { ... },
  modalClosed(state) { ... },
  
  // 필터 컨트롤
  filterApplied(state, action) { ... },
  filterCleared(state) { ... },
  
  // 초기화
  stateReset() { return initialState; },
}
```

### 4. 액션에 올바르게 타입 지정하기

```tsx
// ✅ 좋음: 명시적인 페이로드 타입
userUpdated(state, action: PayloadAction<{ id: string; name: string }>) {
  state.user = action.payload;
}

// ❌ 나쁨: 타입이 지정되지 않은 페이로드
userUpdated(state, action) {
  state.user = action.payload; // 타입 안전성 없음
}
```

## 슬라이스 테스트

```tsx
// land.slice.test.ts
import reducer, { mapCenterChanged, mapZoomed } from './land.slice';

describe('land 슬라이스', () => {
  const initialState = {
    mapCenter: { x: 0, y: 0 },
    mapZoom: 1,
    // ... 다른 상태
  };

  it('mapCenterChanged를 처리해야 함', () => {
    const newCenter = { x: 10, y: 20 };
    const actual = reducer(initialState, mapCenterChanged(newCenter));
    expect(actual.mapCenter).toEqual(newCenter);
  });

  it('mapZoomed를 처리해야 함', () => {
    const actual = reducer(initialState, mapZoomed(2));
    expect(actual.mapZoom).toBe(2);
  });
});
```

## 다음 단계

* 검토 [컴포넌트 패턴](/docs/contributor/contributor-ko/contributor-guides/ui/component-patterns/) 컴포넌트에서 슬라이스를 사용하는 방법
* 학습해 보세요 [Web3 통합](/docs/contributor/contributor-ko/contributor-guides/ui/web3-integration/) 블록체인 상태용
* 참고 [테스트 및 성능](/docs/contributor/contributor-ko/contributor-guides/ui/testing-and-performance/) 최적화 팁을 위해


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui/state-management.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
