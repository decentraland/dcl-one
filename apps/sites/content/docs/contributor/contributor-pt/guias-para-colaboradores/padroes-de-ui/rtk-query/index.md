# RTK Query

RTK Query é a poderosa camada de busca e cache de dados do Redux Toolkit. Ela elimina a necessidade de escrever criadores de ações assíncronas e gerencia automaticamente estados de carregamento, cache e sincronização de dados.

## Configuração base do cliente

Todos os endpoints do RTK Query derivam de uma única instância base do cliente.

### Configuração da Base Query

```tsx
// src/services/baseQuery.ts
import { fetchBaseQuery } from '@reduxjs/toolkit/query/react';
import type { RootState } from '@/app/store';

export const baseQuery = fetchBaseQuery({
  baseUrl: process.env.NEXT_PUBLIC_API_URL, // por exemplo, https://api.decentraland.org
  
  prepareHeaders: (headers, { getState }) => {
    const state = getState() as RootState;
    
    // Adicionar token de autenticação
    const token = state.user.session?.authToken;
    if (token) {
      headers.set('authorization', `Bearer ${token}`);
    }
    
    // Adicionar chain ID para contexto web3
    const chainId = state.user.chainId;
    if (chainId) {
      headers.set('x-chain-id', String(chainId));
    }
    
    // Cabeçalhos padrão
    headers.set('accept', 'application/json');
    headers.set('content-type', 'application/json');
    
    return headers;
  },
  
  credentials: 'omit', // ou 'include' se o backend exigir cookies
});
```

### Instância do Cliente

```tsx
// src/services/client.ts
import { createApi } from '@reduxjs/toolkit/query/react';
import { baseQuery } from './baseQuery';

export const client = createApi({
  reducerPath: 'client',
  baseQuery,
  
  // Defina todos os possíveis tipos de tags para invalidação de cache
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
  
  // Configuração de cache
  keepUnusedDataFor: 60,         // Manter dados não utilizados por 60 segundos
  refetchOnFocus: true,           // Buscar novamente quando a janela recuperar o foco
  refetchOnReconnect: true,       // Buscar novamente ao reconectar
  refetchOnMountOrArgChange: 30,  // Buscar novamente se os dados tiverem mais de 30s
  
  // Os endpoints serão injetados nos arquivos de feature
  endpoints: () => ({}),
});
```

## Convenções de Tags

As tags são usadas para invalidação e sincronização de cache. Siga estas convenções:

### Nomeação de Tags

| Tipo de Recurso     | Tags de Consulta                     | Invalidações da Mutação        |
| ------------------- | ------------------------------------ | ------------------------------ |
| **Coleções**        | `'Parcels'` (plural)                 | `'Parcels'`                    |
| **Entidade Única**  | `{type: 'Parcels', id: '123'}`       | `{type: 'Parcels', id: '123'}` |
| **Lista + Detalhe** | `['Parcels', {type: 'Parcels', id}]` | `['Parcels']` ou id específico |

### Exemplos de Tags

```tsx
// Coleção: fornece tag de lista
providesTags: ['Parcels']

// Entidade única: fornece tag específica + tag de lista
providesTags: (result) => 
  result 
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// Mutação: invalida tanto a lista quanto a entidade específica
invalidatesTags: (result, error, arg) => [
  { type: 'Parcels', id: arg.id },
  'Parcels'
]
```

## Criando Endpoints

Os endpoints DEVEM ser colocados junto com sua feature em `feature.client.ts` arquivos.

### Endpoint de Consulta (Leitura)

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
    // Obter todos os tiles
    getTiles: build.query<Record<string, Tile>, void>({
      query: () => '/v1/tiles',
      providesTags: ['Parcels'],
    }),
    
    // Obter parcel por coordenadas
    getParcelByCoords: build.query<Parcel, { x: number; y: number }>({
      query: ({ x, y }) => `/v1/lands/${x}/${y}`,
      providesTags: (result, error, arg) =>
        result
          ? [{ type: 'Parcels', id: result.id }, 'Parcels']
          : ['Parcels'],
    }),
    
    // Obter parcels por proprietário
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

### Endpoint de Mutação (Escrita)

```tsx
// src/features/land/land.client.ts (continuação)
export const landClient = client.injectEndpoints({
  endpoints: (build) => ({
    // ... endpoints de consulta ...
    
    // Atualizar nome do parcel
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
    
    // Transferir parcel
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
        'Parcels', // Invalida a lista para atualizar os filtros por proprietário
      ],
    }),
  }),
});

export const {
  useUpdateParcelNameMutation,
  useTransferParcelMutation,
} = landClient;
```

## Atualizações Otimistas

