# Pruebas y rendimiento

Esta página cubre estrategias de pruebas para aplicaciones de Redux/RTK Query y técnicas de optimización del rendimiento.

## Pruebas de Redux Slices

### Pruebas básicas de Reducer

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

### Pruebas de Entity Adapters

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

## Pruebas de Selectors

### Selectors simples

```tsx
// user.selectors.test.ts
import { selectAccount, selectIsAuthenticated } from './user.slice';

describe('user selectors', () => {
  const mockState = {
    user: {
      account: '0x123...',
      isAuthenticated: true,
    },
    // ... otras slices
  };

  it('should select account', () => {
    expect(selectAccount(mockState)).toBe('0x123...');
  });

  it('should select authentication status', () => {
    expect(selectIsAuthenticated(mockState)).toBe(true);
  });
});
```

### Selectores memorizados

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
    
    // Misma referencia = memoizado
    expect(result1).toBe(result2);
  });
});
```

## Pruebas de RTK Query con MSW

### Configurar MSW

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

### Pruebas de Queries

```tsx
// land.client.test.tsx
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { setupStore } from '@/app/store';
import { useGetParcelQuery } from './land.client';

function TestComponent({ id }: { id: string }) {
  const { data, isLoading, isError } = useGetParcelQuery({ id });

  if (isLoading) return <div>Cargando...</div>;
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

    expect(screen.getByText('Cargando...')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('Test Parcel')).toBeInTheDocument();
    });
  });
});
```

### Pruebas de Mutations

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
        Otorgar
      </button>
      {isLoading && <div>Cargando...</div>}
      {isSuccess && <div>¡Éxito!</div>}
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

    fireEvent.click(screen.getByText('Otorgar'));

    await waitFor(() => {
      expect(screen.getByText('¡Éxito!')).toBeInTheDocument();
    });
  });
});
```

### Pruebas de Actualizaciones Optimistas

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

    // Precargar saldo inicial
    await store.dispatch(
      creditsClient.endpoints.getBalance.initiate({ address })
    );

    const initialBalance = creditsClient.endpoints.getBalance.select({ address })(
      store.getState()
    ).data?.amount;

    expect(initialBalance).toBe(100); // del handler de MSW

    // Simular fallo
    server.use(
      http.post('/api/v1/credits/grant', () => {
        return HttpResponse.json({ error: 'Failed' }, { status: 500 });
      })
    );

    // Disparar mutation
    const mutation = store.dispatch(
      creditsClient.endpoints.grantCredits.initiate({
        address,
        amount: 50,
      })
    );

    // Comprobar actualización optimista
    const optimisticBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(optimisticBalance).toBe(150); // 100 + 50

    // Esperar a que falle la mutation
    await expect(mutation).rejects.toThrow();

    // Comprobar rollback
    const rolledBackBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(rolledBackBalance).toBe(100); // Vuelve al original
  });
});
```

## Optimización del Rendimiento

### Usa `selectFromResult` para evitar re-renders

```tsx
// ✅ Bien: Solo se suscribe a campos específicos
const { owner } = useGetParcelQuery(
  { id },
  {
    selectFromResult: ({ data }) => ({
      owner: data?.owner,
    }),
  }
);

// El componente solo se vuelve a renderizar cuando cambia owner
```

### Evita seleccionar todo el estado

```tsx
// ✅ Bien: Selecciona valores específicos
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ Mal: Selecciona toda la slice
const ui = useAppSelector((state) => state.ui);
```

### Memoiza Selectors costosos

```tsx
import { createSelector } from '@reduxjs/toolkit';

// ✅ Bien: Selector memorizado
export const selectFilteredParcels = createSelector(
  [selectAllParcels, selectFilters],
  (parcels, filters) => {
    // Lógica costosa de filtrado
    return parcels.filter(/* ... */);
  }
);

// ❌ Mal: Calculado en el componente
function Component() {
  const parcels = useAppSelector(selectAllParcels);
  const filters = useAppSelector(selectFilters);
  
  // Se recalcula en cada renderizado!
  const filtered = parcels.filter(/* ... */);
}
```

### Usa Entity Adapters para datos normalizados

```tsx
// ✅ Bien: Normalizado con entity adapter
const adapter = createEntityAdapter<Parcel>();

