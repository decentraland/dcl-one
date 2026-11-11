# 테스트 및 성능

이 페이지에서는 Redux/RTK Query 애플리케이션의 테스트 전략과 성능 최적화 기법을 다룹니다.

## Redux 슬라이스 테스트

### 기본 리듀서 테스트

```tsx
// user.slice.test.ts
import reducer, { userLoggedIn, userLoggedOut } from './user.slice';

describe('user 슬라이스', () => {
  const initialState = {
    account: null,
    isAuthenticated: false,
  };

  it('userLoggedIn을 처리해야 함', () => {
    const account = '0x123...';
    const actual = reducer(initialState, userLoggedIn({ account }));
    
    expect(actual.account).toBe(account);
    expect(actual.isAuthenticated).toBe(true);
  });

  it('userLoggedOut을 처리해야 함', () => {
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

### 엔티티 어댑터 테스트

```tsx
// credits.slice.test.ts
import reducer, { txAdded, txsCleared, creditsSelectors } from './credits.slice';

describe('엔티티 어댑터가 있는 credits 슬라이스', () => {
  it('거래를 추가해야 함', () => {
    const initialState = reducer(undefined, { type: 'unknown' });
    const tx = { id: '1', address: '0x123', amount: 100, type: 'grant', timestamp: Date.now() };
    
    const actual = reducer(initialState, txAdded(tx));
    
    expect(creditsSelectors.selectById({ credits: actual }, '1')).toEqual(tx);
    expect(creditsSelectors.selectTotal({ credits: actual })).toBe(1);
  });

  it('모든 거래를 지워야 함', () => {
    const initialState = reducer(undefined, { type: 'unknown' });
    const withTx = reducer(initialState, txAdded({ id: '1', /* ... */ }));
    
    const actual = reducer(withTx, txsCleared());
    
    expect(creditsSelectors.selectTotal({ credits: actual })).toBe(0);
  });
});
```

## 셀렉터 테스트

### 단순 셀렉터

```tsx
// user.selectors.test.ts
import { selectAccount, selectIsAuthenticated } from './user.slice';

describe('user 셀렉터', () => {
  const mockState = {
    user: {
      account: '0x123...',
      isAuthenticated: true,
    },
    // ... 다른 slice들
  };

  it('계정을 선택해야 함', () => {
    expect(selectAccount(mockState)).toBe('0x123...');
  });

  it('인증 상태를 선택해야 함', () => {
    expect(selectIsAuthenticated(mockState)).toBe(true);
  });
});
```

### 메모이제이션된 셀렉터

```tsx
// land.selectors.test.ts
import { selectFilteredParcels, selectActiveFiltersCount } from './land.selectors';

describe('land 셀렉터', () => {
  const mockState = {
    land: {
      filters: { owner: '0x123', minPrice: 100 },
      parcels: [
        { id: '1', owner: '0x123', price: 150 },
        { id: '2', owner: '0x456', price: 200 },
      ],
    },
  };

  it('활성 필터 수를 세어야 함', () => {
    expect(selectActiveFiltersCount(mockState)).toBe(2);
  });

  it('필지를 필터링해야 함', () => {
    const result = selectFilteredParcels(mockState);
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('1');
  });

  it('결과를 메모이제이션해야 함', () => {
    const result1 = selectFilteredParcels(mockState);
    const result2 = selectFilteredParcels(mockState);
    
    // 동일한 참조 = 메모이제이션됨
    expect(result1).toBe(result2);
  });
});
```

## MSW로 RTK Query 테스트

### MSW 설정

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
      name: '테스트 필지',
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

### 쿼리 테스트

```tsx
// land.client.test.tsx
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { setupStore } from '@/app/store';
import { useGetParcelQuery } from './land.client';

function TestComponent({ id }: { id: string }) {
  const { data, isLoading, isError } = useGetParcelQuery({ id });

  if (isLoading) return <div>로딩 중...</div>;
  if (isError) return <div>오류</div>;
  if (!data) return null;

  return <div>{data.name}</div>;
}

