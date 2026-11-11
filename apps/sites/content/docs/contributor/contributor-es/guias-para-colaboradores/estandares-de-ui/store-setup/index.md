# Configuración del almacén

Esta página cubre cómo configurar tu store de Redux, configurar hooks tipados y organizar la estructura de tu proyecto.

## Estructura de carpetas

Los proyectos DEBEN seguir esta estructura para mantener la consistencia y la mantenibilidad:

```
src/
  app/
    store.ts              # Configuración del store
    hooks.ts              # Hooks tipados (useAppDispatch/useAppSelector)
  shared/
    types/                # DTOs (API), modelos de dominio, mappers
    utils/                # Utilidades compartidas
  services/
    client.ts             # Cliente base de RTK Query
    baseQuery.ts          # Consulta base con auth/chainId/retry
  features/
    user/
      user.client.ts      # Endpoints de RTK Query
      user.slice.ts       # Slice del estado de UI
      user.selectors.ts   # Selectores memorizados
      __tests__/          # Pruebas
    land/
      land.client.ts
      land.slice.ts
      land.selectors.ts
    credits/
      credits.client.ts
      credits.slice.ts
      credits.selectors.ts
```

### Principios de organización de carpetas

* **Basado en features** - Agrupa por dominio de negocio, no por rol técnico
* **Co-ubicación** - Mantén el código relacionado junto
* **Separación clara** - Distingue entre datos remotos (`.client.ts`) y estado local (`.slice.ts`)

## Configuración del store

El store DEBE configurarse con configureStore de RTK `configureStore` e incluir todo el middleware necesario.

### Configuración básica del store

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';
import { client } from '@/services/client';
import userReducer from '@/features/user/user.slice';
import landReducer from '@/features/land/land.slice';
import creditsReducer from '@/features/credits/credits.slice';

export const store = configureStore({
  reducer: {
    // Reducer cliente de RTK Query (DEBE incluirse)
    [client.reducerPath]: client.reducer,
    
    // Slices de features
    user: userReducer,
    land: landReducer,
    credits: creditsReducer,
  },
  
  middleware: (getDefaultMiddleware) =>
    getDefaultMiddleware({
      serializableCheck: {
        // Ignora redux-persist u otras rutas no serializables conocidas
        ignoredActions: ['persist/PERSIST', 'persist/REHYDRATE'],
        ignoredPaths: ['register'],
      },
    }).concat(client.middleware), // DEBE incluir el middleware de RTK Query
    
  devTools: process.env.NODE_ENV !== 'production',
});

// Exporta tipos para usarlos en toda la app
export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
```

### Opciones de configuración

#### Comprobación de serializabilidad

El `serializableCheck` El middleware valida que todo el estado sea serializable. Configúralo para ignorar excepciones conocidas:

```tsx
serializableCheck: {
  // Acciones a ignorar
  ignoredActions: [
    'persist/PERSIST',
    'persist/REHYDRATE',
  ],
  // Rutas de estado a ignorar
  ignoredPaths: ['register', 'socket.connection'],
}
```


> [!WARNING]
> **Nunca desactives `serializableCheck` completamente.** En su lugar, configura excepciones y mantén los datos no serializables fuera de Redux.


#### Herramientas de desarrollo

Activa Redux DevTools en desarrollo para depurar:

```tsx
devTools: process.env.NODE_ENV !== 'production'
```

Para la depuración en producción (si es necesario), usa una configuración específica:

```tsx
devTools: process.env.NODE_ENV !== 'production' ? true : {
  name: 'Decentraland App',
  trace: false,
  traceLimit: 25,
}
```

## Hooks tipados

Crea versiones tipadas de `useDispatch` y `useSelector` para una mejor inferencia de tipos y experiencia de desarrollo.

### Configuración de hooks

```tsx
// src/app/hooks.ts
import { TypedUseSelectorHook, useDispatch, useSelector } from 'react-redux';
import type { RootState, AppDispatch } from './store';

// Hook tipado de dispatch
export const useAppDispatch = () => useDispatch<AppDispatch>();

// Hook tipado de selector
export const useAppSelector: TypedUseSelectorHook<RootState> = useSelector;
```

### Uso en componentes

```tsx
// ❌ Mal: usando hooks estándar
import { useDispatch, useSelector } from 'react-redux';
import { RootState } from '@/app/store';

function Component() {
  const dispatch = useDispatch(); // Sin inferencia de tipos
  const user = useSelector((state: RootState) => state.user); // Tipado manual
}

