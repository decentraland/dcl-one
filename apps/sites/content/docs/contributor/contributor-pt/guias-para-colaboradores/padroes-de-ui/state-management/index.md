# Gestão de estado

Esta página aborda a criação de slices do Redux para UI e gerenciamento de estado local usando o `createSlice` e `createEntityAdapter`.

## Quando usar Slices vs RTK Query

Escolha a ferramenta certa para o seu estado:

| Tipo de Estado                               | Ferramenta          | Exemplos                                                            |
| -------------------------------------------- | ------------------- | ------------------------------------------------------------------- |
| **Dados remotos** (pertencentes ao servidor) | RTK Query           | Perfis de usuário, NFTs, itens de catálogo, pedidos                 |
| **Estado da UI** (pertencente ao cliente)    | createSlice         | Filtros, modais, preferências de visualização, estado de formulário |
| **Coleções normalizadas**                    | createEntityAdapter | Listas ordenadas/filtradas, atualizações otimistas                  |


> [!WARNING]
> **Não duplique os mesmos dados** tanto em um slice quanto no RTK Query. Escolha uma única fonte de verdade.


## Criando um Slice Básico

Use `createSlice` para estado simples da UI:

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
    
    // Definir valor específico
    modalOpened(state) {
      state.modalOpen = true;
    },
    
    modalClosed(state) {
      state.modalOpen = false;
    },
    
    // Actions com payload
    viewModeChanged(state, action: PayloadAction<'grid' | 'list'>) {
      state.viewMode = action.payload;
    },
    
    themeChanged(state, action: PayloadAction<'light' | 'dark'>) {
      state.theme = action.payload;
    },
    
    // Múltiplas propriedades
    uiReset() {
      return initialState;
    },
  },
});

// Exportar actions
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

// Exportar selectors
export const selectSidebarOpen = (state: RootState) => state.ui.sidebarOpen;
export const selectModalOpen = (state: RootState) => state.ui.modalOpen;
export const selectViewMode = (state: RootState) => state.ui.viewMode;
export const selectTheme = (state: RootState) => state.ui.theme;
```

## Usando Entity Adapters

Para coleções normalizadas (listas com IDs), use `createEntityAdapter`:

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

// Criar entity adapter
const txAdapter = createEntityAdapter<CreditTransaction>({
  selectId: (tx) => tx.id,
  sortComparer: (a, b) => b.timestamp - a.timestamp, // Mais recentes primeiro
});

// Criar slice com o estado inicial do adapter
const creditsSlice = createSlice({
  name: 'credits',
  initialState: txAdapter.getInitialState({
    sending: false,
    error: null as string | null,
  }),
  reducers: {
    // Adicionar uma transação
    txAdded: txAdapter.addOne,
    
    // Adicionar várias transações
    txsAdded: txAdapter.addMany,
    
    // Atualizar uma transação
    txUpdated: txAdapter.updateOne,
    
    // Remover uma transação
    txRemoved: txAdapter.removeOne,
    
    // Limpar todas as transações
    txsCleared: txAdapter.removeAll,
    
    // Reducer personalizado com estado extra
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

// Exportar actions
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

// Criar selectors
const selectCreditsState = (state: RootState) => state.credits;

export const creditsSelectors = txAdapter.getSelectors(selectCreditsState);

// Selectors personalizados adicionais
export const selectIsSending = (state: RootState) => state.credits.sending;
export const selectError = (state: RootState) => state.credits.error;

// Selectors memoizados
export const selectTotalCredits = (state: RootState) => {
  const txs = creditsSelectors.selectAll(state);
  return txs.reduce((total, tx) => {
    return total + (tx.type === 'grant' ? tx.amount : -tx.amount);
  }, 0);
};
```

## Métodos do Entity Adapter

### Mutations de Estado

```tsx
// Adicionar
txAdapter.addOne(state, entity)
txAdapter.addMany(state, entities)

// Atualizar
txAdapter.updateOne(state, { id, changes })
txAdapter.updateMany(state, updates)

// Upsert (adicionar ou atualizar)
txAdapter.upsertOne(state, entity)
txAdapter.upsertMany(state, entities)

// Remover
txAdapter.removeOne(state, id)
txAdapter.removeMany(state, ids)
txAdapter.removeAll(state)

// Definir (substituir tudo)
txAdapter.setAll(state, entities)
txAdapter.setOne(state, entity)
txAdapter.setMany(state, entities)
```

### Selectors Gerados

```tsx
const selectors = txAdapter.getSelectors(selectState);

// Selecionar todas as entities como array
selectors.selectAll(state)

// Selecionar entities como { [id]: entity }
selectors.selectEntities(state)

// Selecionar todos os IDs como array
selectors.selectIds(state)

// Selecionar contagem total
selectors.selectTotal(state)

// Selecionar uma única entity por ID
selectors.selectById(state, id)
```

