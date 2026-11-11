# Padrões de Components

Esta página aborda como utilizar eficazmente o Redux e o RTK Query em componentes React, incluindo hooks, padrões de otimização e melhores práticas.

## Utilização básica de consultas

Use hooks gerados dos endpoints do RTK Query:

```tsx
import { useGetParcelByCoordsQuery } from '@/features/land/land.client';

function ParcelCard({ x, y }: { x: number; y: number }) {
  const { data, isLoading, isError, error } = useGetParcelByCoordsQuery({ x, y });

  if (isLoading) return <div>Carregando...</div>;
  if (isError) return <div>Erro: {error.toString()}</div>;
  if (!data) return null;

  return (
    <div>
      <h2>{data.name || `Parcela ${x},${y}`}</h2>
      <p>Proprietário: {data.owner}</p>
    </div>
  );
}
```

## Otimizar re-renders com `selectFromResult`

Restrinja o resultado apenas aos campos de que precisa:

```tsx
function ParcelOwner({ x, y }: { x: number; y: number }) {
  // ✅ Bom: subscreve apenas às alterações do campo owner
  const { owner, isFetching } = useGetParcelByCoordsQuery(
    { x, y },
    {
      selectFromResult: ({ data, isFetching }) => ({
        owner: data?.owner,
        isFetching,
      }),
    }
  );

  if (isFetching) return <span>A carregar...</span>;
  return <span>Proprietário: {owner}</span>;
}

// ❌ Mau: re-renderiza em qualquer alteração dos dados
function ParcelOwnerBad({ x, y }: { x: number; y: number }) {
  const { data } = useGetParcelByCoordsQuery({ x, y });
  return <span>Proprietário: {data?.owner}</span>;
}
```

## Consultas condicionais com `skip`

Ignore consultas quando os argumentos não estiverem prontos:

```tsx
function UserProfile() {
  const { data: session } = useGetSessionQuery();
  const userId = session?.userId;

  // Ignore a consulta de perfil até termos userId
  const { data: profile } = useGetProfileQuery(
    { id: userId! },
    {
      skip: !userId, // Não faça fetch se userId estiver indefinido
    }
  );

  if (!userId) return <div>Inicie sessão, por favor</div>;
  if (!profile) return <div>A carregar perfil...</div>;

  return <div>{profile.name}</div>;
}
```

## Polling para atualizações em tempo real

```tsx
function LiveBalance({ address }: { address: string }) {
  const { data } = useGetBalanceQuery(
    { address },
    {
      pollingInterval: 10000, // Faz polling a cada 10 segundos
      skipPollingIfUnfocused: true, // Pausa quando a aba não está em foco
    }
  );

  return <div>Saldo: {data?.amount ?? 0}</div>;
}
```

## Consultas lazy

Dispare consultas manualmente em vez de na montagem do componente:

```tsx
function SearchParcels() {
  const [trigger, result] = useLazyGetParcelsByOwnerQuery();
  const [owner, setOwner] = useState('');

  const handleSearch = () => {
    if (owner) {
      trigger({ owner });
    }
  };

  return (
    <div>
      <input
        value={owner}
        onChange={(e) => setOwner(e.target.value)}
        placeholder="Introduza o endereço do proprietário"
      />
      <button onClick={handleSearch}>Pesquisar</button>
      
      {result.isLoading && <div>A pesquisar...</div>}
      {result.data && (
        <ul>
          {result.data.map((parcel) => (
            <li key={parcel.id}>{parcel.name}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

## Utilização de mutações

### Mutação básica

```tsx
function GrantCreditsButton({ address }: { address: string }) {
  const [grantCredits, { isLoading, isError, error }] = useGrantCreditsMutation();

  const handleGrant = async () => {
    try {
      await grantCredits({ address, amount: 100 }).unwrap();
      toast.success('Créditos concedidos com sucesso!');
    } catch (err) {
      toast.error('Falha ao conceder créditos');
      console.error('Falha na concessão:', err);
    }
  };

  return (
    <button onClick={handleGrant} disabled={isLoading}>
      {isLoading ? 'A conceder...' : 'Conceder 100 créditos'}
    </button>
  );
}
```

### Mutação com feedback

```tsx
import { isFetchBaseQueryError } from '@/services/client';

