# Gestión del estado

Esta página cubre la creación de slices de Redux para UI y la gestión del estado local usando Redux Toolkit `createSlice` y `createEntityAdapter`.

## Cuándo usar slices frente a RTK Query

Elige la herramienta adecuada para tu estado:

| Tipo de estado                             | Herramienta         | Ejemplos                                                       |
| ------------------------------------------ | ------------------- | -------------------------------------------------------------- |
| **Datos remotos** (propiedad del servidor) | RTK Query           | Perfiles de usuario, NFTs, elementos del catálogo, pedidos     |
| **Estado de UI** (propiedad del cliente)   | createSlice         | Filtros, modales, preferencias de vista, estado del formulario |
| **Colecciones normalizadas**               | createEntityAdapter | Listas ordenadas/filtradas, actualizaciones optimistas         |


> [!WARNING]
> **No dupliques los mismos datos** ni en un slice ni en RTK Query. Elige una única fuente de verdad.


## Creación de un slice básico

Usa `createSlice` para estado simple de UI:

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
    // Alternadores booleanos
    sidebarToggled(state) {
      state.sidebarOpen = !state.sidebarOpen;
    },
    
    // Establecer un valor específico
    modalOpened(state) {
      state.modalOpen = true;
    },
    
    modalClosed(state) {
      state.modalOpen = false;
    },
    
    // Acciones con payload
    viewModeChanged(state, action: PayloadAction<'grid' | 'list'>) {
      state.viewMode = action.payload;
    },
    
    themeChanged(state, action: PayloadAction<'light' | 'dark'>) {
      state.theme = action.payload;
    },
    
    // Varias propiedades
    uiReset() {
      return initialState;
    },
  },
});

// Exportar acciones
export const {
  sidebarToggled,
  modalOpened,
  modalClosed,
  viewModeChanged,
  themeChanged,
  uiReset,
} = uiSlice.actions;

// Exportar reducer
export default uiSlice.reducer;

// Exportar selectores
export const selectSidebarOpen = (state: RootState) => state.ui.sidebarOpen;
export const selectModalOpen = (state: RootState) => state.ui.modalOpen;
export const selectViewMode = (state: RootState) => state.ui.viewMode;
export const selectTheme = (state: RootState) => state.ui.theme;
```

## Uso de Entity Adapters

Para colecciones normalizadas (listas con IDs), usa `createEntityAdapter`:

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

// Crear entity adapter
const txAdapter = createEntityAdapter<CreditTransaction>({
  selectId: (tx) => tx.id,
  sortComparer: (a, b) => b.timestamp - a.timestamp, // Más recientes primero
});

// Crear un slice con el estado inicial del adapter
const creditsSlice = createSlice({
  name: 'credits',
  initialState: txAdapter.getInitialState({
    sending: false,
    error: null as string | null,
  }),
  reducers: {
    // Añadir una sola transacción
    txAdded: txAdapter.addOne,
    
    // Añadir varias transacciones
    txsAdded: txAdapter.addMany,
    
    // Actualizar una transacción
    txUpdated: txAdapter.updateOne,
    
    // Eliminar una transacción
    txRemoved: txAdapter.removeOne,
    
    // Limpiar todas las transacciones
    txsCleared: txAdapter.removeAll,
    
    // Reducer personalizado con estado adicional
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

// Exportar acciones
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

// Exportar reducer
export default creditsSlice.reducer;

// Crear selectores
const selectCreditsState = (state: RootState) => state.credits;

export const creditsSelectors = txAdapter.getSelectors(selectCreditsState);

// Selectores personalizados adicionales
export const selectIsSending = (state: RootState) => state.credits.sending;
export const selectError = (state: RootState) => state.credits.error;

// Selectores memorizados
export const selectTotalCredits = (state: RootState) => {
  const txs = creditsSelectors.selectAll(state);
  return txs.reduce((total, tx) => {
    return total + (tx.type === 'grant' ? tx.amount : -tx.amount);
  }, 0);
};
```

## Métodos de Entity Adapter

### Mutaciones de estado

```tsx
// Añadir
txAdapter.addOne(state, entity)
txAdapter.addMany(state, entities)

// Actualizar
txAdapter.updateOne(state, { id, changes })
txAdapter.updateMany(state, updates)

// Upsert (añadir o actualizar)
txAdapter.upsertOne(state, entity)
txAdapter.upsertMany(state, entities)

// Eliminar
txAdapter.removeOne(state, id)
txAdapter.removeMany(state, ids)
txAdapter.removeAll(state)

// Establecer (reemplazar todo)
txAdapter.setAll(state, entities)
txAdapter.setOne(state, entity)
txAdapter.setMany(state, entities)
```

### Selectores generados

```tsx
const selectors = txAdapter.getSelectors(selectState);

// Seleccionar todas las entidades como array
selectors.selectAll(state)

// Seleccionar entidades como { [id]: entity }
selectors.selectEntities(state)

// Seleccionar todos los IDs como array
selectors.selectIds(state)

// Seleccionar el total
selectors.selectTotal(state)

// Seleccionar una sola entidad por ID
selectors.selectById(state, id)
```

## Ejemplo de estado complejo

