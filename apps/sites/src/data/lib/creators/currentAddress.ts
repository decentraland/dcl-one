// Who is signed in, for the cross-cutting concerns that stamp it on what they send (analytics, Sentry).
// They can't read the wallet store directly: the store imports THEM, and a cycle between the two is
// fragile at module-init time. The store registers a reader instead, and reads stay lazy so an event
// fired before sign-in still picks up the address once there is one.
let read: () => string | undefined = () => undefined

export function setCurrentAddressReader(reader: () => string | undefined): void {
  read = reader
}

/** The connected wallet, or undefined. Never throws: a broken reader must not break the flow being tracked. */
export function currentAddress(): string | undefined {
  try {
    return read()
  } catch {
    return undefined
  }
}
