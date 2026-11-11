# RTK Query

RTK Query는 Redux Toolkit의 강력한 데이터 가져오기 및 캐싱 레이어입니다. 비동기 액션 크리에이터를 작성할 필요를 없애주며, 로딩 상태, 캐싱, 데이터 동기화를 자동으로 관리합니다.

## 기본 클라이언트 구성

모든 RTK Query 엔드포인트는 단일 기본 클라이언트 인스턴스에서 확장됩니다.

### 기본 쿼리 설정

```tsx
// src/services/baseQuery.ts
import { fetchBaseQuery } from '@reduxjs/toolkit/query/react';
import type { RootState } from '@/app/store';

export const baseQuery = fetchBaseQuery({
  baseUrl: process.env.NEXT_PUBLIC_API_URL, // 예: https://api.decentraland.org
  
  prepareHeaders: (headers, { getState }) => {
    const state = getState() as RootState;
    
    // 인증 토큰 추가
    const token = state.user.session?.authToken;
    if (token) {
      headers.set('authorization', `Bearer ${token}`);
    }
    
    // web3 컨텍스트용 체인 ID 추가
    const chainId = state.user.chainId;
    if (chainId) {
      headers.set('x-chain-id', String(chainId));
    }
    
    // 표준 헤더
    headers.set('accept', 'application/json');
    headers.set('content-type', 'application/json');
    
    return headers;
  },
  
  credentials: 'omit', // 백엔드가 쿠키를 요구하면 'include'
});
```

### 클라이언트 인스턴스

```tsx
// src/services/client.ts
import { createApi } from '@reduxjs/toolkit/query/react';
import { baseQuery } from './baseQuery';

export const client = createApi({
  reducerPath: 'client',
  baseQuery,
  
  // 캐시 무효화를 위한 모든 가능한 태그 유형 정의
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
  
  // 캐시 구성
  keepUnusedDataFor: 60,         // 사용하지 않는 데이터를 60초간 유지
  refetchOnFocus: true,           // 창이 다시 포커스를 얻으면 다시 가져오기
  refetchOnReconnect: true,       // 다시 연결될 때 다시 가져오기
  refetchOnMountOrArgChange: 30,  // 데이터가 30초보다 오래되었으면 다시 가져오기
  
  // 엔드포인트는 기능 파일에 주입됩니다
  endpoints: () => ({}),
});
```

## 태그 규칙

태그는 캐시 무효화와 동기화에 사용됩니다. 다음 규칙을 따르세요:

### 태그 이름

| 리소스 유형      | 쿼리 태그                                | 뮤테이션 무효화                       |
| ----------- | ------------------------------------ | ------------------------------ |
| **컬렉션**     | `'Parcels'` (복수형)                    | `'Parcels'`                    |
| **단일 엔터티**  | `{type: 'Parcels', id: '123'}`       | `{type: 'Parcels', id: '123'}` |
| **목록 + 상세** | `['Parcels', {type: 'Parcels', id}]` | `['Parcels']` 또는 특정 id         |

### 태그 예시

```tsx
// 컬렉션: 목록 태그 제공
providesTags: ['Parcels']

// 단일 엔터티: 특정 태그 + 목록 태그 제공
providesTags: (result) => 
  result 
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// 뮤테이션: 목록과 특정 엔터티를 모두 무효화
invalidatesTags: (result, error, arg) => [
  { type: 'Parcels', id: arg.id },
  'Parcels'
]
```

## 엔드포인트 생성

엔드포인트는 반드시 다음 위치에서 기능과 함께 배치해야 합니다. `feature.client.ts` 파일들.

### 쿼리 엔드포인트(읽기)

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
    // 모든 타일 가져오기
    getTiles: build.query<Record<string, Tile>, void>({
      query: () => '/v1/tiles',
      providesTags: ['Parcels'],
    }),
    
    // 좌표로 parcel 가져오기
    getParcelByCoords: build.query<Parcel, { x: number; y: number }>({
      query: ({ x, y }) => `/v1/lands/${x}/${y}`,
      providesTags: (result, error, arg) =>
        result
          ? [{ type: 'Parcels', id: result.id }, 'Parcels']
          : ['Parcels'],
    }),
    
    // 소유자별 parcels 가져오기
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

// 훅 내보내기
export const {
  useGetTilesQuery,
  useGetParcelByCoordsQuery,
  useGetParcelsByOwnerQuery,
} = landClient;
```

### 뮤테이션 엔드포인트(쓰기)

```tsx
// src/features/land/land.client.ts (계속)
export const landClient = client.injectEndpoints({
  endpoints: (build) => ({
    // ... 쿼리 엔드포인트 ...
    
    // parcel 이름 업데이트
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
    
    // parcel 전송
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
        'Parcels', // 소유자 필터를 업데이트하기 위해 목록을 무효화
      ],
    }),
  }),
});

export const {
  useUpdateParcelNameMutation,
  useTransferParcelMutation,
} = landClient;
```

## 낙관적 업데이트

다음 `onQueryStarted` 실패 시 자동 롤백이 있는 낙관적 UI 업데이트용입니다.

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
      
      // 낙관적 업데이트
      async onQueryStarted({ address, amount }, { dispatch, queryFulfilled }) {
        // 캐시를 낙관적으로 업데이트
        const patchResult = dispatch(
          client.util.updateQueryData('getBalance', { address }, (draft) => {
            draft.amount += amount;
            draft.lastUpdated = new Date().toISOString();
          })
        );
        
        try {
          // 뮤테이션이 완료될 때까지 대기
          const { data } = await queryFulfilled;
          
          // 서버 응답으로 업데이트
          dispatch(
            client.util.updateQueryData('getBalance', { address }, (draft) => {
              draft.amount = data.newBalance;
            })
          );
        } catch {
          // 실패 시 롤백
          patchResult.undo();
        }
      },
      
      // 일관성을 보장하기 위해 또한 무효화
      invalidatesTags: (result, error, arg) => [
        { type: 'Credits', id: arg.address }
      ],
    }),
  }),
});

export const { useGetBalanceQuery, useGrantCreditsMutation } = creditsClient;
```

