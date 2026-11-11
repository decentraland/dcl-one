# 스토어 설정

이 페이지에서는 Redux 저장소를 구성하고, 타입이 지정된 훅을 설정하며, 프로젝트 구조를 정리하는 방법을 다룹니다.

## 폴더 구조

프로젝트는 일관성과 유지보수성을 위해 반드시 이 구조를 따라야 합니다:

```
src/
  app/
    store.ts              # 저장소 설정
    hooks.ts              # 타입이 지정된 훅 (useAppDispatch/useAppSelector)
  shared/
    types/                # DTO(API), 도메인 모델, 매퍼
    utils/                # 공유 유틸리티
  services/
    client.ts             # RTK Query 기본 클라이언트
    baseQuery.ts          # 인증/chainId/retry가 포함된 기본 쿼리
  features/
    user/
      user.client.ts      # RTK Query 엔드포인트
      user.slice.ts       # UI 상태 슬라이스
      user.selectors.ts   # 메모이즈된 셀렉터
      __tests__/          # 테스트
    land/
      land.client.ts
      land.slice.ts
      land.selectors.ts
    credits/
      credits.client.ts
      credits.slice.ts
      credits.selectors.ts
```

### 폴더 구성 원칙

* **기능 중심** - 기술적 역할이 아니라 비즈니스 도메인별로 그룹화
* **코 위치 배치** - 관련 코드는 함께 두기
* **명확한 분리** - 원격 데이터(`.client.ts`)와 로컬 상태(`.slice.ts`)

## 저장소 설정

저장소는 RTK의 `configureStore` 로 구성되어야 하며 필요한 모든 미들웨어를 포함해야 합니다.

### 기본 저장소 설정

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';
import { client } from '@/services/client';
import userReducer from '@/features/user/user.slice';
import landReducer from '@/features/land/land.slice';
import creditsReducer from '@/features/credits/credits.slice';

export const store = configureStore({
  reducer: {
    // RTK Query 클라이언트 reducer (반드시 포함해야 함)
    [client.reducerPath]: client.reducer,
    
    // 기능 슬라이스
    user: userReducer,
    land: landReducer,
    credits: creditsReducer,
  },
  
  middleware: (getDefaultMiddleware) =>
    getDefaultMiddleware({
      serializableCheck: {
        // redux-persist 또는 기타 알려진 비직렬화 경로 무시
        ignoredActions: ['persist/PERSIST', 'persist/REHYDRATE'],
        ignoredPaths: ['register'],
      },
    }).concat(client.middleware), // RTK Query 미들웨어를 반드시 포함
    
  devTools: process.env.NODE_ENV !== 'production',
});

// 앱 전반에서 사용할 타입 내보내기
export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
```

### 설정 옵션

#### 직렬화 가능성 검사

이 `serializableCheck` 미들웨어는 모든 상태가 직렬화 가능한지 검증합니다. 알려진 예외는 무시하도록 설정하세요:

```tsx
serializableCheck: {
  // 무시할 액션
  ignoredActions: [
    'persist/PERSIST',
    'persist/REHYDRATE',
  ],
  // 무시할 상태 경로
  ignoredPaths: ['register', 'socket.connection'],
}
```


> [!WARNING]
> **절대 비활성화하지 `serializableCheck` 마세요.** 대신 예외를 구성하고 비직렬화 데이터는 Redux 밖에 두세요.


#### 개발자 도구

디버깅을 위해 개발 환경에서 Redux DevTools를 활성화하세요:

```tsx
devTools: process.env.NODE_ENV !== 'production'
```

프로덕션 디버깅이 필요하다면, 다음과 같이 특정 구성을 사용하세요:

```tsx
devTools: process.env.NODE_ENV !== 'production' ? true : {
  name: 'Decentraland App',
  trace: false,
  traceLimit: 25,
}
```

## 타입이 지정된 훅

다음의 타입이 지정된 버전을 만드세요 `useDispatch` 및 `useSelector` 더 나은 타입 추론과 개발자 경험을 위해.

### 훅 설정

```tsx
// src/app/hooks.ts
import { TypedUseSelectorHook, useDispatch, useSelector } from 'react-redux';
import type { RootState, AppDispatch } from './store';

// 타입이 지정된 dispatch 훅
export const useAppDispatch = () => useDispatch<AppDispatch>();

// 타입이 지정된 selector 훅
export const useAppSelector: TypedUseSelectorHook<RootState> = useSelector;
```

### 컴포넌트에서의 사용

```tsx
// ❌ 나쁨: 표준 훅 사용
import { useDispatch, useSelector } from 'react-redux';
import { RootState } from '@/app/store';

function Component() {
  const dispatch = useDispatch(); // 타입 추론 없음
  const user = useSelector((state: RootState) => state.user); // 수동 타입 지정
}

// ✅ 좋음: 타입이 지정된 훅 사용
import { useAppDispatch, useAppSelector } from '@/app/hooks';