function UpdateParcelName({ parcelId }: { parcelId: string }) {
  const [name, setName] = useState('');
  const [updateName, { isLoading, isSuccess, isError, error }] =
    useUpdateParcelNameMutation();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    try {
      const result = await updateName({ id: parcelId, name }).unwrap();
      toast.success(`Parcela renomeada para "${result.name}"`);
      setName(''); // Limpar formulário
    } catch (err) {
      if (isFetchBaseQueryError(err)) {
        toast.error(`Erro: ${JSON.stringify(err.data)}`);
      } else {
        toast.error('Ocorreu um erro inesperado');
      }
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Novo nome"
        disabled={isLoading}
      />
      <button type="submit" disabled={isLoading || !name}>
        {isLoading ? 'A atualizar...' : 'Atualizar nome'}
      </button>
      
      {isSuccess && <p className="success">Nome atualizado!</p>}
      {isError && <p className="error">Falha na atualização</p>}
    </form>
  );
}
```

## Utilização do estado do slice

Aceda ao estado do slice com `useAppSelector`:

```tsx
import { useAppSelector, useAppDispatch } from '@/app/hooks';
import { selectViewMode, viewModeChanged } from '@/features/ui/ui.slice';

function ViewModeToggle() {
  const dispatch = useAppDispatch();
  const viewMode = useAppSelector(selectViewMode);

  const toggleMode = () => {
    dispatch(viewModeChanged(viewMode === 'grid' ? 'list' : 'grid'));
  };

  return (
    <button onClick={toggleMode}>
      Vista: {viewMode === 'grid' ? '⊞ Grelha' : '☰ Lista'}
    </button>
  );
}
```

## Combinar várias consultas

```tsx
function ParcelDetails({ id }: { id: string }) {
  const { data: parcel, isLoading: parcelLoading } = useGetParcelQuery({ id });
  const { data: owner, isLoading: ownerLoading } = useGetProfileQuery(
    { id: parcel?.owner! },
    { skip: !parcel?.owner }
  );

  const isLoading = parcelLoading || ownerLoading;

  if (isLoading) return <div>Carregando...</div>;
  if (!parcel) return <div>Parcela não encontrada</div>;

  return (
    <div>
      <h1>{parcel.name}</h1>
      <p>Coordenadas: {parcel.x}, {parcel.y}</p>
      {owner && <p>Proprietário: {owner.name}</p>}
    </div>
  );
}
```

## Estado derivado com seletores

```tsx
import { useAppSelector } from '@/app/hooks';
import { selectFilteredParcels, selectActiveFiltersCount } from '@/features/land/land.selectors';

function ParcelList() {
  const parcels = useAppSelector(selectFilteredParcels);
  const filterCount = useAppSelector(selectActiveFiltersCount);

  return (
    <div>
      <h2>
        Parcelas ({parcels.length})
        {filterCount > 0 && ` - ${filterCount} filtro(s) ativo(s)`}
      </h2>
      <ul>
        {parcels.map((parcel) => (
          <li key={parcel.id}>{parcel.name}</li>
        ))}
      </ul>
    </div>
  );
}
```

## Despachar ações

```tsx
import { useAppDispatch } from '@/app/hooks';
import { modalOpened, modalClosed } from '@/features/ui/ui.slice';

function TransferButton({ parcelId }: { parcelId: string }) {
  const dispatch = useAppDispatch();

  const handleClick = () => {
    // Dispare a ação para abrir o modal
    dispatch(modalOpened());
  };

  return <button onClick={handleClick}>Transferir parcela</button>;
}
```

## Seletores do Entity Adapter

```tsx
import { useAppSelector } from '@/app/hooks';
import { creditsSelectors, selectTotalCredits } from '@/features/credits/credits.slice';

function CreditsSummary() {
  // Obter todas as transações
  const allTxs = useAppSelector(creditsSelectors.selectAll);
  
  // Obter uma transação específica
  const txId = 'tx-123';
  const tx = useAppSelector((state) => 
    creditsSelectors.selectById(state, txId)
  );
  
  // Obter a contagem total
  const count = useAppSelector(creditsSelectors.selectTotal);
  
  // Obter o valor derivado
  const total = useAppSelector(selectTotalCredits);

  return (
    <div>
      <p>Total de créditos: {total}</p>
      <p>Transações: {count}</p>
      <ul>
        {allTxs.map((tx) => (
          <li key={tx.id}>
            {tx.type === 'grant' ? '+' : '-'}
            {tx.amount} - {tx.description}
          </li>
        ))}
      </ul>
    </div>
  );
}
```

## Pré-carregamento de dados

Faça pré-carregamento dos dados antes da navegação para uma UX mais rápida:

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';
import { Link } from 'react-router-dom';

function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // Faça pré-carregamento dos detalhes da parcela ao passar o rato
    dispatch(
      client.util.prefetch('getParcel', { id: parcel.id }, { force: false })
    );
  };

  return (
    <Link
      to={`/parcels/${parcel.id}`}
      onMouseEnter={handleMouseEnter}
    >
      {parcel.name}
    </Link>
  );
}
```

