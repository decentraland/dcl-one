# Testes e desempenho

Esta página abrange estratégias de teste para aplicações Redux/RTK Query e técnicas de otimização de desempenho.

## Testando Redux Slices

### Testes básicos de reducer

```tsx
// user.slice.test.ts
import reducer, { userLoggedIn, userLoggedOut } from './user.slice';

describe('user slice', () => {
  const initialState = {
    account: null,
    isAuthenticated: false,
  };

  it('deve lidar com userLoggedIn', () => {
    const account = '0x123...';
    const actual = reducer(initialState, userLoggedIn({ account }));
    
    expect(actual.account).toBe(account);
    expect(actual.isAuthenticated).toBe(true);
  });

  it('deve lidar com userLoggedOut', () => {
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

### Testando Entity Adapters

```tsx
// credits.slice.test.ts
import reducer, { txAdded, txsCleared, creditsSelectors } from './credits.slice';

describe('credits slice com entity adapter', () => {
  it('deve adicionar uma transação', () => {
    const initialState = reducer(undefined, { type: 'unknown' });
    const tx = { id: '1', address: '0x123', amount: 100, type: 'grant', timestamp: Date.now() };
    
    const actual = reducer(initialState, txAdded(tx));
    
    expect(creditsSelectors.selectById({ credits: actual }, '1')).toEqual(tx);
    expect(creditsSelectors.selectTotal({ credits: actual })).toBe(1);
  });

  it('deve limpar todas as transações', () => {
    const initialState = reducer(undefined, { type: 'unknown' });
    const withTx = reducer(initialState, txAdded({ id: '1', /* ... */ }));
    
    const actual = reducer(withTx, txsCleared());
    
    expect(creditsSelectors.selectTotal({ credits: actual })).toBe(0);
  });
});
```

## Testando Selectors

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
    // ... outras slices
  };

  it('deve selecionar a conta', () => {
    expect(selectAccount(mockState)).toBe('0x123...');
  });

  it('deve selecionar o status de autenticação', () => {
    expect(selectIsAuthenticated(mockState)).toBe(true);
  });
});
```

### Selectors Memoizados

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

  it('deve contar os filtros ativos', () => {
    expect(selectActiveFiltersCount(mockState)).toBe(2);
  });

  it('deve filtrar parcels', () => {
    const result = selectFilteredParcels(mockState);
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('1');
  });

  it('deve memorizar os resultados', () => {
    const result1 = selectFilteredParcels(mockState);
    const result2 = selectFilteredParcels(mockState);
    
    // Mesma referência = memoizado
    expect(result1).toBe(result2);
  });
});
```

## Testando RTK Query com MSW

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
      name: 'Parcela de teste',
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

### Testando Queries

```tsx
// land.client.test.tsx
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { setupStore } from '@/app/store';
import { useGetParcelQuery } from './land.client';

function TestComponent({ id }: { id: string }) {
  const { data, isLoading, isError } = useGetParcelQuery({ id });

  if (isLoading) return <div>Carregando...</div>;
  if (isError) return <div>Erro</div>;
  if (!data) return null;

  return <div>{data.name}</div>;
}

describe('land client', () => {
  it('deve buscar e exibir os dados da parcela', async () => {
    const store = setupStore();

    render(
      <Provider store={store}>
        <TestComponent id="1" />
      </Provider>
    );

    expect(screen.getByText('Carregando...')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('Parcela de teste')).toBeInTheDocument();
    });
  });
});
```

### Testando Mutations

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
        Conceder
      </button>
      {isLoading && <div>Carregando...</div>}
      {isSuccess && <div>Sucesso!</div>}
    </div>
  );
}

describe('credits client mutations', () => {
  it('deve conceder créditos com sucesso', async () => {
    const store = setupStore();

    render(
      <Provider store={store}>
        <TestComponent />
      </Provider>
    );

    fireEvent.click(screen.getByText('Conceder'));

    await waitFor(() => {
      expect(screen.getByText('Sucesso!')).toBeInTheDocument();
    });
  });
});
```

### Testando atualizações otimistas

```tsx
// credits.client.test.ts
import { server } from '@/test/server';
import { http, HttpResponse } from 'msw';
import { setupStore } from '@/app/store';
import { creditsClient } from './credits.client';

describe('optimistic updates', () => {
  it('deve atualizar o cache de forma otimista e reverter em caso de erro', async () => {
    const store = setupStore();
    const address = '0x123';

    // Pré-carregar saldo inicial
    await store.dispatch(
      creditsClient.endpoints.getBalance.initiate({ address })
    );

    const initialBalance = creditsClient.endpoints.getBalance.select({ address })(
      store.getState()
    ).data?.amount;

    expect(initialBalance).toBe(100); // do handler do MSW

    // Simular falha
    server.use(
      http.post('/api/v1/credits/grant', () => {
        return HttpResponse.json({ error: 'Falha' }, { status: 500 });
      })
    );

    // Disparar mutation
    const mutation = store.dispatch(
      creditsClient.endpoints.grantCredits.initiate({
        address,
        amount: 50,
      })
    );

    // Verificar atualização otimista
    const optimisticBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(optimisticBalance).toBe(150); // 100 + 50

    // Aguardar a mutation falhar
    await expect(mutation).rejects.toThrow();

    // Verificar rollback
    const rolledBackBalance = creditsClient.endpoints.getBalance.select({
      address,
    })(store.getState()).data?.amount;

    expect(rolledBackBalance).toBe(100); // De volta ao original
  });
});
```

