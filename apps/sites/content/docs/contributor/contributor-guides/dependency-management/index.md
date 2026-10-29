# Dependency Management Standard

This document defines the recommended practices for managing dependencies in JavaScript/TypeScript projects across Decentraland's ecosystem. It applies to front-end, back-end, SDKs, shared libraries, and any npm/yarn/pnpm-based package.

> **TL;DR**: 
> - **Version strategy** → Use **exact/fixed versions** for `dependencies` (security) and `devDependencies` (consistent dev environment). Use **version ranges** only for `peerDependencies` (flexibility).
> - **Exception** → Decentraland packages (`@dcl/*`, `decentraland-*`) can use version ranges (`^`) as they are trusted internal packages.
> - **Libraries/shared packages** → Use `peerDependencies` with version ranges (^) for shared libraries (React, @dcl/schemas, etc.). Use `dependencies` with exact versions only for utilities safe to duplicate.
> - **Apps/final services** → Use `dependencies` with exact versions for runtime packages (React, ethers, etc.).

## 1. Rationale

Many libraries rely on global singletons, React Context, class identity, shared caches, schema enums, or shared DB pools. Installing multiple versions of the same library can cause:
- Context not shared between copies
- Duplicate caches or pools
- Failing instanceof checks
- Enum/symbol mismatches
- Increased bundle size
- Hard-to-diagnose runtime bugs

Correct dependency management prevents these issues.

### Non-goals

This standard does **not** attempt to:
- Enforce a single package manager (npm, yarn, pnpm are all supported)
- Guarantee a single physical copy in `node_modules` (focus is on runtime/bundle deduplication)

## 2. Definitions

| Field | Purpose | Consumer responsibility | Applies to |
|-------|---------|------------------------|------------|
| `dependencies` | Packages used internally by the module | None - consumer is not expected to provide them | Apps and libraries |
| `peerDependencies` | Packages that must resolve to a single effective version at runtime | Must install them (or use peerDependenciesMeta for optional) | **Libraries only** |
| `peerDependenciesMeta` | Metadata for peerDependencies, used to mark peers as optional | None - only affects package manager behavior | **Libraries only** |
| `devDependencies` | Tooling only used during development | None | Apps and libraries |

### About peerDependenciesMeta

`peerDependenciesMeta` is a field that provides metadata about your `peerDependencies`. The most common use case is marking peers as **optional**:

- **Required peers** (default): Package manager will warn if not installed
- **Optional peers**: Package manager won't warn if missing; your package should handle their absence gracefully

This is useful for packages that can work with or without certain dependencies (e.g., a utility that works with or without React, or a library that supports multiple Web3 providers).

### Apps vs Libraries

**Libraries / Shared Packages:**
- Use `peerDependencies` for packages that must resolve to a single effective version at runtime (React, ethers, @dcl/schemas)
- Consumer (app) provides these dependencies
- Prevents duplicate installations and singleton conflicts

**Apps / Final Services:**
- Often use `dependencies` for runtime packages (React, ethers, etc.)
- They are the final consumer, so duplication is not a concern
- `peerDependencies` still valid if the app might be consumed as a dependency

## 3. When to Use Each Field

> **Quick reference**: See [Section 4](#4-examples) for common packages and their recommended placement.

### Use peerDependencies for (Libraries only):

**In shared libraries/packages**, use `peerDependencies` for packages that must resolve to a single effective version at runtime:
- React, Redux, wagmi, ethers, viem
- @dcl/schemas and similar cross-ecosystem libraries
- decentraland/connect
- DB drivers when sharing pools
- Any library relying on singletons or context

✅ Correct (Library):
```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "@dcl/schemas": "^20.0.0"
  }
}
```

❌ Incorrect (Library using dependencies for shared libs):
```json
{
  "dependencies": {
    "react": "^18.0.0"
  }
}
```

### Use dependencies for:
- Utilities safe to duplicate (lodash-es, date-fns)
- **In apps**: Runtime packages like React, ethers (when app is final consumer)

> **Important**: Always use **exact/fixed versions** in `dependencies` for security.

✅ Correct (Library):
```json
{
  "dependencies": {
    "lodash-es": "4.17.21",
    "date-fns": "3.6.0"
  }
}
```

✅ Correct (App):
```json
{
  "dependencies": {
    "react": "18.3.1",
    "ethers": "6.13.0",
    "lodash-es": "4.17.21"
  }
}
```

### Optional peerDependencies (peerDependenciesMeta)

For reusable packages that work with or without certain dependencies, use `peerDependenciesMeta` to mark peers as optional:

✅ Correct:
```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "ethers": "^6.0.0"
  },
  "peerDependenciesMeta": {
    "ethers": {
      "optional": true
    }
  }
}
```

**What this means:**
- `react` is **required**: Consumer must install it or package manager will warn
- `ethers` is **optional**: Consumer doesn't need to install it; your package should check for its presence before using it

**Use cases:**
- Packages that work in both React and non-React environments
- Libraries that support multiple Web3 providers (ethers, viem, etc.)
- Utilities that enhance other libraries but aren't required

### Use devDependencies for:
- Tooling (TypeScript, ESLint, testers, bundlers)

> **Important**: Always use **exact/fixed versions** in `devDependencies` to ensure a consistent development environment across the team.

✅ Correct:
```json
{
  "devDependencies": {
    "typescript": "5.4.5",
    "eslint": "8.57.0",
    "vitest": "1.6.0"
  }
}
```

## 4. Examples

### Packages that MUST be peerDependencies (shared context / singletons)

- `react`, `react-dom`, `react-redux`, `react-router-dom`
- `redux`, `@reduxjs/toolkit`
- `ethers`, `viem`, `wagmi`
- `@dcl/schemas`, `@dcl/ui-env`, `@dcl/crypto`
- `decentraland-dapps`, `decentraland-ui`, `decentraland-ui2`, `decentraland-connect`
- `pg`, `pg-pool`

### Packages that SHOULD be dependencies (safe to duplicate)

- `lodash-es`, `date-fns`
- `uuid`, `nanoid`
- `zod`, `ajv`
- `ms`, `mitt`, `fp-future`

## 5. Related Standards

- [Testing Standards](/docs/contributor/contributor-guides/testing-standards/)
- [UI Standards](/docs/contributor/contributor-guides/ui-standards/)
- [Web UI Standards](/docs/contributor/contributor-guides/web-ui-standards/)
- [WKC Architecture](/docs/contributor/contributor-guides/well-known-components/)
- [API Documentation Guidelines](/docs/contributor/contributor-guides/api-documentation/)