Use `onQueryStarted` para atualizações otimistas da UI com rollback automático em caso de falha.

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
      
      // Atualização otimista
      async onQueryStarted({ address, amount }, { dispatch, queryFulfilled }) {
        // Atualize o cache de forma otimista
        const patchResult = dispatch(
          client.util.updateQueryData('getBalance', { address }, (draft) => {
            draft.amount += amount;
            draft.lastUpdated = new Date().toISOString();
          })
        );
        
        try {
          // Aguarde a mutação ser concluída
          const { data } = await queryFulfilled;
          
          // Atualize com a resposta do servidor
          dispatch(
            client.util.updateQueryData('getBalance', { address }, (draft) => {
              draft.amount = data.newBalance;
            })
          );
        } catch {
          // Reverter em caso de falha
          patchResult.undo();
        }
      },
      
      // Também invalide para garantir consistência
      invalidatesTags: (result, error, arg) => [
        { type: 'Credits', id: arg.address }
      ],
    }),
  }),
});

export const { useGetBalanceQuery, useGrantCreditsMutation } = creditsClient;
```

## Opções Avançadas de Consulta

### Consulta periódica

```tsx
// Consultar a cada 10 segundos
const { data } = useGetBalanceQuery(
  { address },
  { pollingInterval: 10000 }
);
```

### Consulta condicional

```tsx
// Ignorar a consulta se o address não estiver disponível
const { data } = useGetBalanceQuery(
  { address: address! },
  { skip: !address }
);
```

### Consulta sob demanda

```tsx
const [trigger, result] = useLazyGetParcelByCoordsQuery();

// Disparar manualmente
const handleClick = () => {
  trigger({ x: 10, y: 20 });
};
```

### Transformar resposta

```tsx
getParcel: build.query<Parcel, string>({
  query: (id) => `/v1/lands/${id}`,
  transformResponse: (response: ApiResponse<Parcel>) => response.data,
})
```

### Serialização personalizada

Para paginação ou pesquisa, personalize a serialização da chave de cache:

```tsx
searchParcels: build.query<Parcel[], { q: string; owner?: string; page?: number }>({
  query: (args) => ({
    url: '/v1/parcels/search',
    params: args,
  }),
  
  // Chave de cache personalizada para lidar com parâmetros opcionais
  serializeQueryArgs: ({ endpointName, queryArgs }) => {
    const { q, owner = 'any', page = 1 } = queryArgs;
    return `${endpointName}-${q}-${owner}-${page}`;
  },
  
  // Mesclar resultados para paginação
  merge(currentCache, newItems, { arg }) {
    if (arg.page === 1) {
      return newItems;
    }
    return [...currentCache, ...newItems];
  },
  
  // Forçar nova busca quando os argumentos mudarem
  forceRefetch({ currentArg, previousArg }) {
    return JSON.stringify(currentArg) !== JSON.stringify(previousArg);
  },
  
  providesTags: ['Parcels'],
})
```

## Tratamento de erros

### Tratamento Personalizado de Erros

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

Uso nos componentes:

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

## Gerenciamento de Cache

### Atualizações Manuais de Cache

```tsx
// Atualize o cache diretamente
dispatch(
  client.util.updateQueryData('getBalance', { address }, (draft) => {
    draft.amount = 1000;
  })
);
```

### Invalidar Cache

```tsx
// Invalidar todas as consultas de Credits
dispatch(client.util.invalidateTags(['Credits']));

// Invalidar entidade específica
dispatch(client.util.invalidateTags([{ type: 'Credits', id: address }]));
```

### Redefinir Estado do Cliente

```tsx
// Redefinir todo o estado do cliente
dispatch(client.util.resetApiState());
```

### Pré-carregar Dados

```tsx
// Pré-carregar dados antes da navegação
dispatch(
  client.util.prefetch('getParcel', { id: '123' }, { force: false })
);
```

## Melhores práticas

### 1. Use Nomes Descritivos para Endpoints

```tsx
// ✅ Bom
getParcelByCoords
getParcelsByOwner
updateParcelName

// ❌ Ruim
getParcel
fetch
update
```

### 2. Forneça Tags Abrangentes

```tsx
// ✅ Bom: Fornece tags de lista e de entidade
providesTags: (result) =>
  result
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// ❌ Ruim: Fornece apenas tag de lista
providesTags: ['Parcels']
```

### 3. Lide com Estados de Carregamento e Erro

```tsx
// ✅ Bom: Tratamento completo de estado
const { data, isLoading, isFetching, isError, error } = useGetParcelQuery({ id });

if (isLoading) return <Spinner />;
if (isError) return <Error error={error} />;
if (!data) return null;

// ❌ Ruim: Tratamento incompleto de estado
const { data } = useGetParcelQuery({ id });
return <div>{data.name}</div>; // Pode travar se data for undefined
```

### 4. Use Type Guards

```tsx
// ✅ Bom: Tratamento de erro com segurança de tipos
if (isFetchBaseQueryError(error)) {
  // Tratar erro de busca
} else if (isErrorWithMessage(error)) {
  // Tratar erro com mensagem
}

// ❌ Ruim: Cast inseguro de tipos
const message = (error as any).message;
```

## Próximos Passos

* Saiba mais sobre [Gerenciamento de estado](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/state-management/) para estado local da UI
* Revisar [Padrões de Componentes](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/component-patterns/) para exemplos de uso
* Entenda [Integração com Web3](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/web3-integration/) para dados da blockchain


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/rtk-query.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
