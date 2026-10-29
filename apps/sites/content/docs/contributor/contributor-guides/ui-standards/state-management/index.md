# State Management

This page covers creating Redux slices for UI and local state management using Redux Toolkit's `createSlice` and `createEntityAdapter`.

## When to Use Slices vs RTK Query

Choose the right tool for your state:

| State Type | Tool | Examples |
|------------|------|----------|
| **Remote data** (server-owned) | RTK Query | User profiles, NFTs, catalog items, orders |
| **UI state** (client-owned) | createSlice | Filters, modals, view preferences, form state |
| **Normalized collections** | createEntityAdapter | Sorted/filtered lists, optimistic updates |


> [!WARNING]
> **Do not duplicate the same data** in both a slice and RTK Query. Choose one source of truth.


## Creating a Basic Slice

Use `createSlice` for simple UI state:

```tsx
// src/features/ui/ui.slice.ts
import { createSlice, PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';

interface UIState {
  sidebarOpen: boolean;
  modalOpen: boolean;
  viewMode: 'grid' | 'list';
  theme: 'light' | 'dark';
}

const initialState: UIState = {
  sidebarOpen: true,
  modalOpen: false,
  viewMode: 'grid',
  theme: 'light',
};

const uiSlice = createSlice({
  name: 'ui',
  initialState,
  reducers: {
    // Boolean toggles
    sidebarToggled(state) {
      state.sidebarOpen = !state.sidebarOpen;
    },
    
    // Set specific value
    modalOpened(state) {
      state.modalOpen = true;
    },
    
    modalClosed(state) {
      state.modalOpen = false;
    },
    
    // Payload actions
    viewModeChanged(state, action: PayloadAction<'grid' | 'list'>) {
      state.viewMode = action.payload;
    },
    
    themeChanged(state, action: PayloadAction<'light' | 'dark'>) {
      state.theme = action.payload;
    },
    
    // Multiple properties
    uiReset() {
      return initialState;
    },
  },
});

// Export actions
export const {
  sidebarToggled,
  modalOpened,
  modalClosed,
  viewModeChanged,
  themeChanged,
  uiReset,
} = uiSlice.actions;

// Export reducer
export default uiSlice.reducer;

// Export selectors
export const selectSidebarOpen = (state: RootState) => state.ui.sidebarOpen;
export const selectModalOpen = (state: RootState) => state.ui.modalOpen;
export const selectViewMode = (state: RootState) => state.ui.viewMode;
export const selectTheme = (state: RootState) => state.ui.theme;
```

## Using Entity Adapters

For normalized collections (lists with IDs), use `createEntityAdapter`:

```tsx
// src/features/credits/credits.slice.ts
import { createSlice, createEntityAdapter, PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';

export type CreditTransaction = {
  id: string;
  address: string;
  amount: number;
  type: 'grant' | 'spend';
  timestamp: number;
  description?: string;
};

// Create entity adapter
const txAdapter = createEntityAdapter<CreditTransaction>({
  selectId: (tx) => tx.id,
  sortComparer: (a, b) => b.timestamp - a.timestamp, // Latest first
});

// Create slice with adapter's initial state
const creditsSlice = createSlice({
  name: 'credits',
  initialState: txAdapter.getInitialState({
    sending: false,
    error: null as string | null,
  }),
  reducers: {
    // Add a single transaction
    txAdded: txAdapter.addOne,
    
    // Add multiple transactions
    txsAdded: txAdapter.addMany,
    
    // Update a transaction
    txUpdated: txAdapter.updateOne,
    
    // Remove a transaction
    txRemoved: txAdapter.removeOne,
    
    // Clear all transactions
    txsCleared: txAdapter.removeAll,
    
    // Custom reducer with extra state
    sendingStarted(state) {
      state.sending = true;
      state.error = null;
    },
    
    sendingSucceeded(state, action: PayloadAction<CreditTransaction>) {
      state.sending = false;
      txAdapter.addOne(state, action.payload);
    },
    
    sendingFailed(state, action: PayloadAction<string>) {
      state.sending = false;
      state.error = action.payload;
    },
  },
});

// Export actions
export const {
  txAdded,
  txsAdded,
  txUpdated,
  txRemoved,
  txsCleared,
  sendingStarted,
  sendingSucceeded,
  sendingFailed,
} = creditsSlice.actions;

// Export reducer
export default creditsSlice.reducer;

// Create selectors
const selectCreditsState = (state: RootState) => state.credits;

export const creditsSelectors = txAdapter.getSelectors(selectCreditsState);

// Additional custom selectors
export const selectIsSending = (state: RootState) => state.credits.sending;
export const selectError = (state: RootState) => state.credits.error;

// Memoized selectors
export const selectTotalCredits = (state: RootState) => {
  const txs = creditsSelectors.selectAll(state);
  return txs.reduce((total, tx) => {
    return total + (tx.type === 'grant' ? tx.amount : -tx.amount);
  }, 0);
};
```