// ✅ Bien: usando hooks tipados
import { useAppDispatch, useAppSelector } from '@/app/hooks';

function Component() {
  const dispatch = useAppDispatch(); // Tipado automáticamente
  const user = useAppSelector((state) => state.user); // RootState inferido
}
```

### Beneficios de los hooks tipados

1. **Autocompletado** - IntelliSense completo para la estructura del estado
2. **Seguridad de tipos** - Detecta errores en tiempo de compilación
3. **Refactorización** - Renombra propiedades del estado con confianza
4. **Menos código repetitivo** - No hace falta especificar `RootState` repetidamente

## Configuración del Provider

Envuelve tu app con el Redux Provider:

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

## Configuración del entorno

Configura ajustes específicos del entorno:

```tsx
// src/config/constants.ts
export const API_URL = process.env.NEXT_PUBLIC_API_URL || 'https://api.decentraland.org';
export const WS_URL = process.env.NEXT_PUBLIC_WS_URL || 'wss://api.decentraland.org';
export const CHAIN_ID = process.env.NEXT_PUBLIC_CHAIN_ID || '1';

// Configuración de RTK Query
export const RTK_QUERY_CONFIG = {
  keepUnusedDataFor: 60, // segundos
  refetchOnFocus: true,
  refetchOnReconnect: true,
  refetchOnMountOrArgChange: 30, // segundos
} as const;
```

## Definiciones de tipos

Crea definiciones de tipos compartidas para mantener la consistencia:

```tsx
// src/shared/types/api.types.ts
/** Envoltorio de respuesta de API */
export interface ApiResponse<T> {
  ok: boolean;
  data: T;
  error?: string;
}

/** Respuesta paginada */
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  limit: number;
}

/** Error común de API */
export interface ApiError {
  message: string;
  code: string;
  details?: Record<string, any>;
}
```

```tsx
// src/shared/types/domain.types.ts
/** Modelo de dominio de usuario */
export interface User {
  id: string;
  address: string;
  name?: string;
  avatar?: string;
  createdAt: string;
}

/** Modelo de dominio de Parcel */
export interface Parcel {
  id: string;
  x: number;
  y: number;
  owner: string;
  name?: string;
}
```

## Múltiples instancias del store

Para pruebas o micro-frontends, podrías necesitar múltiples instancias del store:

```tsx
// src/app/store.ts
import { configureStore } from '@reduxjs/toolkit';

export function createStore(preloadedState?: Partial<RootState>) {
  return configureStore({
    reducer: {
      // ... reducers
    },
    preloadedState,
    // ... otra configuración
  });
}

// Instancia predeterminada del store
export const store = createStore();

export type AppStore = ReturnType<typeof createStore>;
export type RootState = ReturnType<AppStore['getState']>;
export type AppDispatch = AppStore['dispatch'];
```

## Buenas prácticas

### 1. Store único

Usa siempre una sola instancia del store por aplicación:

```tsx
// ✅ Bien: un solo store
export const store = configureStore({ ... });

// ❌ Mal: múltiples stores
export const userStore = configureStore({ ... });
export const cartStore = configureStore({ ... });
```

### 2. Carga diferida de reducers

Para dividir el código, inyecta reducers dinámicamente:

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

// En el store
let currentReducers = createReducer();

export function injectReducer(key: string, reducer: Reducer) {
  currentReducers = createReducer({ [key]: reducer });
  store.replaceReducer(currentReducers);
}
```

### 3. Reemplazo de módulos en caliente

Activa HMR para reducers en desarrollo:

```tsx
if (process.env.NODE_ENV === 'development' && module.hot) {
  module.hot.accept('./reducer', () => {
    const newRootReducer = require('./reducer').default;
    store.replaceReducer(newRootReducer);
  });
}
```

### 4. Persistencia del estado

Al usar redux-persist, configura con cuidado:

```tsx
import { persistStore, persistReducer } from 'redux-persist';
import storage from 'redux-persist/lib/storage';

const persistConfig = {
  key: 'root',
  storage,
  // Persistir solo slices específicos
  whitelist: ['user', 'preferences'],
  // Nunca persistir la caché de RTK Query
  blacklist: ['client'],
};

const persistedReducer = persistReducer(persistConfig, rootReducer);
```

## Próximos pasos

* Aprende sobre [RTK Query](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/rtk-query/) para la obtención de datos
* Entiende [la gestión del estado](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/state-management/) para el estado local
* Revisa [Patrones de Componentes](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/component-patterns/) para ejemplos de uso


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/store-setup.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
