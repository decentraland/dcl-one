// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'

import { cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SubNav } from './SubNav'

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true })
})

vi.mock('@/data/lib/creators/idle', () => ({ afterLoadIdle: () => () => undefined }))

afterEach(cleanup)

describe('SubNav', () => {
  describe('when on the overview', () => {
    beforeEach(() => {
      render(
        <MemoryRouter initialEntries={['/create']}>
          <SubNav />
        </MemoryRouter>
      )
    })

    it('should mark Overview as the current page', () => {
      expect(screen.getByRole('link', { name: 'Overview', current: 'page' })).toHaveAttribute('href', '/create')
    })

    it('should link Land/World to the Creator Hub download', () => {
      expect(screen.getByRole('link', { name: 'Land/World' })).toHaveAttribute('href', '/download/creator-hub')
    })

    it('should omit the curation route before the committee check', () => {
      expect(screen.queryByRole('link', { name: 'Curation' })).not.toBeInTheDocument()
    })
  })

  describe('when on a collection detail route', () => {
    beforeEach(() => {
      render(
        <MemoryRouter initialEntries={['/create/collections/example']}>
          <SubNav />
        </MemoryRouter>
      )
    })

    it('should mark Collections as the current section', () => {
      expect(screen.getByRole('link', { name: 'Collections', current: 'page' })).toHaveAttribute('href', '/create/collections')
    })
  })
})
