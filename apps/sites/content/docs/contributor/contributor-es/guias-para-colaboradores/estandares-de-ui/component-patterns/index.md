# Patrones de componentes

Esta página cubre cómo usar eficazmente Redux y RTK Query en componentes de React, incluyendo hooks, patrones de optimización y buenas prácticas.

## Uso básico de Query

Usa los hooks generados por los endpoints de RTK Query:

```tsx
import { useGetParcelByCoordsQuery } from '@/features/land/land.client';

function ParcelCard({ x, y }: { x: number; y: number }) {
  const { data, isLoading, isError, error } = useGetParcelByCoordsQuery({ x, y });

  if (isLoading) return <div>Cargando...</div>;
  if (isError) return <div>Error: {error.toString()}</div>;
  if (!data) return null;

  return (
    <div>
      <h2>{data.name || `Parcel ${x},${y}`}</h2>
      <p>Propietario: {data.owner}</p>
    </div>
  );
}
```

## Optimizando re-renderizados con `selectFromResult`

Reduce el resultado solo a los campos que necesitas:

```tsx
function ParcelOwner({ x, y }: { x: number; y: number }) {
  // ✅ Bien: Solo se suscribe a los cambios del campo owner
  const { owner, isFetching } = useGetParcelByCoordsQuery(
    { x, y },
    {
      selectFromResult: ({ data, isFetching }) => ({
        owner: data?.owner,
        isFetching,
      }),
    }
  );

  if (isFetching) return <span>Cargando...</span>;
  return <span>Propietario: {owner}</span>;
}

// ❌ Mal: Re-renderiza ante cualquier cambio de datos
function ParcelOwnerBad({ x, y }: { x: number; y: number }) {
  const { data } = useGetParcelByCoordsQuery({ x, y });
  return <span>Propietario: {data?.owner}</span>;
}
```

## Consultas condicionales con `skip`

Omite consultas cuando los argumentos no estén listos:

```tsx
function UserProfile() {
  const { data: session } = useGetSessionQuery();
  const userId = session?.userId;

  // Omitir la consulta del perfil hasta que tengamos userId
  const { data: profile } = useGetProfileQuery(
    { id: userId! },
    {
      skip: !userId, // No obtener datos si userId es undefined
    }
  );

  if (!userId) return <div>Por favor inicia sesión</div>;
  if (!profile) return <div>Cargando perfil...</div>;

  return <div>{profile.name}</div>;
}
```

## Polling para actualizaciones en tiempo real

```tsx
function LiveBalance({ address }: { address: string }) {
  const { data } = useGetBalanceQuery(
    { address },
    {
      pollingInterval: 10000, // Consultar cada 10 segundos
      skipPollingIfUnfocused: true, // Pausar cuando la pestaña no está enfocada
    }
  );

  return <div>Balance: {data?.amount ?? 0}</div>;
}
```

## Consultas perezosas

Activa las consultas manualmente en lugar de hacerlo al montar el componente:

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
        placeholder="Introduce la dirección del propietario"
      />
      <button onClick={handleSearch}>Buscar</button>
      
      {result.isLoading && <div>Buscando...</div>}
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

## Uso de Mutation

### Mutation básica

```tsx
function GrantCreditsButton({ address }: { address: string }) {
  const [grantCredits, { isLoading, isError, error }] = useGrantCreditsMutation();

  const handleGrant = async () => {
    try {
      await grantCredits({ address, amount: 100 }).unwrap();
      toast.success('¡Créditos otorgados correctamente!');
    } catch (err) {
      toast.error('No se pudieron otorgar los créditos');
      console.error('Falló el otorgamiento:', err);
    }
  };

  return (
    <button onClick={handleGrant} disabled={isLoading}>
      {isLoading ? 'Otorgando...' : 'Otorgar 100 créditos'}
    </button>
  );
}
```

### Mutation con retroalimentación

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
      toast.success(`Parcel renombrado a "${result.name}"`);
      setName(''); // Limpiar formulario
    } catch (err) {
      if (isFetchBaseQueryError(err)) {
        toast.error(`Error: ${JSON.stringify(err.data)}`);
      } else {
        toast.error('Ocurrió un error inesperado');
      }
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Nuevo nombre"
        disabled={isLoading}
      />
      <button type="submit" disabled={isLoading || !name}>
        {isLoading ? 'Actualizando...' : 'Actualizar nombre'}
      </button>
      
      {isSuccess && <p className="success">¡Nombre actualizado!</p>}
      {isError && <p className="error">La actualización falló</p>}
    </form>
  );
}
```

## Uso del estado del slice

Accede al estado del slice con `useAppSelector`:

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
      Vista: {viewMode === 'grid' ? '⊞ Grid' : '☰ List'}
    </button>
  );
}
```

## Combinando múltiples consultas

```tsx
function ParcelDetails({ id }: { id: string }) {
  const { data: parcel, isLoading: parcelLoading } = useGetParcelQuery({ id });
  const { data: owner, isLoading: ownerLoading } = useGetProfileQuery(
    { id: parcel?.owner! },
    { skip: !parcel?.owner }
  );

  const isLoading = parcelLoading || ownerLoading;

  if (isLoading) return <div>Cargando...</div>;
  if (!parcel) return <div>Parcel no encontrada</div>;

  return (
    <div>
      <h1>{parcel.name}</h1>
      <p>Coordenadas: {parcel.x}, {parcel.y}</p>
      {owner && <p>Propietario: {owner.name}</p>}
    </div>
  );
}
```

## Estado derivado con selectores

