# Configuração do Store

Esta página explica como configurar a sua store Redux, definir hooks tipados e organizar a estrutura do seu projeto.

## Estrutura de pastas

Os projetos DEVEM seguir esta estrutura para garantir consistência e manutenção:

```
src/
  app/
    store.ts              # Configuração da store
    hooks.ts              # Hooks tipados (useAppDispatch/useAppSelector)
  shared/
    types/                # DTOs (API), modelos de domínio, mappers
    utils/                # Utilitários partilhados
  services/
    client.ts             # Cliente base do RTK Query
    baseQuery.ts          # Query base com auth/chainId/retry
  features/
    user/
      user.client.ts      # Endpoints do RTK Query
      user.slice.ts       # Slice do estado da UI
      user.selectors.ts   # Selectors memorizados
      __tests__/          # Testes
    land/
      land.client.ts
      land.slice.ts
      land.selectors.ts
    credits/
      credits.client.ts
      credits.slice.ts
      credits.selectors.ts
```

### Princípios de organização de pastas

* **Baseado em features** - Agrupar por domínio de negócio, não por função técnica
* **Co-localização** - Manter o código relacionado junto
* **Separação clara** - Distinguir entre dados remotos (`.client.ts`) e estado local (`.slice.ts`)

## Configuração da store

A store DEVE ser configurada com o `configureStore` do RTK e incluir todo o middleware necessário.

### Configuração básica da store

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';
import { client } from '@/services/client';
import userReducer from '@/features/user/user.slice';
import landReducer from '@/features/land/land.slice';
import creditsReducer from '@/features/credits/credits.slice';

export const store = configureStore({
  reducer: {
    // Reducer do cliente RTK Query (DEVE ser incluído)
    [client.reducerPath]: client.reducer,
    
    // Slices das features
    user: userReducer,
    land: landReducer,
    credits: creditsReducer,
  },
  
  middleware: (getDefaultMiddleware) =>
    getDefaultMiddleware({
      serializableCheck: {
        // Ignorar redux-persist ou outros caminhos conhecidos não serializáveis
        ignoredActions: ['persist/PERSIST', 'persist/REHYDRATE'],
        ignoredPaths: ['register'],
      },
    }).concat(client.middleware), // DEVE incluir o middleware do RTK Query
    
  devTools: process.env.NODE_ENV !== 'production',
});

// Exportar tipos para uso em toda a app
export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
```

### Opções de configuração

#### Verificação de serialização

O `serializableCheck` O middleware valida que todo o estado é serializável. Configure-o para ignorar exceções conhecidas:

```tsx
serializableCheck: {
  // Ações a ignorar
  ignoredActions: [
    'persist/PERSIST',
    'persist/REHYDRATE',
  ],
  // Caminhos do estado a ignorar
  ignoredPaths: ['register', 'socket.connection'],
}
```


> [!WARNING]
> **Nunca desative `serializableCheck` por completo.** Em vez disso, configure exceções e mantenha os dados não serializáveis fora do Redux.


#### Ferramentas de desenvolvimento

Ative o Redux DevTools em desenvolvimento para depuração:

```tsx
devTools: process.env.NODE_ENV !== 'production'
```

Para depuração em produção (se necessário), use uma configuração específica:

```tsx
devTools: process.env.NODE_ENV !== 'production' ? true : {
  name: 'Decentraland App',
  trace: false,
  traceLimit: 25,
}
```

## Hooks tipados

Crie versões tipadas de `useDispatch` e `useSelector` para uma melhor inferência de tipos e experiência de desenvolvimento.

### Configuração dos hooks

```tsx
// src/app/hooks.ts
import { TypedUseSelectorHook, useDispatch, useSelector } from 'react-redux';
import type { RootState, AppDispatch } from './store';

// Hook dispatch tipado
export const useAppDispatch = () => useDispatch<AppDispatch>();

// Hook selector tipado
export const useAppSelector: TypedUseSelectorHook<RootState> = useSelector;
```

### Utilização em componentes

```tsx
// ❌ Mau: usar hooks padrão
import { useDispatch, useSelector } from 'react-redux';
import { RootState } from '@/app/store';

function Component() {
  const dispatch = useDispatch(); // Sem inferência de tipos
  const user = useSelector((state: RootState) => state.user); // Tipagem manual
}

// ✅ Bom: usar hooks tipados
import { useAppDispatch, useAppSelector } from '@/app/hooks';