Combinando varias preocupaciones en un solo slice:

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
  // Estado de vista
  mapCenter: { x: number; y: number };
  mapZoom: number;
  
  // Estado de filtros
  filters: LandFilter;
  
  // Estado de selección (usando adapter)
  selectedParcels: ReturnType<typeof selectedParcelsAdapter.getInitialState>;
  
  // Estado de UI
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
    // Controles del mapa
    mapCenterChanged(state, action: PayloadAction<{ x: number; y: number }>) {
      state.mapCenter = action.payload;
    },
    
    mapZoomed(state, action: PayloadAction<number>) {
      state.mapZoom = action.payload;
    },
    
    // Filtros
    filtersUpdated(state, action: PayloadAction<Partial<LandFilter>>) {
      state.filters = { ...state.filters, ...action.payload };
    },
    
    filtersCleared(state) {
      state.filters = {};
    },
    
    // Selección
    parcelSelected(state, action: PayloadAction<SelectedParcel>) {
      selectedParcelsAdapter.addOne(state.selectedParcels, action.payload);
    },
    
    parcelDeselected(state, action: PayloadAction<string>) {
      selectedParcelsAdapter.removeOne(state.selectedParcels, action.payload);
    },
    
    selectionCleared(state) {
      selectedParcelsAdapter.removeAll(state.selectedParcels);
    },
    
    // Alternadores de UI
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

// Selectores
export const selectMapCenter = (state: RootState) => state.land.mapCenter;
export const selectMapZoom = (state: RootState) => state.land.mapZoom;
export const selectFilters = (state: RootState) => state.land.filters;
export const selectShowGrid = (state: RootState) => state.land.showGrid;
export const selectHighlightOwned = (state: RootState) => state.land.highlightOwned;

// Selectores de selección
const selectSelectedParcelsState = (state: RootState) => state.land.selectedParcels;
export const selectedParcelsSelectors = selectedParcelsAdapter.getSelectors(
  selectSelectedParcelsState
);
```

## Selectores memorizados

Usa `createSelector` de Reselect para estado calculado/derivado:

```tsx
// src/features/land/land.selectors.ts
import { createSelector } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';
import { selectFilters } from './land.slice';

// Lógica de filtrado costosa - memorizada
export const selectActiveFiltersCount = createSelector(
  [selectFilters],
  (filters) => {
    return Object.values(filters).filter(Boolean).length;
  }
);

// Combinar varios selectores
export const selectHasActiveFilters = createSelector(
  [selectActiveFiltersCount],
  (count) => count > 0
);

// Múltiples entradas
export const selectFilteredParcels = createSelector(
  [
    (state: RootState) => state.land.allParcels, // suponiendo que esto existe
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

## Lógica asíncrona con extraReducers

Maneja las respuestas de RTK Query o de async thunk en tu slice:

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

## Buenas prácticas

### 1. Mantén el estado al mínimo

```tsx
// ✅ Bien: Guarda solo lo que necesitas
interface State {
  userId: string | null;
  isAuthenticated: boolean;
}

// ❌ Mal: Guardar valores derivados/calculados
interface State {
  userId: string | null;
  isAuthenticated: boolean;
  hasUserId: boolean; // Se puede calcular
  userIdLength: number; // Se puede calcular
}
```

### 2. Usa mutaciones compatibles con Immer

```tsx
// ✅ Bien: Mutación directa (Immer se encarga)
reducers: {
  itemAdded(state, action) {
    state.items.push(action.payload);
    state.count += 1;
  }
}

// ❌ Mal: spread manual (innecesario)
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

### 3. Organiza los Reducers lógicamente

```tsx
// ✅ Bueno: Agrupado por funcionalidad
reducers: {
  // Controles del modal
  modalOpened(state) { ... },
  modalClosed(state) { ... },
  
  // Controles del filtro
  filterApplied(state, action) { ... },
  filterCleared(state) { ... },
  
  // Reiniciar
  stateReset() { return initialState; },
}
```

### 4. Tipa las Actions correctamente

```tsx
// ✅ Bueno: Tipo de payload explícito
userUpdated(state, action: PayloadAction<{ id: string; name: string }>) {
  state.user = action.payload;
}

// ❌ Malo: payload sin tipar
userUpdated(state, action) {
  state.user = action.payload; // Sin seguridad de tipos
}
```

## Pruebas de Slices

```tsx
// land.slice.test.ts
import reducer, { mapCenterChanged, mapZoomed } from './land.slice';

describe('land slice', () => {
  const initialState = {
    mapCenter: { x: 0, y: 0 },
    mapZoom: 1,
    // ... otro estado
  };

  it('debería manejar mapCenterChanged', () => {
    const newCenter = { x: 10, y: 20 };
    const actual = reducer(initialState, mapCenterChanged(newCenter));
    expect(actual.mapCenter).toEqual(newCenter);
  });

  it('debería manejar mapZoomed', () => {
    const actual = reducer(initialState, mapZoomed(2));
    expect(actual.mapZoom).toBe(2);
  });
});
```

## Próximos pasos

* Revisa [Patrones de Componentes](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/component-patterns/) para usar slices en components
* Aprende sobre [Integración con Web3](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/web3-integration/) para el estado de blockchain
* Ver [Pruebas y rendimiento](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/testing-and-performance/) para obtener consejos de optimización


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/state-management.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
