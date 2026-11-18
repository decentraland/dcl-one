import 'react'

// React 18's DOM types predate `inert`; React passes an empty string through, which sets the attribute.
declare module 'react' {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars -- the type parameter must match React's to merge
  interface HTMLAttributes<T> {
    inert?: '' | undefined
  }
}
