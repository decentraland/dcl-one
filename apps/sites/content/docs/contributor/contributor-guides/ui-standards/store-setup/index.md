# Store Setup

This page covers how to configure your Redux store, set up typed hooks, and organize your project structure.

## Folder Structure

Projects MUST follow this structure for consistency and maintainability:

```
src/
  app/
    store.ts              # Store configuration
    hooks.ts              # Typed hooks (useAppDispatch/useAppSelector)
  shared/
    types/                # DTOs (API), Domain models, mappers
    utils/                # Shared utilities
  services/
    client.ts             # RTK Query base client
    baseQuery.ts          # Base query with auth/chainId/retry
  features/
    user/
      user.client.ts      # RTK Query endpoints
      user.slice.ts       # UI state slice
      user.selectors.ts   # Memoized selectors
      __tests__/          # Tests
    land/
      land.client.ts
      land.slice.ts
      land.selectors.ts
    credits/
      credits.client.ts
      credits.slice.ts
      credits.selectors.ts
```

### Folder Organization Principles

* **Feature-based** - Group by business domain, not technical role
* **Co-location** - Keep related code together
* **Clear separation** - Distinguish between remote data (`.client.ts`) and local state (`.slice.ts`)

## Store Configuration

The store MUST be configured with RTK's `configureStore` and include all necessary middleware.

### Basic Store Setup

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';
import { client } from '@/services/client';
import userReducer from '@/features/user/user.slice';
import landReducer from '@/features/land/land.slice';
import creditsReducer from '@/features/credits/credits.slice';

export const store = configureStore({
  reducer: {
    // RTK Query client reducer (MUST be included)
    [client.reducerPath]: client.reducer,
    
    // Feature slices
    user: userReducer,
    land: landReducer,
    credits: creditsReducer,
  },
  
  middleware: (getDefaultMiddleware) =>
    getDefaultMiddleware({
      serializableCheck: {
        // Ignore redux-persist or other known non-serializable paths
        ignoredActions: ['persist/PERSIST', 'persist/REHYDRATE'],
        ignoredPaths: ['register'],
      },
    }).concat(client.middleware), // MUST include RTK Query middleware
    
  devTools: process.env.NODE_ENV !== 'production',
});

// Export types for use throughout the app
export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
```

### Configuration Options

#### Serializable Check

The `serializableCheck` middleware validates that all state is serializable. Configure it to ignore known exceptions:

```tsx
serializableCheck: {
  // Actions to ignore
  ignoredActions: [
    'persist/PERSIST',
    'persist/REHYDRATE',
  ],
  // State paths to ignore
  ignoredPaths: ['register', 'socket.connection'],
}
```


> [!WARNING]
> **Never disable `serializableCheck` entirely.** Instead, configure exceptions and keep non-serializable data outside Redux.


#### Dev Tools

Enable Redux DevTools in development for debugging:

```tsx
devTools: process.env.NODE_ENV !== 'production'
```

For production debugging (if needed), use specific configuration:

```tsx
devTools: process.env.NODE_ENV !== 'production' ? true : {
  name: 'Decentraland App',
  trace: false,
  traceLimit: 25,
}
```

## Typed Hooks

Create typed versions of `useDispatch` and `useSelector` for better type inference and developer experience.

### Hook Setup

```tsx
// src/app/hooks.ts
import { TypedUseSelectorHook, useDispatch, useSelector } from 'react-redux';
import type { RootState, AppDispatch } from './store';

// Typed dispatch hook
export const useAppDispatch = () => useDispatch<AppDispatch>();

// Typed selector hook
export const useAppSelector: TypedUseSelectorHook<RootState> = useSelector;
```

### Usage in Components

```tsx
// ❌ Bad: Using standard hooks
import { useDispatch, useSelector } from 'react-redux';
import { RootState } from '@/app/store';

function Component() {
  const dispatch = useDispatch(); // No type inference
  const user = useSelector((state: RootState) => state.user); // Manual typing
}

// ✅ Good: Using typed hooks
import { useAppDispatch, useAppSelector } from '@/app/hooks';

