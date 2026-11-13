import { useQueryClient, type QueryClient } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { StrictMode, useEffect } from 'react'
import { beforeEach, describe, expect, it } from 'vitest'
import { ShopQueries } from '../features/shop/ShopQueries'

const seen: QueryClient[] = []
let mounts = 0
function Probe(): null {
  const client = useQueryClient()
  if (!seen.includes(client)) seen.push(client)
  useEffect(() => {
    mounts += 1
  }, [])
  return null
}

beforeEach(() => {
  seen.length = 0
  mounts = 0
})

describe('ShopQueries', () => {
  it('keeps the open Shop and its cache when the first account arrives with the profile', () => {
    const view = render(<ShopQueries account={undefined}><Probe /></ShopQueries>)
    seen[0]!.setQueryData(['catalogue'], ['urn'])
    view.rerender(<ShopQueries account="0xAbC"><Probe /></ShopQueries>)
    view.rerender(<ShopQueries account="0xabc"><Probe /></ShopQueries>)
    expect(seen).toHaveLength(1)
    expect(mounts).toBe(1)
    expect(seen[0]!.getQueryData(['catalogue'])).toEqual(['urn'])
  })

  it('remounts with a fresh client when another account replaces it, and empties the replaced one', () => {
    const view = render(<ShopQueries account="0xabc"><Probe /></ShopQueries>)
    seen[0]!.setQueryData(['balance', '0xabc'], 7)
    view.rerender(<ShopQueries account="0xdef"><Probe /></ShopQueries>)
    expect(seen).toHaveLength(2)
    expect(seen[1]).not.toBe(seen[0])
    expect(mounts).toBe(2)
    expect(seen[0]!.getQueryData(['balance', '0xabc'])).toBeUndefined()
  })

  it('treats signing out as a replaced account', () => {
    const view = render(<ShopQueries account="0xabc"><Probe /></ShopQueries>)
    seen[0]!.setQueryData(['purchases', '0xabc'], ['urn'])
    view.rerender(<ShopQueries account={undefined}><Probe /></ShopQueries>)
    expect(seen).toHaveLength(2)
    expect(seen[0]!.getQueryData(['purchases', '0xabc'])).toBeUndefined()
  })

  it('keeps what it read on mount through StrictMode mounting it twice', () => {
    // Written while rendering, before StrictMode's simulated unmount and remount run.
    function Reader(): null {
      const client = useQueryClient()
      if (!seen.includes(client)) seen.push(client)
      if (client.getQueryData(['balance', '0xabc']) === undefined) client.setQueryData(['balance', '0xabc'], 7)
      return null
    }
    render(
      <StrictMode>
        <ShopQueries account="0xabc"><Reader /></ShopQueries>
      </StrictMode>
    )
    expect(seen[0]!.getQueryData(['balance', '0xabc'])).toBe(7)
  })
})
