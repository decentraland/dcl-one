# RTK Query

RTK Query es la potente capa de obtención y almacenamiento en caché de datos de Redux Toolkit. Elimina la necesidad de escribir creadores de acciones asíncronas y gestiona automáticamente los estados de carga, el almacenamiento en caché y la sincronización de datos.

## Configuración base del cliente

Todos los endpoints de RTK Query se extienden desde una única instancia del cliente base.

### Configuración de baseQuery

```tsx
// src/services/baseQuery.ts
import { fetchBaseQuery } from '@reduxjs/toolkit/query/react';
import type { RootState } from '@/app/store';

export const baseQuery = fetchBaseQuery({
  baseUrl: process.env.NEXT_PUBLIC_API_URL, // por ejemplo, https://api.decentraland.org
  
  prepareHeaders: (headers, { getState }) => {
    const state = getState() as RootState;
    
    // Añadir token de autenticación
    const token = state.user.session?.authToken;
    if (token) {
      headers.set('authorization', `Bearer ${token}`);
    }
    
    // Añadir chain ID para el contexto de web3
    const chainId = state.user.chainId;
    if (chainId) {
      headers.set('x-chain-id', String(chainId));
    }
    
    // Encabezados estándar
    headers.set('accept', 'application/json');
    headers.set('content-type', 'application/json');
    
    return headers;
  },
  
  credentials: 'omit', // o 'include' si el backend requiere cookies
});
```

### Instancia del cliente

```tsx
// src/services/client.ts
import { createApi } from '@reduxjs/toolkit/query/react';
import { baseQuery } from './baseQuery';

export const client = createApi({
  reducerPath: 'client',
  baseQuery,
  
  // Definir todos los posibles tipos de tag para invalidar la caché
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
  
  // Configuración de caché
  keepUnusedDataFor: 60,         // Conservar los datos no utilizados durante 60 segundos
  refetchOnFocus: true,           // Volver a obtener cuando la ventana recupere el foco
  refetchOnReconnect: true,       // Volver a obtener al reconectarse
  refetchOnMountOrArgChange: 30,  // Volver a obtener si los datos tienen más de 30 s
  
  // Los endpoints se inyectarán en los archivos de funciones
  endpoints: () => ({}),
});
```

## Convenciones de tags

Los tags se usan para la invalidación y sincronización de caché. Sigue estas convenciones:

### Nomenclatura de tags

| Tipo de recurso     | Tags de consulta                     | La mutación invalida           |
| ------------------- | ------------------------------------ | ------------------------------ |
| **Colecciones**     | `'Parcels'` (plural)                 | `'Parcels'`                    |
| **Entidad única**   | `{type: 'Parcels', id: '123'}`       | `{type: 'Parcels', id: '123'}` |
| **Lista + detalle** | `['Parcels', {type: 'Parcels', id}]` | `['Parcels']` o id específico  |

### Ejemplos de tags

```tsx
// Colección: proporciona el tag de lista
providesTags: ['Parcels']

// Entidad única: proporciona el tag específico + el tag de lista
providesTags: (result) => 
  result 
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// Mutación: invalida tanto la lista como la entidad específica
invalidatesTags: (result, error, arg) => [
  { type: 'Parcels', id: arg.id },
  'Parcels'
]
```

## Creando endpoints

Los endpoints DEBERÍAN estar colocados junto a su función en `feature.client.ts` archivos.

### Endpoint de consulta (lectura)

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
    // Obtener todos los tiles
    getTiles: build.query<Record<string, Tile>, void>({
      query: () => '/v1/tiles',
      providesTags: ['Parcels'],
    }),
    
    // Obtener parcela por coordenadas
    getParcelByCoords: build.query<Parcel, { x: number; y: number }>({
      query: ({ x, y }) => `/v1/lands/${x}/${y}`,
      providesTags: (result, error, arg) =>
        result
          ? [{ type: 'Parcels', id: result.id }, 'Parcels']
          : ['Parcels'],
    }),
    
    // Obtener parcelas por propietario
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

// Exportar hooks
export const {
  useGetTilesQuery,
  useGetParcelByCoordsQuery,
  useGetParcelsByOwnerQuery,
} = landClient;
```

### Endpoint de mutación (escritura)

```tsx
// src/features/land/land.client.ts (continued)
export const landClient = client.injectEndpoints({
  endpoints: (build) => ({
    // ... query endpoints ...
    
    // Actualizar el nombre de la parcela
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
    
    // Transferir parcela
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
        'Parcels', // Invalidar la lista para actualizar los filtros por propietario
      ],
    }),
  }),
});

export const {
  useUpdateParcelNameMutation,
  useTransferParcelMutation,
} = landClient;
```

## Actualizaciones optimistas

Usa `onQueryStarted` para actualizaciones optimistas de la UI con reversión automática en caso de fallo.

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
      
      // Actualización optimista
      async onQueryStarted({ address, amount }, { dispatch, queryFulfilled }) {
        // Actualiza la caché de forma optimista
        const patchResult = dispatch(
          client.util.updateQueryData('getBalance', { address }, (draft) => {
            draft.amount += amount;
            draft.lastUpdated = new Date().toISOString();
          })
        );
        
        try {
          // Esperar a que se complete la mutación
          const { data } = await queryFulfilled;
          
          // Actualizar con la respuesta del servidor
          dispatch(
            client.util.updateQueryData('getBalance', { address }, (draft) => {
              draft.amount = data.newBalance;
            })
          );
        } catch {
          // Revertir en caso de fallo
          patchResult.undo();
        }
      },
      
      // También invalidar para garantizar la consistencia
      invalidatesTags: (result, error, arg) => [
        { type: 'Credits', id: arg.address }
      ],
    }),
  }),
});

export const { useGetBalanceQuery, useGrantCreditsMutation } = creditsClient;
```

