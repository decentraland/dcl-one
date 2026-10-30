# Component Patterns

This page covers how to effectively use Redux and RTK Query in React components, including hooks, optimization patterns, and best practices.

## Basic Query Usage

Use generated hooks from RTK Query endpoints:

```tsx
import { useGetParcelByCoordsQuery } from '@/features/land/land.client';

function ParcelCard({ x, y }: { x: number; y: number }) {
  const { data, isLoading, isError, error } = useGetParcelByCoordsQuery({ x, y });

  if (isLoading) return <div>Loading...</div>;
  if (isError) return <div>Error: {error.toString()}</div>;
  if (!data) return null;

  return (
    <div>
      <h2>{data.name || `Parcel ${x},${y}`}</h2>
      <p>Owner: {data.owner}</p>
    </div>
  );
}
```

## Optimizing Re-renders with `selectFromResult`

Narrow the result to only the fields you need:

```tsx
function ParcelOwner({ x, y }: { x: number; y: number }) {
  // ✅ Good: Only subscribes to owner field changes
  const { owner, isFetching } = useGetParcelByCoordsQuery(
    { x, y },
    {
      selectFromResult: ({ data, isFetching }) => ({
        owner: data?.owner,
        isFetching,
      }),
    }
  );

  if (isFetching) return <span>Loading...</span>;
  return <span>Owner: {owner}</span>;
}

// ❌ Bad: Re-renders on any data change
function ParcelOwnerBad({ x, y }: { x: number; y: number }) {
  const { data } = useGetParcelByCoordsQuery({ x, y });
  return <span>Owner: {data?.owner}</span>;
}
```

## Conditional Queries with `skip`

Skip queries when arguments aren't ready:

```tsx
function UserProfile() {
  const { data: session } = useGetSessionQuery();
  const userId = session?.userId;

  // Skip profile query until we have userId
  const { data: profile } = useGetProfileQuery(
    { id: userId! },
    {
      skip: !userId, // Don't fetch if userId is undefined
    }
  );

  if (!userId) return <div>Please log in</div>;
  if (!profile) return <div>Loading profile...</div>;

  return <div>{profile.name}</div>;
}
```

## Polling for Real-time Updates

```tsx
function LiveBalance({ address }: { address: string }) {
  const { data } = useGetBalanceQuery(
    { address },
    {
      pollingInterval: 10000, // Poll every 10 seconds
      skipPollingIfUnfocused: true, // Pause when tab is not focused
    }
  );

  return <div>Balance: {data?.amount ?? 0}</div>;
}
```

## Lazy Queries

Trigger queries manually instead of on component mount:

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
        placeholder="Enter owner address"
      />
      <button onClick={handleSearch}>Search</button>
      
      {result.isLoading && <div>Searching...</div>}
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

## Mutation Usage

### Basic Mutation

```tsx
function GrantCreditsButton({ address }: { address: string }) {
  const [grantCredits, { isLoading, isError, error }] = useGrantCreditsMutation();

  const handleGrant = async () => {
    try {
      await grantCredits({ address, amount: 100 }).unwrap();
      toast.success('Credits granted successfully!');
    } catch (err) {
      toast.error('Failed to grant credits');
      console.error('Grant failed:', err);
    }
  };

  return (
    <button onClick={handleGrant} disabled={isLoading}>
      {isLoading ? 'Granting...' : 'Grant 100 Credits'}
    </button>
  );
}
```

### Mutation with Feedback

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
      toast.success(`Parcel renamed to "${result.name}"`);
      setName(''); // Clear form
    } catch (err) {
      if (isFetchBaseQueryError(err)) {
        toast.error(`Error: ${JSON.stringify(err.data)}`);
      } else {
        toast.error('An unexpected error occurred');
      }
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="New name"
        disabled={isLoading}
      />
      <button type="submit" disabled={isLoading || !name}>
        {isLoading ? 'Updating...' : 'Update Name'}
      </button>
      
      {isSuccess && <p className="success">Name updated!</p>}
      {isError && <p className="error">Update failed</p>}
    </form>
  );
}
```

## Using Slice State

Access slice state with `useAppSelector`:

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
      View: {viewMode === 'grid' ? '⊞ Grid' : '☰ List'}
    </button>
  );
}
```

## Combining Multiple Queries

```tsx
function ParcelDetails({ id }: { id: string }) {
  const { data: parcel, isLoading: parcelLoading } = useGetParcelQuery({ id });
  const { data: owner, isLoading: ownerLoading } = useGetProfileQuery(
    { id: parcel?.owner! },
    { skip: !parcel?.owner }
  );

  const isLoading = parcelLoading || ownerLoading;

  if (isLoading) return <div>Loading...</div>;
  if (!parcel) return <div>Parcel not found</div>;

  return (
    <div>
      <h1>{parcel.name}</h1>
      <p>Coordinates: {parcel.x}, {parcel.y}</p>
      {owner && <p>Owner: {owner.name}</p>}
    </div>
  );
}
```

