// The in-world Shop's NAMEs: a typed NAME checked as the web Shop's NAMEs page checks it (its length, the
// banned list, then the registrar on the Ethereum network the Shop registers on), and the Credits a NAME
// costs. A free NAME is only available with that price: without a credits quote the web page offers it for
// MANA alone, which it reads as taken. The registrar's answer is advisory: the purchase checks again.

import { queryOptions, useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { shopClient } from '../../../shop/api/shop'
import { creditsCeil } from '../../../shop/model/money'
import { fetchBannedNames } from '../../../shop/names/api'
import { checkNameAvailability, NAME_CHECK_DEBOUNCE_MS, NAME_MAX_LENGTH, NAME_MIN_LENGTH, type NameAvailability } from '../../../shop/names/names'
import { quoteNameCredits } from '../../../shop/names/offer'
import type { ShopNameStatus } from './ui'

export const nameQueries = {
  banned: () =>
    queryOptions({
      queryKey: ['shop', 'names', 'banned'] as const,
      queryFn: ({ signal }) => fetchBannedNames({ signal }).then((names) => new Set(names)),
      staleTime: 10 * 60_000
    }),
  /** A NAME's price in whole Credits (the credits quote of its 100 MANA), or null while none can be given. */
  price: () =>
    queryOptions({
      queryKey: ['shop', 'names', 'price'] as const,
      queryFn: async ({ signal }) => {
        const quote = await quoteNameCredits(shopClient(), signal)
        return 'quoted' in quote ? creditsCeil(quote.quoted) : null
      },
      staleTime: 60_000
    }),
  availability: (name: string) =>
    queryOptions({
      queryKey: ['shop', 'names', 'availability', name.toLowerCase()] as const,
      queryFn: ({ signal }): Promise<NameAvailability> => checkNameAvailability(name, { signal }),
      staleTime: 30_000
    })
}

export interface NameCheck {
  status: ShopNameStatus
  /** The NAME's price in whole Credits, once known. */
  price: number | null
  /** Asks the registrar again after a failed check. */
  retry: () => void
}

/** What `name` (letters and digits, as typed) is, read while `enabled`. */
export function useNameCheck(name: string, enabled: boolean): NameCheck {
  // The NAME the typing last rested on: only that one is checked.
  const [settled, setSettled] = useState(name)
  useEffect(() => {
    const t = setTimeout(() => setSettled(name), NAME_CHECK_DEBOUNCE_MS)
    return () => clearTimeout(t)
  }, [name])
  const length = name.length
  const valid = length >= NAME_MIN_LENGTH && length <= NAME_MAX_LENGTH
  const banned = useQuery({ ...nameQueries.banned(), enabled })
  const isBanned = valid && banned.data?.has(name.toLowerCase()) === true
  // Until the banned list answers a NAME isn't offered; once it fails, the registrar alone decides (the
  // purchase checks again).
  const listKnown = !banned.isPending
  const ask = enabled && valid && listKnown && !isBanned && settled === name
  const availability = useQuery({ ...nameQueries.availability(settled), enabled: ask })
  const price = useQuery({ ...nameQueries.price(), enabled })

  const status: ShopNameStatus =
    length === 0
      ? 'idle'
      : length < NAME_MIN_LENGTH
        ? 'short'
        : length > NAME_MAX_LENGTH
          ? 'tooLong'
          : isBanned
            ? 'banned'
            : !ask || availability.isPending || (availability.isError && availability.isFetching)
              ? 'checking'
              : availability.isError
                ? 'error'
                : availability.data !== 'available'
                  ? 'taken'
                  : price.isPending || (price.data == null && price.isFetching)
                    ? 'checking'
                    : price.data == null
                      ? 'error'
                      : 'available'
  // TRY AGAIN asks again for what failed: the registrar, or the price.
  const retry = (): void => void (availability.isError ? availability.refetch() : price.refetch())
  return { status, price: price.data ?? null, retry }
}
