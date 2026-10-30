# Testing & Performance

This page covers testing strategies for Redux/RTK Query applications and performance optimization techniques.

## Testing Redux Slices

### Basic Reducer Tests

```tsx
// user.slice.test.ts
import reducer, { userLoggedIn, userLoggedOut } from './user.slice';

describe('user slice', () => {
  const initialState = {
    account: null,
    isAuthenticated: false,
  };

  it('should handle userLoggedIn', () => {
    const account = '0x123...';
    const actual = reducer(initialState, userLoggedIn({ account }));
    
    expect(actual.account).toBe(account);
    expect(actual.isAuthenticated).toBe(true);
  });

  it('should handle userLoggedOut', () => {
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

### Testing Entity Adapters

```tsx
// credits.slice.test.ts
import reducer, { txAdded, txsCleared, creditsSelectors } from './credits.slice';

describe('credits slice with entity adapter', () => {
  it('should add a transaction', () => {
    const initialState = reducer(undefined, { type: 'unknown' });
    const tx = { id: '1', address: '0x123', amount: 100, type: 'grant', timestamp: Date.now() };
    
    const actual = reducer(initialState, txAdded(tx));
    
    expect(creditsSelectors.selectById({ credits: actual }, '1')).toEqual(tx);
    expect(creditsSelectors.selectTotal({ credits: actual })).toBe(1);
  });

  it('should clear all transactions', () => {
    const initialState = reducer(undefined, { type: 'unknown' });
    const withTx = reducer(initialState, txAdded({ id: '1', /* ... */ }));
    
    const actual = reducer(withTx, txsCleared());
    
    expect(creditsSelectors.selectTotal({ credits: actual })).toBe(0);
  });
});
```

## Testing Selectors

### Simple Selectors

```tsx
// user.selectors.test.ts
import { selectAccount, selectIsAuthenticated } from './user.slice';

describe('user selectors', () => {
  const mockState = {
    user: {
      account: '0x123...',
      isAuthenticated: true,
    },
    // ... other slices
  };

  it('should select account', () => {
    expect(selectAccount(mockState)).toBe('0x123...');
  });

  it('should select authentication status', () => {
    expect(selectIsAuthenticated(mockState)).toBe(true);
  });
});
```

### Memoized Selectors

```tsx
// land.selectors.test.ts
import { selectFilteredParcels, selectActiveFiltersCount } from './land.selectors';

describe('land selectors', () => {
  const mockState = {
    land: {
      filters: { owner: '0x123', minPrice: 100 },
      parcels: [
        { id: '1', owner: '0x123', price: 150 },
        { id: '2', owner: '0x456', price: 200 },
      ],
    },
  };

  it('should count active filters', () => {
    expect(selectActiveFiltersCount(mockState)).toBe(2);
  });

  it('should filter parcels', () => {
    const result = selectFilteredParcels(mockState);
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('1');
  });

  it('should memoize results', () => {
    const result1 = selectFilteredParcels(mockState);
    const result2 = selectFilteredParcels(mockState);
    
    // Same reference = memoized
    expect(result1).toBe(result2);
  });
});
```

## Testing RTK Query with MSW

### Setup MSW

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
      name: 'Test Parcel',
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

### Testing Queries

```tsx
// land.client.test.tsx
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { setupStore } from '@/app/store';
import { useGetParcelQuery } from './land.client';

function TestComponent({ id }: { id: string }) {
  const { data, isLoading, isError } = useGetParcelQuery({ id });

  if (isLoading) return <div>Loading...</div>;
  if (isError) return <div>Error</div>;
  if (!data) return null;

  return <div>{data.name}</div>;
}

describe('land client', () => {
  it('should fetch and display parcel data', async () => {
    const store = setupStore();

    render(
      <Provider store={store}>
        <TestComponent id="1" />
      </Provider>
    );

    expect(screen.getByText('Loading...')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('Test Parcel')).toBeInTheDocument();
    });
  });
});
```

### Testing Mutations

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
        Grant
      </button>
      {isLoading && <div>Loading...</div>}
      {isSuccess && <div>Success!</div>}
    </div>
  );
}

describe('credits client mutations', () => {
  it('should grant credits successfully', async () => {
    const store = setupStore();

    render(
      <Provider store={store}>
        <TestComponent />
      </Provider>
    );

    fireEvent.click(screen.getByText('Grant'));

    await waitFor(() => {
      expect(screen.getByText('Success!')).toBeInTheDocument();
    });
  });
});
```

### Testing Optimistic Updates

```tsx
// credits.client.test.ts
import { server } from '@/test/server';
import { http, HttpResponse } from 'msw';
import { setupStore } from '@/app/store';
import { creditsClient } from './credits.client';

describe('optimistic updates', () => {
  it('should update cache optimistically and rollback on error', async () => {
    const store = setupStore();
    const address = '0x123';

    // Prefetch initial balance
    await store.dispatch(
      creditsClient.endpoints.getBalance.initiate({ address })
    );

    const initialBalance = creditsClient.endpoints.getBalance.select({ address })(
      store.getState()
    ).data?.amount;

    expect(initialBalance).toBe(100); // from MSW handler

    // Mock failure
    server.use(
      http.post('/api/v1/credits/grant', () => {
        return HttpResponse.json({ error: 'Failed' }, { status: 500 });
      })
    );

    // Trigger mutation
    const mutation = store.dispatch(
      creditsClient.endpoints.grantCredits.initiate({
        address,
        amount: 50,
      })
    );

    // Check optimistic update
    const optimisticBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(optimisticBalance).toBe(150); // 100 + 50

    // Wait for mutation to fail
    await expect(mutation).rejects.toThrow();

    // Check rollback
    const rolledBackBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(rolledBackBalance).toBe(100); // Back to original
  });
});
```