## Derived State with Selectors

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
        {filterCount > 0 && ` - ${filterCount} filter(s) active`}
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

## Dispatching Actions

```tsx
import { useAppDispatch } from '@/app/hooks';
import { modalOpened, modalClosed } from '@/features/ui/ui.slice';

function TransferButton({ parcelId }: { parcelId: string }) {
  const dispatch = useAppDispatch();

  const handleClick = () => {
    // Dispatch action to open modal
    dispatch(modalOpened());
  };

  return <button onClick={handleClick}>Transfer Parcel</button>;
}
```

## Entity Adapter Selectors

```tsx
import { useAppSelector } from '@/app/hooks';
import { creditsSelectors, selectTotalCredits } from '@/features/credits/credits.slice';

function CreditsSummary() {
  // Get all transactions
  const allTxs = useAppSelector(creditsSelectors.selectAll);
  
  // Get specific transaction
  const txId = 'tx-123';
  const tx = useAppSelector((state) => 
    creditsSelectors.selectById(state, txId)
  );
  
  // Get total count
  const count = useAppSelector(creditsSelectors.selectTotal);
  
  // Get derived value
  const total = useAppSelector(selectTotalCredits);

  return (
    <div>
      <p>Total Credits: {total}</p>
      <p>Transactions: {count}</p>
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

## Prefetching Data

Prefetch data before navigation for faster UX:

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';
import { Link } from 'react-router-dom';

function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // Prefetch parcel details on hover
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

## Manual Cache Management

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';

function RefreshButton() {
  const dispatch = useAppDispatch();

  const handleRefresh = () => {
    // Invalidate all Parcels queries
    dispatch(client.util.invalidateTags(['Parcels']));
    
    // Or reset entire client state
    // dispatch(client.util.resetApiState());
  };

  return <button onClick={handleRefresh}>Refresh Data</button>;
}
```

## Handling Query States

```tsx
function ComprehensiveExample({ id }: { id: string }) {
  const {
    data,
    isLoading,      // Initial load
    isFetching,     // Any fetch (including refetch)
    isSuccess,      // Query succeeded
    isError,        // Query failed
    error,          // Error object
    refetch,        // Manual refetch function
  } = useGetParcelQuery({ id });

  // Different states
  if (isLoading) {
    return <Spinner />;
  }

  if (isError) {
    return (
      <div>
        <p>Error: {error.toString()}</p>
        <button onClick={() => refetch()}>Retry</button>
      </div>
    );
  }

  if (!data) {
    return <div>No data</div>;
  }

  return (
    <div>
      {isFetching && <div className="refetch-indicator">Updating...</div>}
      <h1>{data.name}</h1>
      <button onClick={() => refetch()}>Refresh</button>
    </div>
  );
}
```

## Form Integration

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
        placeholder="Owner address"
      />
      <input
        type="number"
        value={minPrice}
        onChange={(e) => setMinPrice(e.target.value)}
        placeholder="Min price"
      />
      <button type="submit">Apply Filters</button>
    </form>
  );
}
```

## Best Practices

### 1. Use Typed Hooks

```tsx
// ✅ Good: Uses typed hooks
import { useAppSelector, useAppDispatch } from '@/app/hooks';

const value = useAppSelector(selectValue);
const dispatch = useAppDispatch();

// ❌ Bad: Uses untyped hooks
import { useSelector, useDispatch } from 'react-redux';

const value = useSelector((state: RootState) => state.value);
const dispatch = useDispatch();
```

### 2. Handle All Query States

```tsx
// ✅ Good: Comprehensive state handling
const { data, isLoading, isError, error } = useQuery(args);
if (isLoading) return <Loading />;
if (isError) return <Error error={error} />;
if (!data) return null;
return <Content data={data} />;

// ❌ Bad: Missing error handling
const { data } = useQuery(args);
return <Content data={data} />; // May crash
```

### 3. Use `.unwrap()` for Mutations

```tsx
// ✅ Good: Explicit error handling
try {
  const result = await mutation(args).unwrap();
  toast.success('Success!');
} catch (error) {
  toast.error('Failed!');
}

// ❌ Bad: No error handling
mutation(args);
```

### 4. Narrow Results with `selectFromResult`

```tsx
// ✅ Good: Only subscribe to needed fields
const { name } = useQuery(args, {
  selectFromResult: ({ data }) => ({ name: data?.name }),
});

// ❌ Bad: Subscribe to entire object
const { data } = useQuery(args);
const name = data?.name;
```

### 5. Avoid Selecting Entire Slices

```tsx
// ✅ Good: Select specific values
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ Bad: Select entire slice
const ui = useAppSelector((state) => state.ui);
const viewMode = ui.viewMode;
```

## Next Steps

* Learn about [Web3 Integration](/docs/contributor/contributor-guides/ui-standards/web3-integration/) for blockchain patterns
* Review [Testing & Performance](/docs/contributor/contributor-guides/ui-standards/testing-and-performance/) for optimization
* See [RTK Query](/docs/contributor/contributor-guides/ui-standards/rtk-query/) for advanced endpoint configuration
