# RTK Query

RTK Query is Redux Toolkit's powerful data fetching and caching layer. It eliminates the need to write async action creators and manages loading states, caching, and data synchronization automatically.

## Base Client Configuration

All RTK Query endpoints extend from a single base client instance.

### Base Query Setup

```tsx
// src/services/baseQuery.ts
import { fetchBaseQuery } from '@reduxjs/toolkit/query/react';
import type { RootState } from '@/app/store';

export const baseQuery = fetchBaseQuery({
  baseUrl: process.env.NEXT_PUBLIC_API_URL, // e.g., https://api.decentraland.org
  
  prepareHeaders: (headers, { getState }) => {
    const state = getState() as RootState;
    
    // Add authentication token
    const token = state.user.session?.authToken;
    if (token) {
      headers.set('authorization', `Bearer ${token}`);
    }
    
    // Add chain ID for web3 context
    const chainId = state.user.chainId;
    if (chainId) {
      headers.set('x-chain-id', String(chainId));
    }
    
    // Standard headers
    headers.set('accept', 'application/json');
    headers.set('content-type', 'application/json');
    
    return headers;
  },
  
  credentials: 'omit', // or 'include' if backend requires cookies
});
```

### Client Instance

```tsx
// src/services/client.ts
import { createApi } from '@reduxjs/toolkit/query/react';
import { baseQuery } from './baseQuery';

export const client = createApi({
  reducerPath: 'client',
  baseQuery,
  
  // Define all possible tag types for cache invalidation
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
  
  // Cache configuration
  keepUnusedDataFor: 60,         // Keep unused data for 60 seconds
  refetchOnFocus: true,           // Refetch when window regains focus
  refetchOnReconnect: true,       // Refetch when reconnecting
  refetchOnMountOrArgChange: 30,  // Refetch if data is older than 30s
  
  // Endpoints will be injected in feature files
  endpoints: () => ({}),
});
```

## Tag Conventions

Tags are used for cache invalidation and synchronization. Follow these conventions:

### Tag Naming

| Resource Type | Query Tags | Mutation Invalidates |
|--------------|------------|---------------------|
| **Collections** | `'Parcels'` (plural) | `'Parcels'` |
| **Single Entity** | `{type: 'Parcels', id: '123'}` | `{type: 'Parcels', id: '123'}` |
| **List + Detail** | `['Parcels', {type: 'Parcels', id}]` | `['Parcels']` or specific id |

### Tag Examples

```tsx
// Collection: provides list tag
providesTags: ['Parcels']

// Single entity: provides specific tag + list tag
providesTags: (result) => 
  result 
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// Mutation: invalidates both list and specific entity
invalidatesTags: (result, error, arg) => [
  { type: 'Parcels', id: arg.id },
  'Parcels'
]
```

## Creating Endpoints

Endpoints SHOULD be co-located with their feature in `feature.client.ts` files.

### Query Endpoint (Read)

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
    // Get all tiles
    getTiles: build.query<Record<string, Tile>, void>({
      query: () => '/v1/tiles',
      providesTags: ['Parcels'],
    }),
    
    // Get parcel by coordinates
    getParcelByCoords: build.query<Parcel, { x: number; y: number }>({
      query: ({ x, y }) => `/v1/lands/${x}/${y}`,
      providesTags: (result, error, arg) =>
        result
          ? [{ type: 'Parcels', id: result.id }, 'Parcels']
          : ['Parcels'],
    }),
    
    // Get parcels by owner
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

// Export hooks
export const {
  useGetTilesQuery,
  useGetParcelByCoordsQuery,
  useGetParcelsByOwnerQuery,
} = landClient;
```

### Mutation Endpoint (Write)

```tsx
// src/features/land/land.client.ts (continued)
export const landClient = client.injectEndpoints({
  endpoints: (build) => ({
    // ... query endpoints ...
    
    // Update parcel name
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
    
    // Transfer parcel
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
        'Parcels', // Invalidate list to update owner filters
      ],
    }),
  }),
});

export const {
  useUpdateParcelNameMutation,
  useTransferParcelMutation,
} = landClient;
```

## Optimistic Updates

Use `onQueryStarted` for optimistic UI updates with automatic rollback on failure.

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
      
      // Optimistic update
      async onQueryStarted({ address, amount }, { dispatch, queryFulfilled }) {
        // Optimistically update the cache
        const patchResult = dispatch(
          client.util.updateQueryData('getBalance', { address }, (draft) => {
            draft.amount += amount;
            draft.lastUpdated = new Date().toISOString();
          })
        );
        
        try {
          // Wait for the mutation to complete
          const { data } = await queryFulfilled;
          
          // Update with server response
          dispatch(
            client.util.updateQueryData('getBalance', { address }, (draft) => {
              draft.amount = data.newBalance;
            })
          );
        } catch {
          // Rollback on failure
          patchResult.undo();
        }
      },
      
      // Also invalidate to ensure consistency
      invalidatesTags: (result, error, arg) => [
        { type: 'Credits', id: arg.address }
      ],
    }),
  }),
});

export const { useGetBalanceQuery, useGrantCreditsMutation } = creditsClient;
```