## Performance Optimization

### Use `selectFromResult` to Prevent Re-renders

```tsx
// ✅ Good: Only subscribes to specific fields
const { owner } = useGetParcelQuery(
  { id },
  {
    selectFromResult: ({ data }) => ({
      owner: data?.owner,
    }),
  }
);

// Component only re-renders when owner changes
```

### Avoid Selecting Entire State

```tsx
// ✅ Good: Select specific values
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ Bad: Selects entire slice
const ui = useAppSelector((state) => state.ui);
```

### Memoize Expensive Selectors

```tsx
import { createSelector } from '@reduxjs/toolkit';

// ✅ Good: Memoized selector
export const selectFilteredParcels = createSelector(
  [selectAllParcels, selectFilters],
  (parcels, filters) => {
    // Expensive filtering logic
    return parcels.filter(/* ... */);
  }
);

// ❌ Bad: Computed in component
function Component() {
  const parcels = useAppSelector(selectAllParcels);
  const filters = useAppSelector(selectFilters);
  
  // Re-computes on every render!
  const filtered = parcels.filter(/* ... */);
}
```

### Use Entity Adapters for Normalized Data

```tsx
// ✅ Good: Normalized with entity adapter
const adapter = createEntityAdapter<Parcel>();

// Efficient lookups by ID
const parcel = adapter.getSelectors().selectById(state, id);

// ❌ Bad: Array lookup
const parcel = state.parcels.find((p) => p.id === id);
```

### Tune RTK Query Cache Settings

```tsx
export const client = createApi({
  // ...
  keepUnusedDataFor: 60, // Keep data for 60 seconds
  refetchOnMountOrArgChange: 30, // Refetch if data is older than 30s
  refetchOnFocus: true, // Refetch when window regains focus
  refetchOnReconnect: true, // Refetch when reconnecting
});
```

### Prefetch for Better UX

```tsx
function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // Prefetch on hover
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

### Polling Strategy

```tsx
// Poll only when needed
const { data } = useGetBalanceQuery(
  { address },
  {
    pollingInterval: isActive ? 10000 : 0, // Poll only when active
    skipPollingIfUnfocused: true, // Pause when tab not focused
  }
);
```

## Redux DevTools

### Enable in Development

```tsx
export const store = configureStore({
  // ...
  devTools: process.env.NODE_ENV !== 'production',
});
```

### Action Sanitizer

Sanitize sensitive data in DevTools:

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

## Best Practices Checklist

### Performance

- [ ]  Use `selectFromResult` for large query results
- [ ]  Memoize expensive selectors with `createSelector`
- [ ]  Use entity adapters for normalized collections
- [ ]  Avoid selecting entire slices in components
- [ ]  Tune `keepUnusedDataFor` based on your use case
- [ ]  Prefetch data before navigation
- [ ]  Use polling strategically (only when needed)

### Testing

- [ ]  Unit test all reducers and actions
- [ ]  Test memoized selectors for correctness and performance
- [ ]  Use MSW for RTK Query endpoint tests
- [ ]  Test optimistic updates and rollback logic
- [ ]  Test error handling in components
- [ ]  Write integration tests for critical flows

### Code Quality

- [ ]  Use typed hooks (`useAppSelector`, `useAppDispatch`)
- [ ]  Handle all query states (loading, error, success)
- [ ]  Use `.unwrap()` for mutation error handling
- [ ]  Invalidate or update cache after mutations
- [ ]  Keep non-serializable data out of Redux
- [ ]  Document complex selectors and logic

## Anti-Patterns to Avoid


> [!CAUTION]
> **Don't do these:**
> 
> 1. Store non-serializable objects (providers, signers) in Redux
> 2. Duplicate data in both slices and RTK Query
> 3. Dispatch actions during render
> 4. Create selectors that return new objects without memoization
> 5. Ignore loading and error states
> 6. Fetch the same data in multiple components without RTK Query
> 7. Over-poll or poll without `skipPollingIfUnfocused`


## Monitoring Performance

### Track Selector Calls

```tsx
import { createSelector } from '@reduxjs/toolkit';

const selectExpensiveData = createSelector(
  [selectData],
  (data) => {
    console.log('Selector called'); // Should only log when data changes
    return expensiveOperation(data);
  }
);
```

### Monitor Re-renders

```tsx
import { useEffect, useRef } from 'react';

function useRenderCount() {
  const renderCount = useRef(0);
  
  useEffect(() => {
    renderCount.current += 1;
    console.log('Render count:', renderCount.current);
  });
}

function Component() {
  useRenderCount(); // Track re-renders
  // ...
}
```

## Next Steps

* Review [Component Patterns](/docs/contributor/contributor-guides/ui-standards/component-patterns/) for usage examples
* See [RTK Query](/docs/contributor/contributor-guides/ui-standards/rtk-query/) for caching strategies
* Understand [State Management](/docs/contributor/contributor-guides/ui-standards/state-management/) for slice optimization