function Component() {
  const dispatch = useAppDispatch(); // Typed automatically
  const user = useAppSelector((state) => state.user); // RootState inferred
}
```

### Benefits of Typed Hooks

1. **Autocomplete** - Full IntelliSense for state shape
2. **Type Safety** - Catch errors at compile time
3. **Refactoring** - Rename state properties with confidence
4. **Less Boilerplate** - No need to specify `RootState` repeatedly

## Provider Setup

Wrap your app with the Redux Provider:

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

## Environment Configuration

Configure environment-specific settings:

```tsx
// src/config/constants.ts
export const API_URL = process.env.NEXT_PUBLIC_API_URL || 'https://api.decentraland.org';
export const WS_URL = process.env.NEXT_PUBLIC_WS_URL || 'wss://api.decentraland.org';
export const CHAIN_ID = process.env.NEXT_PUBLIC_CHAIN_ID || '1';

// RTK Query settings
export const RTK_QUERY_CONFIG = {
  keepUnusedDataFor: 60, // seconds
  refetchOnFocus: true,
  refetchOnReconnect: true,
  refetchOnMountOrArgChange: 30, // seconds
} as const;
```

## Type Definitions

Create shared type definitions for consistency:

```tsx
// src/shared/types/api.types.ts
/** API response wrapper */
export interface ApiResponse<T> {
  ok: boolean;
  data: T;
  error?: string;
}

/** Paginated response */
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  limit: number;
}

/** Common API error */
export interface ApiError {
  message: string;
  code: string;
  details?: Record<string, any>;
}
```

```tsx
// src/shared/types/domain.types.ts
/** User domain model */
export interface User {
  id: string;
  address: string;
  name?: string;
  avatar?: string;
  createdAt: string;
}

/** Parcel domain model */
export interface Parcel {
  id: string;
  x: number;
  y: number;
  owner: string;
  name?: string;
}
```

## Multiple Store Instances

For testing or micro-frontends, you may need multiple store instances:

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';

export function createStore(preloadedState?: Partial<RootState>) {
  return configureStore({
    reducer: {
      // ... reducers
    },
    preloadedState,
    // ... other config
  });
}

// Default store instance
export const store = createStore();

export type AppStore = ReturnType<typeof createStore>;
export type RootState = ReturnType<AppStore['getState']>;
export type AppDispatch = AppStore['dispatch'];
```

## Best Practices

### 1. Single Store

Always use a single store instance per application:

```tsx
// ✅ Good: One store
export const store = configureStore({ ... });

// ❌ Bad: Multiple stores
export const userStore = configureStore({ ... });
export const cartStore = configureStore({ ... });
```

### 2. Lazy Loading Reducers

For code splitting, inject reducers dynamically:

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

// In store
let currentReducers = createReducer();

export function injectReducer(key: string, reducer: Reducer) {
  currentReducers = createReducer({ [key]: reducer });
  store.replaceReducer(currentReducers);
}
```

### 3. Hot Module Replacement

Enable HMR for reducers in development:

```tsx
if (process.env.NODE_ENV === 'development' && module.hot) {
  module.hot.accept('./reducer', () => {
    const newRootReducer = require('./reducer').default;
    store.replaceReducer(newRootReducer);
  });
}
```

### 4. State Persistence

When using redux-persist, configure carefully:

```tsx
import { persistStore, persistReducer } from 'redux-persist';
import storage from 'redux-persist/lib/storage';

const persistConfig = {
  key: 'root',
  storage,
  // Only persist specific slices
  whitelist: ['user', 'preferences'],
  // Never persist RTK Query cache
  blacklist: ['client'],
};

const persistedReducer = persistReducer(persistConfig, rootReducer);
```

## Next Steps

* Learn about [RTK Query](/docs/contributor/contributor-guides/ui-standards/rtk-query/) for data fetching
* Understand [State Management](/docs/contributor/contributor-guides/ui-standards/state-management/) for local state
* Review [Component Patterns](/docs/contributor/contributor-guides/ui-standards/component-patterns/) for usage examples