## Advanced Query Options

### Polling

```tsx
// Poll every 10 seconds
const { data } = useGetBalanceQuery(
  { address },
  { pollingInterval: 10000 }
);
```

### Skip Query

```tsx
// Skip query if address is not available
const { data } = useGetBalanceQuery(
  { address: address! },
  { skip: !address }
);
```

### Lazy Query

```tsx
const [trigger, result] = useLazyGetParcelByCoordsQuery();

// Trigger manually
const handleClick = () => {
  trigger({ x: 10, y: 20 });
};
```

### Transform Response

```tsx
getParcel: build.query<Parcel, string>({
  query: (id) => `/v1/lands/${id}`,
  transformResponse: (response: ApiResponse<Parcel>) => response.data,
})
```

### Custom Serialization

For pagination or search, customize cache key serialization:

```tsx
searchParcels: build.query<Parcel[], { q: string; owner?: string; page?: number }>({
  query: (args) => ({
    url: '/v1/parcels/search',
    params: args,
  }),
  
  // Custom cache key to handle optional params
  serializeQueryArgs: ({ endpointName, queryArgs }) => {
    const { q, owner = 'any', page = 1 } = queryArgs;
    return `${endpointName}-${q}-${owner}-${page}`;
  },
  
  // Merge results for pagination
  merge(currentCache, newItems, { arg }) {
    if (arg.page === 1) {
      return newItems;
    }
    return [...currentCache, ...newItems];
  },
  
  // Force refetch when args change
  forceRefetch({ currentArg, previousArg }) {
    return JSON.stringify(currentArg) !== JSON.stringify(previousArg);
  },
  
  providesTags: ['Parcels'],
})
```

## Error Handling

### Custom Error Handling

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

Usage in components:

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

## Cache Management

### Manual Cache Updates

```tsx
// Update cache directly
dispatch(
  client.util.updateQueryData('getBalance', { address }, (draft) => {
    draft.amount = 1000;
  })
);
```

### Invalidate Cache

```tsx
// Invalidate all Credits queries
dispatch(client.util.invalidateTags(['Credits']));

// Invalidate specific entity
dispatch(client.util.invalidateTags([{ type: 'Credits', id: address }]));
```

### Reset Client State

```tsx
// Reset entire client state
dispatch(client.util.resetApiState());
```

### Prefetch Data

```tsx
// Prefetch data before navigation
dispatch(
  client.util.prefetch('getParcel', { id: '123' }, { force: false })
);
```

## Best Practices

### 1. Use Descriptive Endpoint Names

```tsx
// ✅ Good
getParcelByCoords
getParcelsByOwner
updateParcelName

// ❌ Bad
getParcel
fetch
update
```

### 2. Provide Comprehensive Tags

```tsx
// ✅ Good: Provides both list and entity tags
providesTags: (result) =>
  result
    ? [{ type: 'Parcels', id: result.id }, 'Parcels']
    : ['Parcels']

// ❌ Bad: Only provides list tag
providesTags: ['Parcels']
```

### 3. Handle Loading and Error States

```tsx
// ✅ Good: Complete state handling
const { data, isLoading, isFetching, isError, error } = useGetParcelQuery({ id });

if (isLoading) return <Spinner />;
if (isError) return <Error error={error} />;
if (!data) return null;

// ❌ Bad: Incomplete state handling
const { data } = useGetParcelQuery({ id });
return <div>{data.name}</div>; // May crash if data is undefined
```

### 4. Use Type Guards

```tsx
// ✅ Good: Type-safe error handling
if (isFetchBaseQueryError(error)) {
  // Handle fetch error
} else if (isErrorWithMessage(error)) {
  // Handle error with message
}

// ❌ Bad: Unsafe type casting
const message = (error as any).message;
```

## Next Steps

* Learn about [State Management](/docs/contributor/contributor-guides/ui-standards/state-management/) for local UI state
* Review [Component Patterns](/docs/contributor/contributor-guides/ui-standards/component-patterns/) for usage examples
* Understand [Web3 Integration](/docs/contributor/contributor-guides/ui-standards/web3-integration/) for blockchain data
