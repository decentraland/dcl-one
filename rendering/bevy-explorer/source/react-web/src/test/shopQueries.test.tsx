import { useQueryClient, type QueryClient } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ShopQueries } from '../features/shop/ShopQueries'

const seen: QueryClient[] = []
function Probe(): null {
  const client = useQueryClient()
  if (!seen.includes(client)) seen.push(client)
  return null
}

describe('ShopQueries', () => {
  it('keeps one cache per account and clears the one another account replaces', () => {
    seen.length = 0
    const view = render(<ShopQueries account="0xAbC"><Probe /></ShopQueries>)
    seen[0]!.setQueryData(['balance', '0xabc'], 7)
    view.rerender(<ShopQueries account="0xabc"><Probe /></ShopQueries>)
    expect(seen).toHaveLength(1)
    view.rerender(<ShopQueries account="0xdef"><Probe /></ShopQueries>)
    expect(seen).toHaveLength(2)
    expect(seen[1]!.getQueryData(['balance', '0xabc'])).toBeUndefined()
    expect(seen[0]!.getQueryData(['balance', '0xabc'])).toBeUndefined()
  })

  it('clears its cache when the Shop unmounts', () => {
    seen.length = 0
    const view = render(<ShopQueries account="0xabc"><Probe /></ShopQueries>)
    seen[0]!.setQueryData(['purchases', '0xabc'], ['urn'])
    view.unmount()
    expect(seen[0]!.getQueryData(['purchases', '0xabc'])).toBeUndefined()
  })
})