describe('land 클라이언트', () => {
  it('필지 데이터를 가져와 표시해야 함', async () => {
    const store = setupStore();

    render(
      <Provider store={store}>
        <TestComponent id="1" />
      </Provider>
    );

    expect(screen.getByText('로딩 중...')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('테스트 필지')).toBeInTheDocument();
    });
  });
});
```

### 뮤테이션 테스트

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
        지급
      </button>
      {isLoading && <div>로딩 중...</div>}
      {isSuccess && <div>성공!</div>}
    </div>
  );
}

describe('credits 클라이언트 뮤테이션', () => {
  it('크레딧을 성공적으로 지급해야 함', async () => {
    const store = setupStore();

    render(
      <Provider store={store}>
        <TestComponent />
      </Provider>
    );

    fireEvent.click(screen.getByText('지급'));

    await waitFor(() => {
      expect(screen.getByText('성공!')).toBeInTheDocument();
    });
  });
});
```

### 낙관적 업데이트 테스트

```tsx
// credits.client.test.ts
import { server } from '@/test/server';
import { http, HttpResponse } from 'msw';
import { setupStore } from '@/app/store';
import { creditsClient } from './credits.client';

describe('낙관적 업데이트', () => {
  it('캐시를 낙관적으로 업데이트하고 오류 시 롤백해야 함', async () => {
    const store = setupStore();
    const address = '0x123';

    // 초기 잔액 프리페치
    await store.dispatch(
      creditsClient.endpoints.getBalance.initiate({ address })
    );

    const initialBalance = creditsClient.endpoints.getBalance.select({ address })(
      store.getState()
    ).data?.amount;

    expect(initialBalance).toBe(100); // MSW 핸들러에서

    // 실패를 모의
    server.use(
      http.post('/api/v1/credits/grant', () => {
        return HttpResponse.json({ error: 'Failed' }, { status: 500 });
      })
    );

    // 뮤테이션 트리거
    const mutation = store.dispatch(
      creditsClient.endpoints.grantCredits.initiate({
        address,
        amount: 50,
      })
    );

    // 낙관적 업데이트 확인
    const optimisticBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(optimisticBalance).toBe(150); // 100 + 50

    // 뮤테이션 실패를 기다림
    await expect(mutation).rejects.toThrow();

    // 롤백 확인
    const rolledBackBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(rolledBackBalance).toBe(100); // 원래 상태로 복귀
  });
});
```

## 성능 최적화

### 다음 `selectFromResult` 리렌더링 방지용

```tsx
// ✅ 좋음: 특정 필드만 구독
const { owner } = useGetParcelQuery(
  { id },
  {
    selectFromResult: ({ data }) => ({
      owner: data?.owner,
    }),
  }
);

// 컴포넌트는 owner가 변경될 때만 리렌더링됨
```

### 전체 상태 선택 피하기

```tsx
// ✅ 좋음: 특정 값 선택
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ 나쁨: 전체 slice를 선택함
const ui = useAppSelector((state) => state.ui);
```

### 비용이 큰 셀렉터 메모이제이션

```tsx
import { createSelector } from '@reduxjs/toolkit';

// ✅ 좋음: 메모이제이션된 셀렉터
export const selectFilteredParcels = createSelector(
  [selectAllParcels, selectFilters],
  (parcels, filters) => {
    // 비용이 많이 드는 필터링 로직
    return parcels.filter(/* ... */);
  }
);

// ❌ 나쁨: 컴포넌트에서 계산함
function Component() {
  const parcels = useAppSelector(selectAllParcels);
  const filters = useAppSelector(selectFilters);
  
  // 매 렌더링마다 다시 계산됨!
  const filtered = parcels.filter(/* ... */);
}
```

### 정규화된 데이터에는 엔티티 어댑터 사용

```tsx
// ✅ 좋음: 엔티티 어댑터로 정규화
const adapter = createEntityAdapter<Parcel>();

// ID 기반의 효율적인 조회
const parcel = adapter.getSelectors().selectById(state, id);

// ❌ 나쁨: 배열 조회
const parcel = state.parcels.find((p) => p.id === id);
```

### RTK Query 캐시 설정 조정

```tsx
export const client = createApi({
  // ...
  keepUnusedDataFor: 60, // 데이터를 60초 동안 유지
  refetchOnMountOrArgChange: 30, // 데이터가 30초보다 오래되면 다시 가져오기
  refetchOnFocus: true, // 창에 포커스가 돌아오면 다시 가져오기
  refetchOnReconnect: true, // 다시 연결되면 다시 가져오기
});
```