function Component() {
  const dispatch = useAppDispatch(); // 자동으로 타입 지정됨
  const user = useAppSelector((state) => state.user); // RootState가 추론됨
}
```

### 타입이 지정된 훅의 이점

1. **자동완성** - 상태 구조에 대한 전체 IntelliSense
2. **타입 안전성** - 컴파일 시점에 오류를 잡음
3. **리팩터링** - 자신 있게 상태 속성 이름 변경
4. **보일러플레이트 감소** - 다음을 반복해서 지정할 필요 없음 `RootState` 반복적으로

## Provider 설정

앱을 Redux Provider로 감싸세요:

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

### React (Vite/CRA)

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

## 환경 설정

환경별 설정을 구성하세요:

```tsx
// src/config/constants.ts
export const API_URL = process.env.NEXT_PUBLIC_API_URL || 'https://api.decentraland.org';
export const WS_URL = process.env.NEXT_PUBLIC_WS_URL || 'wss://api.decentraland.org';
export const CHAIN_ID = process.env.NEXT_PUBLIC_CHAIN_ID || '1';

// RTK Query 설정
export const RTK_QUERY_CONFIG = {
  keepUnusedDataFor: 60, // 초
  refetchOnFocus: true,
  refetchOnReconnect: true,
  refetchOnMountOrArgChange: 30, // 초
} as const;
```

## 타입 정의

일관성을 위해 공유 타입 정의를 만드세요:

```tsx
// src/shared/types/api.types.ts
/** API 응답 래퍼 */
export interface ApiResponse<T> {
  ok: boolean;
  data: T;
  error?: string;
}

/** 페이지네이션 응답 */
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  limit: number;
}

/** 공통 API 오류 */
export interface ApiError {
  message: string;
  code: string;
  details?: Record<string, any>;
}
```

```tsx
// src/shared/types/domain.types.ts
/** 사용자 도메인 모델 */
export interface User {
  id: string;
  address: string;
  name?: string;
  avatar?: string;
  createdAt: string;
}

/** Parcel 도메인 모델 */
export interface Parcel {
  id: string;
  x: number;
  y: number;
  owner: string;
  name?: string;
}
```

## 여러 저장소 인스턴스

테스트나 마이크로 프런트엔드에서는 여러 저장소 인스턴스가 필요할 수 있습니다:

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';

export function createStore(preloadedState?: Partial<RootState>) {
  return configureStore({
    reducer: {
      // ... 리듀서
    },
    preloadedState,
    // ... 기타 설정
  });
}

// 기본 저장소 인스턴스
export const store = createStore();

export type AppStore = ReturnType<typeof createStore>;
export type RootState = ReturnType<AppStore['getState']>;
export type AppDispatch = AppStore['dispatch'];
```

## 권장 사항

### 1. 단일 저장소

애플리케이션당 항상 하나의 저장소 인스턴스를 사용하세요:

```tsx
// ✅ 좋음: 하나의 저장소
export const store = configureStore({ ... });

// ❌ 나쁨: 여러 저장소
export const userStore = configureStore({ ... });
export const cartStore = configureStore({ ... });
```

### 2. 리듀서 지연 로딩

코드 분할을 위해 리듀서를 동적으로 주입하세요:

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

// 저장소 내부
let currentReducers = createReducer();

export function injectReducer(key: string, reducer: Reducer) {
  currentReducers = createReducer({ [key]: reducer });
  store.replaceReducer(currentReducers);
}
```

### 3. 핫 모듈 교체

개발 환경에서 리듀서에 대해 HMR을 활성화하세요:

```tsx
if (process.env.NODE_ENV === 'development' && module.hot) {
  module.hot.accept('./reducer', () => {
    const newRootReducer = require('./reducer').default;
    store.replaceReducer(newRootReducer);
  });
}
```

### 4. 상태 지속화

redux-persist를 사용할 때는 신중하게 설정하세요:

```tsx
import { persistStore, persistReducer } from 'redux-persist';
import storage from 'redux-persist/lib/storage';

const persistConfig = {
  key: 'root',
  storage,
  // 특정 슬라이스만 지속화
  whitelist: ['user', 'preferences'],
  // RTK Query 캐시는 절대 지속화하지 않음
  blacklist: ['client'],
};

const persistedReducer = persistReducer(persistConfig, rootReducer);
```

## 다음 단계

* 학습해 보세요 [RTK Query](/docs/contributor/contributor-ko/contributor-guides/ui/rtk-query/) 데이터 가져오기를 위한
* 이해하기 [상태 관리](/docs/contributor/contributor-ko/contributor-guides/ui/state-management/) 로컬 상태를 위한
* 검토 [컴포넌트 패턴](/docs/contributor/contributor-ko/contributor-guides/ui/component-patterns/) 사용 예시용


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui/store-setup.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