## Opciones avanzadas de consulta

### Polling

```tsx
// Consultar cada 10 segundos
const { data } = useGetBalanceQuery(
  { address },
  { pollingInterval: 10000 }
);
```

### Omitir consulta

```tsx
// Omitir la consulta si la dirección no está disponible
const { data } = useGetBalanceQuery(
  { address: address! },
  { skip: !address }
);
```

### Consulta diferida

```tsx
const [trigger, result] = useLazyGetParcelByCoordsQuery();

// Activar manualmente
const handleClick = () => {
  trigger({ x: 10, y: 20 });
};
```

### Transformar la respuesta

```tsx
getParcel: build.query<Parcel, string>({
  query: (id) => `/v1/lands/${id}`,
  transformResponse: (response: ApiResponse<Parcel>) => response.data,
})
```

### Serialización personalizada

Para paginación o búsqueda, personaliza la serialización de la clave de caché:

```tsx
searchParcels: build.query<Parcel[], { q: string; owner?: string; page?: number }>({
  query: (args) => ({
    url: '/v1/parcels/search',
    params: args,
  }),
  
  // Clave de caché personalizada para manejar parámetros opcionales
  serializeQueryArgs: ({ endpointName, queryArgs }) => {
    const { q, owner = 'any', page = 1 } = queryArgs;
    return `${endpointName}-${q}-${owner}-${page}`;
  },
  
  // Combinar resultados para la paginación
  merge(currentCache, newItems, { arg }) {
    if (arg.page === 1) {
      return newItems;
    }
    return [...currentCache, ...newItems];
  },
  
  // Forzar una nueva obtención cuando cambien los argumentos
  forceRefetch({ currentArg, previousArg }) {
    return JSON.stringify(currentArg) !== JSON.stringify(previousArg);
  },
  
  providesTags: ['Parcels'],
})
```

## Manejo de errores

### Manejo de errores personalizado

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

Uso en componentes:

```tsx
const { data, error } = useGetParcelQuery({ id });

if (error) {
  if (isFetchBaseQueryError(error)) {
    const errMsg = 'error' in error ? error.error : JSON.stringify(error.data);
    return <div>Error: {errMsg}</div>;
  } else if (isErrorWithMessage(error)) {
    return <div>Error: {error.message}</div>;
  }
}
```

## Gestión de caché

### Actualizaciones manuales de caché

```tsx
// Actualizar la caché directamente
dispatch(
  client.util.updateQueryData('getBalance', { address }, (draft) => {
    draft.amount = 1000;
  })
);
```

### Invalidar caché

```tsx
// Invalidar todas las consultas de Credits
dispatch(client.util.invalidateTags(['Credits']));

// Invalidar entidad específica
dispatch(client.util.invalidateTags([{ type: 'Credits', id: address }]));
```

### Restablecer el estado del cliente

```tsx
// Restablecer todo el estado del cliente
dispatch(client.util.resetApiState());
```

### Precargar datos

```tsx
// Precargar datos antes de la navegación
dispatch(
  client.util.prefetch('getParcel', { id: '123' }, { force: false })
);
```

## Buenas prácticas

### 1. Usa nombres descriptivos para los endpoints

```tsx
// ✅ Bien
getParcelByCoords
getParcelsByOwner
updateParcelName

// ❌ Mal
getParcel
fetch
update
```

### 2. Proporciona tags completos

```tsx
// ✅ Bien: Proporciona tags tanto de lista como de entidad
providesTags: (result) =>
  result
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// ❌ Mal: Solo proporciona el tag de lista
providesTags: ['Parcels']
```

### 3. Maneja los estados de carga y error

```tsx
// ✅ Bien: Manejo completo del estado
const { data, isLoading, isFetching, isError, error } = useGetParcelQuery({ id });

if (isLoading) return <Spinner />;
if (isError) return <Error error={error} />;
if (!data) return null;

// ❌ Mal: Manejo de estado incompleto
const { data } = useGetParcelQuery({ id });
return <div>{data.name}</div>; // Puede fallar si data no está definida
```

### 4. Usa guardas de tipos

```tsx
// ✅ Bien: Manejo de errores con seguridad de tipos
if (isFetchBaseQueryError(error)) {
  // Manejar error de obtención
} else if (isErrorWithMessage(error)) {
  // Manejar error con mensaje
}

// ❌ Mal: Conversión de tipo insegura
const message = (error as any).message;
```

## Próximos pasos

* Aprende sobre [la gestión del estado](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/state-management/) para el estado local de la UI
* Revisa [Patrones de Componentes](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/component-patterns/) para ejemplos de uso
* Entiende [Integración con Web3](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/web3-integration/) para datos de blockchain


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/rtk-query.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
