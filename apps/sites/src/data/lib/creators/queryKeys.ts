import type { Query } from '@tanstack/react-query'

/** Key of account-scoped data: Sites removes `["private", account]` on an account switch and `["private"]` on sign-out. */
export function privateKey(account: string | undefined, domain: string, ...inputs: unknown[]) {
  return ['private', account?.toLowerCase() ?? '', domain, ...inputs] as const
}

/** Filter for every entry of one private domain, for a write that changes it whatever the inputs. */
export function privateDomain(domain: string, matches: (query: Query) => boolean = () => true) {
  return {
    predicate: (query: Query) => query.queryKey[0] === 'private' && query.queryKey[2] === domain && matches(query)
  }
}

/** The domain a key belongs to, in either shape. It carries no id, address or search term. */
export function domainOf(key: readonly unknown[] | undefined): string | undefined {
  const domain = key?.[0] === 'private' ? key[2] : key?.[0]
  return typeof domain === 'string' ? domain : undefined
}