## Gestão manual da cache

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';

function RefreshButton() {
  const dispatch = useAppDispatch();

  const handleRefresh = () => {
    // Invalide todas as consultas de Parcels
    dispatch(client.util.invalidateTags(['Parcels']));
    
    // Ou redefina todo o estado do client
    // dispatch(client.util.resetApiState());
  };

  return <button onClick={handleRefresh}>Atualizar dados</button>;
}
```

## Lidar com estados da consulta

```tsx
function ComprehensiveExample({ id }: { id: string }) {
  const {
    data,
    isLoading,      // Carregamento inicial
    isFetching,     // Qualquer fetch (incluindo refetch)
    isSuccess,      // Consulta bem-sucedida
    isError,        // Consulta falhou
    error,          // Objeto de erro
    refetch,        // Função de refetch manual
  } = useGetParcelQuery({ id });

  // Estados diferentes
  if (isLoading) {
    return <Spinner />;
  }

  if (isError) {
    return (
      <div>
        <p>Erro: {error.toString()}</p>
        <button onClick={() => refetch()}>Tentar novamente</button>
      </div>
    );
  }

  if (!data) {
    return <div>Sem dados</div>;
  }

  return (
    <div>
      {isFetching && <div className="refetch-indicator">A atualizar...</div>}
      <h1>{data.name}</h1>
      <button onClick={() => refetch()}>Atualizar</button>
    </div>
  );
}
```

## Integração com formulários

```tsx
import { useState } from 'react';
import { useAppDispatch } from '@/app/hooks';
import { filtersUpdated } from '@/features/land/land.slice';

function FilterForm() {
  const dispatch = useAppDispatch();
  const [owner, setOwner] = useState('');
  const [minPrice, setMinPrice] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    
    dispatch(filtersUpdated({
      owner: owner || undefined,
      minPrice: minPrice ? parseInt(minPrice, 10) : undefined,
    }));
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="text"
        value={owner}
        onChange={(e) => setOwner(e.target.value)}
        placeholder="Endereço do proprietário"
      />
      <input
        type="number"
        value={minPrice}
        onChange={(e) => setMinPrice(e.target.value)}
        placeholder="Preço mínimo"
      />
      <button type="submit">Aplicar filtros</button>
    </form>
  );
}
```

## Melhores práticas

### 1. Use hooks tipados

```tsx
// ✅ Bom: usa hooks tipados
import { useAppSelector, useAppDispatch } from '@/app/hooks';

const value = useAppSelector(selectValue);
const dispatch = useAppDispatch();

// ❌ Mau: usa hooks sem tipos
import { useSelector, useDispatch } from 'react-redux';

const value = useSelector((state: RootState) => state.value);
const dispatch = useDispatch();
```

### 2. Lide com todos os estados da consulta

```tsx
// ✅ Bom: gestão abrangente do estado
const { data, isLoading, isError, error } = useQuery(args);
if (isLoading) return <Loading />;
if (isError) return <Error error={error} />;
if (!data) return null;
return <Content data={data} />;

// ❌ Mau: falta de tratamento de erros
const { data } = useQuery(args);
return <Content data={data} />; // Pode falhar
```

### 3. Use `.unwrap()` para mutações

```tsx
// ✅ Bom: tratamento explícito de erros
try {
  const result = await mutation(args).unwrap();
  toast.success('Sucesso!');
} catch (error) {
  toast.error('Falhou!');
}

// ❌ Mau: sem tratamento de erros
mutation(args);
```

### 4. Restrinja os resultados com `selectFromResult`

```tsx
// ✅ Bom: subscreva apenas aos campos necessários
const { name } = useQuery(args, {
  selectFromResult: ({ data }) => ({ name: data?.name }),
});

// ❌ Mau: subscreva ao objeto inteiro
const { data } = useQuery(args);
const name = data?.name;
```

### 5. Evite selecionar slices inteiros

```tsx
// ✅ Bom: selecione valores específicos
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ Mau: selecione o slice inteiro
const ui = useAppSelector((state) => state.ui);
const viewMode = ui.viewMode;
```

## Próximos Passos

* Saiba mais sobre [Integração com Web3](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/web3-integration/) para padrões de blockchain
* Revisar [Testes e Performance](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/testing-and-performance/) para otimização
* Veja [RTK Query](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/rtk-query/) para configuração avançada de endpoints


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/component-patterns.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