## Entity Adapter Methods

### State Mutations

```tsx
// Add
txAdapter.addOne(state, entity)
txAdapter.addMany(state, entities)

// Update
txAdapter.updateOne(state, { id, changes })
txAdapter.updateMany(state, updates)

// Upsert (add or update)
txAdapter.upsertOne(state, entity)
txAdapter.upsertMany(state, entities)

// Remove
txAdapter.removeOne(state, id)
txAdapter.removeMany(state, ids)
txAdapter.removeAll(state)

// Set (replace all)
txAdapter.setAll(state, entities)
txAdapter.setOne(state, entity)
txAdapter.setMany(state, entities)
```

### Generated Selectors

```tsx
const selectors = txAdapter.getSelectors(selectState);

// Select all entities as array
selectors.selectAll(state)

// Select entities as { [id]: entity }
selectors.selectEntities(state)

// Select all IDs as array
selectors.selectIds(state)

// Select total count
selectors.selectTotal(state)

// Select single entity by ID
selectors.selectById(state, id)
```

## Complex State Example

Combining multiple concerns in one slice:

```tsx
// src/features/land/land.slice.ts
import { createSlice, createEntityAdapter, PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';

export type LandFilter = {
  owner?: string;
  minPrice?: number;
  maxPrice?: number;
  types?: ('parcel' | 'estate')[];
};

export type SelectedParcel = {
  x: number;
  y: number;
  id?: string;
};

const selectedParcelsAdapter = createEntityAdapter<SelectedParcel>({
  selectId: (p) => `${p.x},${p.y}`,
});

interface LandState {
  // View state
  mapCenter: { x: number; y: number };
  mapZoom: number;
  
  // Filter state
  filters: LandFilter;
  
  // Selection state (using adapter)
  selectedParcels: ReturnType<typeof selectedParcelsAdapter.getInitialState>;
  
  // UI state
  showGrid: boolean;
  highlightOwned: boolean;
}

const initialState: LandState = {
  mapCenter: { x: 0, y: 0 },
  mapZoom: 1,
  filters: {},
  selectedParcels: selectedParcelsAdapter.getInitialState(),
  showGrid: true,
  highlightOwned: false,
};

const landSlice = createSlice({
  name: 'land',
  initialState,
  reducers: {
    // Map controls
    mapCenterChanged(state, action: PayloadAction<{ x: number; y: number }>) {
      state.mapCenter = action.payload;
    },
    
    mapZoomed(state, action: PayloadAction<number>) {
      state.mapZoom = action.payload;
    },
    
    // Filters
    filtersUpdated(state, action: PayloadAction<Partial<LandFilter>>) {
      state.filters = { ...state.filters, ...action.payload };
    },
    
    filtersCleared(state) {
      state.filters = {};
    },
    
    // Selection
    parcelSelected(state, action: PayloadAction<SelectedParcel>) {
      selectedParcelsAdapter.addOne(state.selectedParcels, action.payload);
    },
    
    parcelDeselected(state, action: PayloadAction<string>) {
      selectedParcelsAdapter.removeOne(state.selectedParcels, action.payload);
    },
    
    selectionCleared(state) {
      selectedParcelsAdapter.removeAll(state.selectedParcels);
    },
    
    // UI toggles
    gridToggled(state) {
      state.showGrid = !state.showGrid;
    },
    
    ownedHighlightToggled(state) {
      state.highlightOwned = !state.highlightOwned;
    },
  },
});

export const {
  mapCenterChanged,
  mapZoomed,
  filtersUpdated,
  filtersCleared,
  parcelSelected,
  parcelDeselected,
  selectionCleared,
  gridToggled,
  ownedHighlightToggled,
} = landSlice.actions;

export default landSlice.reducer;

// Selectors
export const selectMapCenter = (state: RootState) => state.land.mapCenter;
export const selectMapZoom = (state: RootState) => state.land.mapZoom;
export const selectFilters = (state: RootState) => state.land.filters;
export const selectShowGrid = (state: RootState) => state.land.showGrid;
export const selectHighlightOwned = (state: RootState) => state.land.highlightOwned;

// Selection selectors
const selectSelectedParcelsState = (state: RootState) => state.land.selectedParcels;
export const selectedParcelsSelectors = selectedParcelsAdapter.getSelectors(
  selectSelectedParcelsState
);
```