function Component() {
  const dispatch = useAppDispatch(); // Tipado automaticamente
  const user = useAppSelector((state) => state.user); // RootState inferido
}
```

### Vantagens dos hooks tipados

1. **Autocompletar** - IntelliSense completo para a estrutura do estado
2. **Segurança de tipos** - Apanhar erros em tempo de compilação
3. **Refatorização** - Renomear propriedades do estado com confiança
4. **Menos código repetitivo** - Não há necessidade de especificar `RootState` repetidamente

## Configuração do Provider

Envolva a sua app com o Provider do Redux:

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

## Configuração do ambiente

Configure definições específicas do ambiente:

```tsx
// src/config/constants.ts
export const API_URL = process.env.NEXT_PUBLIC_API_URL || 'https://api.decentraland.org';
export const WS_URL = process.env.NEXT_PUBLIC_WS_URL || 'wss://api.decentraland.org';
export const CHAIN_ID = process.env.NEXT_PUBLIC_CHAIN_ID || '1';

// Definições do RTK Query
export const RTK_QUERY_CONFIG = {
  keepUnusedDataFor: 60, // segundos
  refetchOnFocus: true,
  refetchOnReconnect: true,
  refetchOnMountOrArgChange: 30, // segundos
} as const;
```

## Definições de tipos

Crie definições de tipos partilhadas para consistência:

```tsx
// src/shared/types/api.types.ts
/** Wrapper da resposta da API */
export interface ApiResponse<T> {
  ok: boolean;
  data: T;
  error?: string;
}

/** Resposta paginada */
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  limit: number;
}

/** Erro comum da API */
export interface ApiError {
  message: string;
  code: string;
  details?: Record<string, any>;
}
```

```tsx
// src/shared/types/domain.types.ts
/** Modelo de domínio do utilizador */
export interface User {
  id: string;
  address: string;
  name?: string;
  avatar?: string;
  createdAt: string;
}

/** Modelo de domínio do Parcel */
export interface Parcel {
  id: string;
  x: number;
  y: number;
  owner: string;
  name?: string;
}
```

## Múltiplas instâncias da store

Para testes ou micro-frontends, pode precisar de várias instâncias da store:

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';

export function createStore(preloadedState?: Partial<RootState>) {
  return configureStore({
    reducer: {
      // ... reducers
    },
    preloadedState,
    // ... outra configuração
  });
}

// Instância padrão da store
export const store = createStore();

export type AppStore = ReturnType<typeof createStore>;
export type RootState = ReturnType<AppStore['getState']>;
export type AppDispatch = AppStore['dispatch'];
```

## Melhores práticas

### 1. Uma única store

Use sempre uma única instância da store por aplicação:

```tsx
// ✅ Bom: uma store
export const store = configureStore({ ... });

// ❌ Mau: várias stores
export const userStore = configureStore({ ... });
export const cartStore = configureStore({ ... });
```

### 2. Carregamento preguiçoso de reducers

Para divisão de código, injete reducers dinamicamente:

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

// Na store
let currentReducers = createReducer();

export function injectReducer(key: string, reducer: Reducer) {
  currentReducers = createReducer({ [key]: reducer });
  store.replaceReducer(currentReducers);
}
```

### 3. Substituição de módulos em tempo real

Ative HMR para reducers em desenvolvimento:

```tsx
if (process.env.NODE_ENV === 'development' && module.hot) {
  module.hot.accept('./reducer', () => {
    const newRootReducer = require('./reducer').default;
    store.replaceReducer(newRootReducer);
  });
}
```

### 4. Persistência do estado

Ao usar redux-persist, configure com cuidado:

```tsx
import { persistStore, persistReducer } from 'redux-persist';
import storage from 'redux-persist/lib/storage';

const persistConfig = {
  key: 'root',
  storage,
  // Persistir apenas slices específicos
  whitelist: ['user', 'preferences'],
  // Nunca persista a cache do RTK Query
  blacklist: ['client'],
};

const persistedReducer = persistReducer(persistConfig, rootReducer);
```

## Próximos Passos

* Saiba mais sobre [RTK Query](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/rtk-query/) para obtenção de dados
* Entenda [Gerenciamento de estado](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/state-management/) para estado local
* Revisar [Padrões de Componentes](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/component-patterns/) para exemplos de uso


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/store-setup.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