// Búsquedas eficientes por ID
const parcel = adapter.getSelectors().selectById(state, id);

// ❌ Mal: Búsqueda en array
const parcel = state.parcels.find((p) => p.id === id);
```

### Ajusta la configuración de caché de RTK Query

```tsx
export const client = createApi({
  // ...
  keepUnusedDataFor: 60, // Mantener los datos durante 60 segundos
  refetchOnMountOrArgChange: 30, // Volver a obtener si los datos tienen más de 30 s
  refetchOnFocus: true, // Volver a obtener cuando la ventana recupere el foco
  refetchOnReconnect: true, // Volver a obtener al reconectar
});
```

### Precarga para una mejor UX

```tsx
function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // Precargar al pasar el cursor
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

### Estrategia de polling

```tsx
// Hacer polling solo cuando sea necesario
const { data } = useGetBalanceQuery(
  { address },
  {
    pollingInterval: isActive ? 10000 : 0, // Hacer polling solo cuando esté activo
    skipPollingIfUnfocused: true, // Pausar cuando la pestaña no tenga foco
  }
);
```

## Redux DevTools

### Habilitar en desarrollo

```tsx
export const store = configureStore({
  // ...
  devTools: process.env.NODE_ENV !== 'production',
});
```

### Sanitizador de Actions

Sanitiza datos sensibles en DevTools:

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

## Lista de verificación de buenas prácticas

### Rendimiento

* [ ] Usa `selectFromResult` para grandes resultados de consultas
* [ ] Memoiza Selectors costosos con `createSelector`
* [ ] Usa entity adapters para colecciones normalizadas
* [ ] Evita seleccionar slices completas en componentes
* [ ] Ajusta `keepUnusedDataFor` según tu caso de uso
* [ ] Precarga datos antes de la navegación
* [ ] Usa polling estratégicamente (solo cuando sea necesario)

### Pruebas

* [ ] Prueba unitariamente todos los reducers y actions
* [ ] Prueba los selectors memorizados para comprobar corrección y rendimiento
* [ ] Usa MSW para pruebas de endpoints de RTK Query
* [ ] Prueba las actualizaciones optimistas y la lógica de rollback
* [ ] Prueba el manejo de errores en los componentes
* [ ] Escribe pruebas de integración para flujos críticos

### Calidad del código

* [ ] Usa hooks tipados (`useAppSelector`, `useAppDispatch`)
* [ ] Maneja todos los estados de la query (cargando, error, éxito)
* [ ] Usa `.unwrap()` para el manejo de errores de la mutation
* [ ] Invalida o actualiza la caché después de las mutations
* [ ] Mantén los datos no serializables fuera de Redux
* [ ] Documenta los selectors y la lógica compleja

## Patrones antihigiénicos a evitar


> [!CAUTION]
> **No hagas esto:**
> 
> 1. Guarda objetos no serializables (providers, signers) en Redux
> 2. Duplica datos tanto en slices como en RTK Query
> 3. Dispara actions durante el renderizado
> 4. Crea selectors que devuelvan nuevos objetos sin memoización
> 5. Ignora los estados de carga y error
> 6. Obtén los mismos datos en varios componentes sin RTK Query
> 7. Haz polling en exceso o haz polling sin `skipPollingIfUnfocused`


## Supervisión del rendimiento

### Seguimiento de llamadas a Selectors

```tsx
import { createSelector } from '@reduxjs/toolkit';

const selectExpensiveData = createSelector(
  [selectData],
  (data) => {
    console.log('Selector called'); // Solo debería registrarse cuando los datos cambian
    return expensiveOperation(data);
  }
);
```

### Supervisar re-renders

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
  useRenderCount(); // Seguimiento de re-renders
  // ...
}
```

## Próximos pasos

* Revisa [Patrones de Componentes](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/component-patterns/) para ejemplos de uso
* Consulta [RTK Query](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/rtk-query/) para estrategias de caché
* Entiende [la gestión del estado](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/state-management/) para la optimización de slices


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/testing-and-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