## Memoized Selectors

Use `createSelector` from Reselect for computed/derived state:

```tsx
// src/features/land/land.selectors.ts
import { createSelector } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';
import { selectFilters } from './land.slice';

// Expensive filtering logic - memoized
export const selectActiveFiltersCount = createSelector(
  [selectFilters],
  (filters) => {
    return Object.values(filters).filter(Boolean).length;
  }
);

// Combine multiple selectors
export const selectHasActiveFilters = createSelector(
  [selectActiveFiltersCount],
  (count) => count > 0
);

// Multiple inputs
export const selectFilteredParcels = createSelector(
  [
    (state: RootState) => state.land.allParcels, // assuming this exists
    selectFilters,
  ],
  (parcels, filters) => {
    return parcels.filter((parcel) => {
      if (filters.owner && parcel.owner !== filters.owner) return false;
      if (filters.minPrice && parcel.price < filters.minPrice) return false;
      if (filters.maxPrice && parcel.price > filters.maxPrice) return false;
      if (filters.types && !filters.types.includes(parcel.type)) return false;
      return true;
    });
  }
);
```

## Async Logic with Extra Reducers

Handle RTK Query or async thunk responses in your slice:

```tsx
import { createSlice } from '@reduxjs/toolkit';
import { creditsClient } from './credits.client';

const slice = createSlice({
  name: 'credits',
  initialState: { lastGranted: null as number | null },
  reducers: {},
  extraReducers: (builder) => {
    builder
      .addMatcher(
        creditsClient.endpoints.grantCredits.matchFulfilled,
        (state, action) => {
          state.lastGranted = action.payload.newBalance;
        }
      )
      .addMatcher(
        creditsClient.endpoints.grantCredits.matchRejected,
        (state) => {
          state.lastGranted = null;
        }
      );
  },
});
```

## Best Practices

### 1. Keep State Minimal

```tsx
// ✅ Good: Only store what you need
interface State {
  userId: string | null;
  isAuthenticated: boolean;
}

// ❌ Bad: Storing derived/computed values
interface State {
  userId: string | null;
  isAuthenticated: boolean;
  hasUserId: boolean; // Can be computed
  userIdLength: number; // Can be computed
}
```

### 2. Use Immer-Friendly Mutations

```tsx
// ✅ Good: Direct mutation (Immer handles it)
reducers: {
  itemAdded(state, action) {
    state.items.push(action.payload);
    state.count += 1;
  }
}

// ❌ Bad: Manual spread (unnecessary)
reducers: {
  itemAdded(state, action) {
    return {
      ...state,
      items: [...state.items, action.payload],
      count: state.count + 1,
    };
  }
}
```

### 3. Organize Reducers Logically

```tsx
// ✅ Good: Grouped by functionality
reducers: {
  // Modal controls
  modalOpened(state) { ... },
  modalClosed(state) { ... },
  
  // Filter controls
  filterApplied(state, action) { ... },
  filterCleared(state) { ... },
  
  // Reset
  stateReset() { return initialState; },
}
```

### 4. Type Actions Properly

```tsx
// ✅ Good: Explicit payload type
userUpdated(state, action: PayloadAction<{ id: string; name: string }>) {
  state.user = action.payload;
}

// ❌ Bad: Untyped payload
userUpdated(state, action) {
  state.user = action.payload; // No type safety
}
```

## Testing Slices

```tsx
// land.slice.test.ts
import reducer, { mapCenterChanged, mapZoomed } from './land.slice';

describe('land slice', () => {
  const initialState = {
    mapCenter: { x: 0, y: 0 },
    mapZoom: 1,
    // ... other state
  };

  it('should handle mapCenterChanged', () => {
    const newCenter = { x: 10, y: 20 };
    const actual = reducer(initialState, mapCenterChanged(newCenter));
    expect(actual.mapCenter).toEqual(newCenter);
  });

  it('should handle mapZoomed', () => {
    const actual = reducer(initialState, mapZoomed(2));
    expect(actual.mapZoom).toBe(2);
  });
});
```

## Next Steps

* Review [Component Patterns](/docs/contributor/contributor-guides/ui-standards/component-patterns/) for using slices in components
* Learn about [Web3 Integration](/docs/contributor/contributor-guides/ui-standards/web3-integration/) for blockchain state
* See [Testing & Performance](/docs/contributor/contributor-guides/ui-standards/testing-and-performance/) for optimization tips
