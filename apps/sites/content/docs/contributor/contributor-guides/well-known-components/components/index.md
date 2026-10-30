# Components

Components are self-contained pieces of software designed as black boxes that receive other components (or none) to perform their tasks. The main objective is to decouple code sections and maximize testability.

## Directory Structure

All components MUST be defined inside a dedicated directory containing at least these four files:

* `component.ts` - Contains the component's implementation code
* `types.ts` - Contains the component's interface and exposed types
* `errors.ts` - Contains all exported error classes
* `index.ts` - Exports the public API of the component

### Example Structure

```
src/
└── components/
    └── my-component/
        ├── component.ts
        ├── types.ts
        ├── errors.ts
        └── index.ts
```

## Type File

Components conform to an interface that defines their public API. This interface can be shared between interchangeable components or used to understand the methods exposed by the component.

All components that have their own interface MUST define it in a `types.ts` file inside the component's directory.

Other types exposed by the component MUST also be placed in this file.

### Example: `types.ts`

```tsx
export type Something = {
  aValue: boolean
  anotherValue: string
}

export interface IMyComponent {
  myFunction: () => boolean
  getSomething: () => Something
}
```

### Shared Types

Types shared between multiple components, such as common interfaces, MUST be placed in a `types.ts` file in the root directory of the project's source code (`/src/types.ts`).

## Component File

The component file contains the component creator function. This function follows a specific naming and structure convention.

### Naming Convention

Component creator functions MUST be named as: `create` + `ComponentName` + `Component`

**Example:** `createMyNewComponent`

### Function Signature

The creator function MUST:

1. Receive a components object as the first parameter containing all dependencies
2. Optionally receive additional configuration parameters
3. Return an object containing the exposed (public) methods

### Component Structure

At the start of the component creator function:

1. Extract dependencies from the components object
2. Initialize common variables (e.g., loggers, configuration)
3. Define internal helper functions
4. Define public methods
5. Return the public API

### Example: `component.ts`

```tsx
import { IMyComponent, Something } from './types'
import { WrongStringError } from './errors'

export function createMyNewComponent(
  components: Pick<AppComponents, 'logs'>
): IMyComponent {
  const { logs } = components
  const logger = logs.getLogger('my-component')

  // Internal method - not exposed
  function computeString(fstString: string, sndString: string): string {
    if (fstString.length === 0) {
      throw new WrongStringError()
    }
    
    return fstString + ' ' + sndString
  }

  // Public method - exposed in return object
  function myFunction(): boolean {
    return true
  }

  // Public method - exposed in return object
  function getSomething(): Something {
    logger.info('Getting something')
    
    return {
      aValue: true,
      anotherValue: computeString('aString', 'anotherString')
    }
  }

  // Return the public API
  return {
    myFunction,
    getSomething
  }
}
```

## Errors File

The errors file contains custom error classes that a component can throw. These errors can be used in other components or controllers to correctly identify and handle specific error conditions.

### Example: `errors.ts`

```tsx
export class WrongStringError extends Error {
  constructor(message?: string) {
    super(message || 'Incorrect String')
    this.name = 'WrongStringError'
  }
}

export class ComponentNotInitializedError extends Error {
  constructor() {
    super('Component has not been initialized')
    this.name = 'ComponentNotInitializedError'
  }
}
```

### Error Handling Best Practices

* Create specific error classes for different error conditions
* Include meaningful error messages
* Set the `name` property to match the class name for easier debugging
* Document which errors can be thrown by each public method

## Index File

The index file serves as the public entry point for the component, exporting only what should be accessible to other parts of the application.

### Example: `index.ts`

```tsx
export { createMyNewComponent } from './component'
export type { IMyComponent, Something } from './types'
export { WrongStringError } from './errors'
```

## Best Practices

1. **Single Responsibility** - Each component should have one clear purpose
2. **Explicit Dependencies** - All dependencies should be injected through the components parameter
3. **Immutability** - Prefer immutable data structures where possible
4. **Error Handling** - Use custom error classes for different error conditions
5. **Logging** - Use the logger for debugging and monitoring
6. **Type Safety** - Leverage TypeScript's type system fully
7. **Documentation** - Document complex methods and business logic

## Component Lifecycle

Some components may need to perform setup or cleanup operations. WKC provides lifecycle methods:

* `[START_COMPONENT]` - Called when the component starts
* `[STOP_COMPONENT]` - Called when the component stops

See the [Adapters](/docs/contributor/contributor-guides/well-known-components/adapters/) section for examples of components using lifecycle methods.