### 더 나은 UX를 위한 프리페치

```tsx
function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // 마우스 오버 시 프리페치
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

### 폴링 전략

```tsx
// 필요할 때만 폴링
const { data } = useGetBalanceQuery(
  { address },
  {
    pollingInterval: isActive ? 10000 : 0, // 활성화되었을 때만 폴링
    skipPollingIfUnfocused: true, // 탭에 포커스가 없으면 일시 중지
  }
);
```

## Redux DevTools

### 개발 환경에서 활성화

```tsx
export const store = configureStore({
  // ...
  devTools: process.env.NODE_ENV !== 'production',
});
```

### 액션 정제기

DevTools에서 민감한 데이터 정제:

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

## 모범 사례 체크리스트

### 성능

* [ ] 다음 `selectFromResult` 대용량 쿼리 결과용
* [ ] 비용이 큰 셀렉터를 `createSelector`
* [ ] 정규화된 컬렉션에는 엔티티 어댑터 사용
* [ ] 컴포넌트에서 전체 slice 선택 피하기
* [ ] 조정 `keepUnusedDataFor` 사용 사례에 맞게
* [ ] 탐색 전에 데이터 프리페치
* [ ] 폴링은 전략적으로 사용(필요할 때만)

### 테스트

* [ ] 모든 리듀서와 액션에 대해 단위 테스트 작성
* [ ] 메모이제이션된 셀렉터의 정확성과 성능 테스트
* [ ] RTK Query 엔드포인트 테스트에 MSW 사용
* [ ] 낙관적 업데이트와 롤백 로직 테스트
* [ ] 컴포넌트의 오류 처리 테스트
* [ ] 핵심 흐름에 대한 통합 테스트 작성

### 코드 품질

* [ ] 타입이 지정된 훅 사용 (`useAppSelector`, `useAppDispatch`)
* [ ] 모든 쿼리 상태(로딩, 오류, 성공) 처리
* [ ] 다음 `.unwrap()` 뮤테이션 오류 처리를 위해
* [ ] 뮤테이션 후 캐시 무효화 또는 업데이트
* [ ] 직렬화 불가능한 데이터를 Redux 밖에 두기
* [ ] 복잡한 셀렉터와 로직 문서화

## 피해야 할 안티패턴


> [!CAUTION]
> **다음은 하지 마세요:**
> 
> 1. 직렬화 불가능한 객체(provider, signer)를 Redux에 저장
> 2. slice와 RTK Query 양쪽에 데이터를 중복 저장
> 3. 렌더링 중 액션 디스패치
> 4. 메모이제이션 없이 새 객체를 반환하는 셀렉터 생성
> 5. 로딩 및 오류 상태 무시
> 6. RTK Query 없이 여러 컴포넌트에서 동일한 데이터 가져오기
> 7. 과도하게 폴링하거나 `skipPollingIfUnfocused`


## 성능 모니터링

### 셀렉터 호출 추적

```tsx
import { createSelector } from '@reduxjs/toolkit';

const selectExpensiveData = createSelector(
  [selectData],
  (data) => {
    console.log('셀렉터 호출됨'); // 데이터가 변경될 때만 로그가 찍혀야 함
    return expensiveOperation(data);
  }
);
```

### 리렌더링 모니터링

```tsx
import { useEffect, useRef } from 'react';

function useRenderCount() {
  const renderCount = useRef(0);
  
  useEffect(() => {
    renderCount.current += 1;
    console.log('렌더링 횟수:', renderCount.current);
  });
}

function Component() {
  useRenderCount(); // 리렌더링 추적
  // ...
}
```

## 다음 단계

* 검토 [컴포넌트 패턴](/docs/contributor/contributor-ko/contributor-guides/ui/component-patterns/) 사용 예시용
* 참고 [RTK Query](/docs/contributor/contributor-ko/contributor-guides/ui/rtk-query/) 캐싱 전략용
* 이해하기 [상태 관리](/docs/contributor/contributor-ko/contributor-guides/ui/state-management/) slice 최적화용


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui/testing-and-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
