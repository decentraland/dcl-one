import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { GeneralMana, GeneralPolygonMana } from '../../foundations/icons/general'
import { ShopManaMark } from './ShopManaMark'

describe('ShopManaMark', () => {
  describe('when the network is omitted', () => {
    it('should show the Ethereum foundation icon at the inline price size', () => {
      expect(renderToStaticMarkup(<ShopManaMark />)).toBe(
        renderToStaticMarkup(<GeneralMana size={16} aria-hidden="true" />)
      )
    })
  })

  describe.each(['polygon', 'matic'] as const)('when the network is %s', (network) => {
    it('should show the Polygon foundation icon with the requested size and class', () => {
      expect(
        renderToStaticMarkup(<ShopManaMark network={network} size={24} className="price" />)
      ).toBe(
        renderToStaticMarkup(<GeneralPolygonMana size={24} className="price" aria-hidden="true" />)
      )
    })
  })
})