## Otimização de desempenho

### Use `selectFromResult` para evitar re-renders

```tsx
// ✅ Bom: subscreve apenas campos específicos
const { owner } = useGetParcelQuery(
  { id },
  {
    selectFromResult: ({ data }) => ({
      owner: data?.owner,
    }),
  }
);

// O componente só faz re-render quando o owner muda
```

### Evite selecionar o estado inteiro

```tsx
// ✅ Bom: selecione valores específicos
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ Ruim: seleciona a slice inteira
const ui = useAppSelector((state) => state.ui);
```

### Memoize Selectors caros

```tsx
import { createSelector } from '@reduxjs/toolkit';

// ✅ Bom: selector memoizado
export const selectFilteredParcels = createSelector(
  [selectAllParcels, selectFilters],
  (parcels, filters) => {
    // Lógica de filtragem cara
    return parcels.filter(/* ... */);
  }
);

// ❌ Ruim: calculado no componente
function Component() {
  const parcels = useAppSelector(selectAllParcels);
  const filters = useAppSelector(selectFilters);
  
  // Recalcula em cada render!
  const filtered = parcels.filter(/* ... */);
}
```

### Use Entity Adapters para dados normalizados

```tsx
// ✅ Bom: normalizado com entity adapter
const adapter = createEntityAdapter<Parcel>();

// Consultas eficientes por ID
const parcel = adapter.getSelectors().selectById(state, id);

// ❌ Ruim: busca em array
const parcel = state.parcels.find((p) => p.id === id);
```

### Ajuste as configurações de cache do RTK Query

```tsx
export const client = createApi({
  // ...
  keepUnusedDataFor: 60, // Manter os dados por 60 segundos
  refetchOnMountOrArgChange: 30, // Buscar novamente se os dados tiverem mais de 30s
  refetchOnFocus: true, // Buscar novamente quando a janela recuperar o foco
  refetchOnReconnect: true, // Buscar novamente ao reconectar
});
```

### Pré-carregamento para melhor UX

```tsx
function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // Pré-carregar no hover
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

### Estratégia de polling

```tsx
// Fazer polling apenas quando necessário
const { data } = useGetBalanceQuery(
  { address },
  {
    pollingInterval: isActive ? 10000 : 0, // Fazer polling apenas quando ativo
    skipPollingIfUnfocused: true, // Pausar quando a aba não estiver em foco
  }
);
```

## Redux DevTools

### Ativar no desenvolvimento

```tsx
export const store = configureStore({
  // ...
  devTools: process.env.NODE_ENV !== 'production',
});
```

### Sanitizador de ações

Sanitize dados sensíveis no DevTools:

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

## Lista de verificação de boas práticas

### Desempenho

* [ ] Use `selectFromResult` para grandes resultados de query
* [ ] Memoize selectors caros com `createSelector`
* [ ] Use Entity Adapters para coleções normalizadas
* [ ] Evite selecionar slices inteiras nos componentes
* [ ] Ajuste `keepUnusedDataFor` de acordo com o seu caso de uso
* [ ] Pré-carregue dados antes da navegação
* [ ] Use polling estrategicamente (apenas quando necessário)

### Testes

* [ ] Teste unitariamente todos os reducers e actions
* [ ] Teste selectors memoizados quanto à correção e ao desempenho
* [ ] Use MSW para testes de endpoints do RTK Query
* [ ] Teste atualizações otimistas e a lógica de rollback
* [ ] Teste o tratamento de erros nos componentes
* [ ] Escreva testes de integração para fluxos críticos

### Qualidade do código

* [ ] Use hooks tipados (`useAppSelector`, `useAppDispatch`)
* [ ] Lide com todos os estados da query (loading, error, success)
* [ ] Use `.unwrap()` para tratamento de erros de mutation
* [ ] Invalide ou atualize o cache após mutations
* [ ] Mantenha dados não serializáveis fora do Redux
* [ ] Documente selectors e lógica complexos

## Anti-padrões a evitar


> [!CAUTION]
> **Não faça o seguinte:**
> 
> 1. Armazene objetos não serializáveis (providers, signers) no Redux
> 2. Duplique dados tanto em slices quanto no RTK Query
> 3. Dispare actions durante o render
> 4. Crie selectors que retornem novos objetos sem memoização
> 5. Ignore os estados loading e error
> 6. Busque os mesmos dados em múltiplos componentes sem RTK Query
> 7. Faça polling em excesso ou faça polling sem `skipPollingIfUnfocused`


## Monitorando o desempenho

### Acompanhe chamadas de selector

```tsx
import { createSelector } from '@reduxjs/toolkit';

const selectExpensiveData = createSelector(
  [selectData],
  (data) => {
    console.log('Selector chamado'); // Deve registrar apenas quando os dados mudarem
    return expensiveOperation(data);
  }
);
```

### Monitore re-renders

```tsx
import { useEffect, useRef } from 'react';

function useRenderCount() {
  const renderCount = useRef(0);
  
  useEffect(() => {
    renderCount.current += 1;
    console.log('Contagem de renderizações:', renderCount.current);
  });
}

function Component() {
  useRenderCount(); // Acompanhar re-renders
  // ...
}
```

## Próximos Passos

* Revisar [Padrões de Componentes](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/component-patterns/) para exemplos de uso
* Veja [RTK Query](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/rtk-query/) para estratégias de cache
* Entenda [Gerenciamento de estado](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/state-management/) para otimização de slice


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/testing-and-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
