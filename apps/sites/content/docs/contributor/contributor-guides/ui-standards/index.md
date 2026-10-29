# UI Standards

This section provides comprehensive guidelines for building user interfaces in Decentraland applications using Redux Toolkit (RTK) and RTK Query. These standards ensure consistent state management, optimal performance, and maintainable code across all UI projects.

## Overview

Our UI architecture is built on three core technologies:

* **Redux Toolkit (RTK)** - Modern Redux with less boilerplate
* **RTK Query** - Powerful data fetching and caching
* **TypeScript** - Type-safe state management

### Guiding Principles

1. **Simplicity first** - Use RTK Query for remote data; `createSlice` for UI/local state
2. **Feature co-location** - Keep related code together (slices, endpoints, selectors)
3. **Serializable store** - Never store non-serializable objects in Redux
4. **Lean store** - Keep ephemeral state in React components
5. **Strict typing** - Leverage TypeScript for better DX and fewer bugs
6. **Web3-aware** - Handle blockchain state changes gracefully

## Documentation Sections

* **[Store Setup](/docs/contributor/contributor-guides/ui-standards/store-setup/)** - Configure the Redux store, typed hooks, and folder structure
* **[RTK Query](/docs/contributor/contributor-guides/ui-standards/rtk-query/)** - Data fetching, caching, tags, and invalidation
* **[State Management](/docs/contributor/contributor-guides/ui-standards/state-management/)** - Creating slices and managing UI state
* **[Component Patterns](/docs/contributor/contributor-guides/ui-standards/component-patterns/)** - Using hooks effectively in components
* **[Web3 Integration](/docs/contributor/contributor-guides/ui-standards/web3-integration/)** - Patterns for blockchain integration
* **[Testing & Performance](/docs/contributor/contributor-guides/ui-standards/testing-and-performance/)** - Best practices, testing, and optimization

## Quick Reference: Glossary

### Core Concepts

| Term | Description |
|------|-------------|
| **Store** | The single Redux state tree for the app |
| **Slice** | A module created with `createSlice` (name + initial state + reducers) |
| **Action** | A plain object describing "what happened" (`type` + optional `payload`) |
| **Reducer** | A pure function that updates state based on actions |
| **Selector** | A function that reads/derives data from the store (can be memoized) |

### RTK Query

| Term | Description |
|------|-------------|
| **RTK Query** | Data-fetching layer: endpoints, caching, refetch, invalidations |
| **Base Query** | The low-level fetcher (e.g., `fetchBaseQuery`) used by endpoints |
| **Endpoint** | One RTK Query operation (`query` = read; `mutation` = write) |
| **Tags** | Strings/objects for cache `provides`/`invalidates` to sync views |
| **`onQueryStarted`** | Lifecycle hook for optimistic updates and rollback |

### Patterns

| Term | Description |
|------|-------------|
| **Optimistic update** | Temporarily update UI before server confirms; rollback if fails |
| **Entity Adapter** | Helpers to manage normalized collections (ids/entities, sorting) |
| **UI/local state** | Client-owned state (filters, modals, form inputs) |
| **Remote data** | Server-owned/cached data (catalog, NFTs, orders) |
| **Typed hooks** | `useAppSelector`/`useAppDispatch` bound to `RootState`/`AppDispatch` |

## Why Typed Hooks?

We use `useAppSelector` and `useAppDispatch` instead of the standard Redux hooks because:

* **Type Safety** - Selector return types are checked against `RootState`
* **Less Boilerplate** - No `<RootState>` generics needed per usage
* **Easier Refactors** - Change store types once; all components follow
* **Better DX** - Works seamlessly with RTK Query type inference

## Getting Started

1. Review the [Store Setup](/docs/contributor/contributor-guides/ui-standards/store-setup/) to understand project structure
2. Learn [RTK Query](/docs/contributor/contributor-guides/ui-standards/rtk-query/) for managing remote data
3. Understand [State Management](/docs/contributor/contributor-guides/ui-standards/state-management/) for local UI state
4. Study [Component Patterns](/docs/contributor/contributor-guides/ui-standards/component-patterns/) for implementation
5. Follow [Web3 Integration](/docs/contributor/contributor-guides/ui-standards/web3-integration/) for blockchain features
6. Apply [Testing & Performance](/docs/contributor/contributor-guides/ui-standards/testing-and-performance/) best practices

## Standards Compliance

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this documentation are to be interpreted as described in [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).

## What's Different About Our Approach

### RTK Query First

We prioritize RTK Query for all remote data instead of writing custom async actions. This gives us:

* Built-in caching and deduplication
* Automatic refetching and background updates
* Optimistic updates with rollback
* Loading and error states out of the box

### Minimal Store Footprint

We keep the Redux store focused on:

* **Remote data cache** (via RTK Query)
* **Global UI state** (filters, preferences, session)
* **Cross-component state** (modals, notifications)

Transient state stays in React components.

### Web3 Considerations

Our patterns account for blockchain-specific challenges:

* Non-serializable objects (providers, signers) stay outside Redux
* Cache invalidation on network/account changes
* Optimistic updates for pending transactions
* Event-driven state updates from blockchain events

## Anti-Patterns to Avoid


> [!CAUTION]
> **Never do these:**
> 
> * Store `window.ethereum`, providers, signers, or sockets in Redux
> * Duplicate the same collection in a slice and in RTK Query
> * Dispatch actions during React render
> * Return new objects from selectors without memoization
> * Create mutations that don't invalidate or patch caches


## Next Steps

Start with [Store Setup](/docs/contributor/contributor-guides/ui-standards/store-setup/) to learn how to structure your project and configure Redux properly.

## Related Standards

* [Dependency Management](/docs/contributor/contributor-guides/dependency-management/) - Managing npm dependencies and peerDependencies
* [Testing Standards](/docs/contributor/contributor-guides/testing-standards/) - Testing patterns for UIs and services
* [Web UI Standards](/docs/contributor/contributor-guides/web-ui-standards/) - Component library and design system