## Exemplo de Estado Complexo

Combinando várias responsabilidades em um único slice:

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
  // Estado da visualização
  mapCenter: { x: number; y: number };
  mapZoom: number;
  
  // Estado dos filtros
  filters: LandFilter;
  
  // Estado da seleção (usando adapter)
  selectedParcels: ReturnType<typeof selectedParcelsAdapter.getInitialState>;
  
  // Estado da UI
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
    // Controles do mapa
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
    
    // Seleção
    parcelSelected(state, action: PayloadAction<SelectedParcel>) {
      selectedParcelsAdapter.addOne(state.selectedParcels, action.payload);
    },
    
    parcelDeselected(state, action: PayloadAction<string>) {
      selectedParcelsAdapter.removeOne(state.selectedParcels, action.payload);
    },
    
    selectionCleared(state) {
      selectedParcelsAdapter.removeAll(state.selectedParcels);
    },
    
    // Alternadores da UI
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

// Selectors de seleção
const selectSelectedParcelsState = (state: RootState) => state.land.selectedParcels;
export const selectedParcelsSelectors = selectedParcelsAdapter.getSelectors(
  selectSelectedParcelsState
);
```

## Selectors Memoizados

Use `createSelector` do Reselect para estado computado/derivado:

```tsx
// src/features/land/land.selectors.ts
import { createSelector } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';
import { selectFilters } from './land.slice';

// Lógica de filtragem custosa - memoizada
export const selectActiveFiltersCount = createSelector(
  [selectFilters],
  (filters) => {
    return Object.values(filters).filter(Boolean).length;
  }
);

// Combinar vários selectors
export const selectHasActiveFilters = createSelector(
  [selectActiveFiltersCount],
  (count) => count > 0
);

// Múltiplas entradas
export const selectFilteredParcels = createSelector(
  [
    (state: RootState) => state.land.allParcels, // assumindo que isso exista
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

## Lógica Assíncrona com extraReducers

Lide com respostas do RTK Query ou async thunk no seu slice:

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

## Melhores Práticas

### 1. Mantenha o estado mínimo

```tsx
// ✅ Bom: armazene apenas o que você precisa
interface State {
  userId: string | null;
  isAuthenticated: boolean;
}

// ❌ Ruim: armazenar valores derivados/computados
interface State {
  userId: string | null;
  isAuthenticated: boolean;
  hasUserId: boolean; // Pode ser computado
  userIdLength: number; // Pode ser computado
}
```

### 2. Use mutações compatíveis com Immer

```tsx
// ✅ Bom: mutação direta (Immer lida com isso)
reducers: {
  itemAdded(state, action) {
    state.items.push(action.payload);
    state.count += 1;
  }
}

// ❌ Ruim: spread manual (desnecessário)
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

### 3. Organize os Reducers logicamente

```tsx
// ✅ Bom: Agrupado por funcionalidade
reducers: {
  // Controles do modal
  modalOpened(state) { ... },
  modalClosed(state) { ... },
  
  // Controles de filtro
  filterApplied(state, action) { ... },
  filterCleared(state) { ... },
  
  // Redefinir
  stateReset() { return initialState; },
}
```

### 4. Defina corretamente os tipos das Actions

```tsx
// ✅ Bom: tipo explícito de payload
userUpdated(state, action: PayloadAction<{ id: string; name: string }>) {
  state.user = action.payload;
}

// ❌ Ruim: payload sem tipo
userUpdated(state, action) {
  state.user = action.payload; // Sem segurança de tipos
}
```

## Testando Slices

```tsx
// land.slice.test.ts
import reducer, { mapCenterChanged, mapZoomed } from './land.slice';

describe('slice de land', () => {
  const initialState = {
    mapCenter: { x: 0, y: 0 },
    mapZoom: 1,
    // ... outro estado
  };

  it('deve lidar com mapCenterChanged', () => {
    const newCenter = { x: 10, y: 20 };
    const actual = reducer(initialState, mapCenterChanged(newCenter));
    expect(actual.mapCenter).toEqual(newCenter);
  });

  it('deve lidar com mapZoomed', () => {
    const actual = reducer(initialState, mapZoomed(2));
    expect(actual.mapZoom).toBe(2);
  });
});
```

## Próximos Passos

* Revisar [Padrões de Componentes](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/component-patterns/) para usar slices em componentes
* Saiba mais sobre [Integração com Web3](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/web3-integration/) para o estado na blockchain
* Veja [Testes e Performance](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/testing-and-performance/) para dicas de otimização


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/state-management.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