## 고급 쿼리 옵션

### 폴링

```tsx
// 10초마다 폴링
const { data } = useGetBalanceQuery(
  { address },
  { pollingInterval: 10000 }
);
```

### 쿼리 건너뛰기

```tsx
// address를 사용할 수 없으면 쿼리 건너뛰기
const { data } = useGetBalanceQuery(
  { address: address! },
  { skip: !address }
);
```

### 지연 쿼리

```tsx
const [trigger, result] = useLazyGetParcelByCoordsQuery();

// 수동으로 트리거
const handleClick = () => {
  trigger({ x: 10, y: 20 });
};
```

### 응답 변환

```tsx
getParcel: build.query<Parcel, string>({
  query: (id) => `/v1/lands/${id}`,
  transformResponse: (response: ApiResponse<Parcel>) => response.data,
})
```

### 사용자 정의 직렬화

페이지네이션이나 검색의 경우 캐시 키 직렬화를 사용자 정의하세요:

```tsx
searchParcels: build.query<Parcel[], { q: string; owner?: string; page?: number }>({
  query: (args) => ({
    url: '/v1/parcels/search',
    params: args,
  }),
  
  // 선택적 매개변수를 처리하기 위한 사용자 정의 캐시 키
  serializeQueryArgs: ({ endpointName, queryArgs }) => {
    const { q, owner = 'any', page = 1 } = queryArgs;
    return `${endpointName}-${q}-${owner}-${page}`;
  },
  
  // 페이지네이션 결과 병합
  merge(currentCache, newItems, { arg }) {
    if (arg.page === 1) {
      return newItems;
    }
    return [...currentCache, ...newItems];
  },
  
  // 인자가 변경되면 강제로 다시 가져오기
  forceRefetch({ currentArg, previousArg }) {
    return JSON.stringify(currentArg) !== JSON.stringify(previousArg);
  },
  
  providesTags: ['Parcels'],
})
```

## 오류 처리

### 사용자 정의 오류 처리

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

컴포넌트에서의 사용:

```tsx
const { data, error } = useGetParcelQuery({ id });

if (error) {
  if (isFetchBaseQueryError(error)) {
    const errMsg = 'error' in error ? error.error : JSON.stringify(error.data);
    return <div>오류: {errMsg}</div>;
  } else if (isErrorWithMessage(error)) {
    return <div>오류: {error.message}</div>;
  }
}
```

## 캐시 관리

### 수동 캐시 업데이트

```tsx
// 캐시를 직접 업데이트
dispatch(
  client.util.updateQueryData('getBalance', { address }, (draft) => {
    draft.amount = 1000;
  })
);
```

### 캐시 무효화

```tsx
// 모든 Credits 쿼리 무효화
dispatch(client.util.invalidateTags(['Credits']));

// 특정 엔터티 무효화
dispatch(client.util.invalidateTags([{ type: 'Credits', id: address }]));
```

### 클라이언트 상태 재설정

```tsx
// 전체 클라이언트 상태 재설정
dispatch(client.util.resetApiState());
```

### 데이터 미리 가져오기

```tsx
// 탐색 전에 데이터 미리 가져오기
dispatch(
  client.util.prefetch('getParcel', { id: '123' }, { force: false })
);
```

## 권장 사항

### 1. 설명적인 엔드포인트 이름 사용

```tsx
// ✅ 좋음
getParcelByCoords
getParcelsByOwner
updateParcelName

// ❌ 나쁨
getParcel
fetch
update
```

### 2. 포괄적인 태그 제공

```tsx
// ✅ 좋음: 목록 태그와 엔터티 태그를 모두 제공합니다
providesTags: (result) =>
  result
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// ❌ 나쁨: 목록 태그만 제공합니다
providesTags: ['Parcels']
```

### 3. 로딩 및 오류 상태 처리

```tsx
// ✅ 좋음: 완전한 상태 처리
const { data, isLoading, isFetching, isError, error } = useGetParcelQuery({ id });

if (isLoading) return <Spinner />;
if (isError) return <Error error={error} />;
if (!data) return null;

// ❌ 나쁨: 불완전한 상태 처리
const { data } = useGetParcelQuery({ id });
return <div>{data.name}</div>; // data가 undefined이면 충돌할 수 있음
```

### 4. 타입 가드 사용

```tsx
// ✅ 좋음: 타입 안전 오류 처리
if (isFetchBaseQueryError(error)) {
  // 가져오기 오류 처리
} else if (isErrorWithMessage(error)) {
  // 메시지가 있는 오류 처리
}

// ❌ 나쁨: 안전하지 않은 타입 캐스팅
const message = (error as any).message;
```

## 다음 단계

* 학습해 보세요 [상태 관리](/docs/contributor/contributor-ko/contributor-guides/ui/state-management/) 로컬 UI 상태용
* 검토 [컴포넌트 패턴](/docs/contributor/contributor-ko/contributor-guides/ui/component-patterns/) 사용 예시용
* 이해하기 [Web3 통합](/docs/contributor/contributor-ko/contributor-guides/ui/web3-integration/) 블록체인 데이터용


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui/rtk-query.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
