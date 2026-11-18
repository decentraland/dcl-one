import { useEffect, useState } from 'react'
import { afterLoadIdle } from '~/lib/idle'

/** Turns true once the page has loaded and gone idle; see `afterLoadIdle`. */
export function useAfterLoadIdle(): boolean {
  const [idle, setIdle] = useState(false)
  useEffect(() => afterLoadIdle(() => setIdle(true)), [])
  return idle
}