```tsx
import { useAppSelector } from '@/app/hooks';
import { selectFilteredParcels, selectActiveFiltersCount } from '@/features/land/land.selectors';

function ParcelList() {
  const parcels = useAppSelector(selectFilteredParcels);
  const filterCount = useAppSelector(selectActiveFiltersCount);

  return (
    <div>
      <h2>
        Parcels ({parcels.length})
        {filterCount > 0 && ` - ${filterCount} filtro(s) activo(s)`}
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

## Despachando acciones

```tsx
import { useAppDispatch } from '@/app/hooks';
import { modalOpened, modalClosed } from '@/features/ui/ui.slice';

function TransferButton({ parcelId }: { parcelId: string }) {
  const dispatch = useAppDispatch();

  const handleClick = () => {
    // Despachar acción para abrir el modal
    dispatch(modalOpened());
  };

  return <button onClick={handleClick}>Transferir Parcel</button>;
}
```

## Selectores de Entity Adapter

```tsx
import { useAppSelector } from '@/app/hooks';
import { creditsSelectors, selectTotalCredits } from '@/features/credits/credits.slice';

function CreditsSummary() {
  // Obtener todas las transacciones
  const allTxs = useAppSelector(creditsSelectors.selectAll);
  
  // Obtener una transacción específica
  const txId = 'tx-123';
  const tx = useAppSelector((state) => 
    creditsSelectors.selectById(state, txId)
  );
  
  // Obtener el total
  const count = useAppSelector(creditsSelectors.selectTotal);
  
  // Obtener valor derivado
  const total = useAppSelector(selectTotalCredits);

  return (
    <div>
      <p>Créditos totales: {total}</p>
      <p>Transacciones: {count}</p>
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

## Precarga de datos

Precarga datos antes de la navegación para una UX más rápida:

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';
import { Link } from 'react-router-dom';

function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // Precargar detalles del parcel al pasar el cursor
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

## Gestión manual de caché

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';

function RefreshButton() {
  const dispatch = useAppDispatch();

  const handleRefresh = () => {
    // Invalidar todas las consultas de Parcels
    dispatch(client.util.invalidateTags(['Parcels']));
    
    // O restablecer todo el estado del cliente
    // dispatch(client.util.resetApiState());
  };

  return <button onClick={handleRefresh}>Actualizar datos</button>;
}
```

## Manejo de estados de Query

```tsx
function ComprehensiveExample({ id }: { id: string }) {
  const {
    data,
    isLoading,      // Carga inicial
    isFetching,     // Cualquier fetch (incluido refetch)
    isSuccess,      // La consulta tuvo éxito
    isError,        // La consulta falló
    error,          // Objeto de error
    refetch,        // Función manual de refetch
  } = useGetParcelQuery({ id });

  // Estados diferentes
  if (isLoading) {
    return <Spinner />;
  }

  if (isError) {
    return (
      <div>
        <p>Error: {error.toString()}</p>
        <button onClick={() => refetch()}>Reintentar</button>
      </div>
    );
  }

  if (!data) {
    return <div>No hay datos</div>;
  }

  return (
    <div>
      {isFetching && <div className="refetch-indicator">Actualizando...</div>}
      <h1>{data.name}</h1>
      <button onClick={() => refetch()}>Actualizar</button>
    </div>
  );
}
```

## Integración con formularios

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
        placeholder="Dirección del propietario"
      />
      <input
        type="number"
        value={minPrice}
        onChange={(e) => setMinPrice(e.target.value)}
        placeholder="Precio mínimo"
      />
      <button type="submit">Aplicar filtros</button>
    </form>
  );
}
```

## Buenas prácticas

### 1. Usa hooks tipados

```tsx
// ✅ Bien: Usa hooks tipados
import { useAppSelector, useAppDispatch } from '@/app/hooks';

const value = useAppSelector(selectValue);
const dispatch = useAppDispatch();

// ❌ Mal: Usa hooks sin tipar
import { useSelector, useDispatch } from 'react-redux';

const value = useSelector((state: RootState) => state.value);
const dispatch = useDispatch();
```

### 2. Maneja todos los estados de la consulta

```tsx
// ✅ Bien: Manejo completo del estado
const { data, isLoading, isError, error } = useQuery(args);
if (isLoading) return <Loading />;
if (isError) return <Error error={error} />;
if (!data) return null;
return <Content data={data} />;

// ❌ Mal: Falta manejo de errores
const { data } = useQuery(args);
return <Content data={data} />; // Puede fallar
```

### 3. Usa `.unwrap()` para Mutations

```tsx
// ✅ Bien: Manejo explícito de errores
try {
  const result = await mutation(args).unwrap();
  toast.success('¡Éxito!');
} catch (error) {
  toast.error('¡Falló!');
}

// ❌ Mal: Sin manejo de errores
mutation(args);
```

### 4. Reduce los resultados con `selectFromResult`

```tsx
// ✅ Bien: Suscríbete solo a los campos necesarios
const { name } = useQuery(args, {
  selectFromResult: ({ data }) => ({ name: data?.name }),
});

// ❌ Mal: Suscríbete al objeto completo
const { data } = useQuery(args);
const name = data?.name;
```

### 5. Evita seleccionar slices completas

```tsx
// ✅ Bien: Selecciona valores específicos
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ Mal: Seleccionar el slice completo
const ui = useAppSelector((state) => state.ui);
const viewMode = ui.viewMode;
```

## Próximos pasos

* Aprende sobre [Integración con Web3](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/web3-integration/) para patrones de blockchain
* Revisa [Pruebas y rendimiento](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/testing-and-performance/) para optimización
* Consulta [RTK Query](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/rtk-query/) para la configuración avanzada de endpoints


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/component-patterns.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
