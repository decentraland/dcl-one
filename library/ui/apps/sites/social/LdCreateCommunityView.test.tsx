import { render, screen } from '@testing-library/react'
import type { ReactNode } from 'react'
import { describe, expect, it } from 'vitest'
import LdCreateCommunityView from './LdCreateCommunityView'

describe('LdCreateCommunityView', () => {
  describe('when creating a community', () => {
    it('should mark the create link as the current mode and show no notice', () => {
      render(<LdCreateCommunityView />)
      expect(screen.getByRole('link', { name: 'Create community' })).toHaveAttribute(
        'aria-current',
        'page'
      )
      expect(screen.getByRole('link', { name: 'Edit community' })).not.toHaveAttribute(
        'aria-current'
      )
      expect(screen.queryByRole('status')).toBeNull()
    })
  })

  describe('when editing a community that loaded', () => {
    it('should show no fallback notice', () => {
      render(<LdCreateCommunityView mode="edit" source="live" editHref="?mode=edit&id=c1" />)
      expect(screen.getByRole('link', { name: 'Edit community' })).toHaveAttribute(
        'href',
        '?mode=edit&id=c1'
      )
      expect(screen.queryByRole('status')).toBeNull()
    })
  })

  describe('when editing a community that failed to load', () => {
    it('should say the draft starts blank because the community could not be loaded', () => {
      render(<LdCreateCommunityView mode="edit" source="error" />)
      expect(screen.getByRole('status')).toHaveTextContent("Couldn't load that community to edit")
    })
  })

  describe('when editing without a selected community', () => {
    it('should say no community was selected', () => {
      render(<LdCreateCommunityView mode="edit" source="empty" />)
      expect(screen.getByRole('status')).toHaveTextContent('No community selected to edit')
    })
  })

  describe('when the app supplies its own link component', () => {
    it('should render the mode links and the children through it', () => {
      const AppLink = ({ to, children, ...rest }: { to: string; children?: ReactNode }) => (
        <a data-app-link href={to} {...rest}>
          {children}
        </a>
      )
      render(
        <LdCreateCommunityView LinkComponent={AppLink}>
          <p>wizard</p>
        </LdCreateCommunityView>
      )
      expect(screen.getByRole('link', { name: 'Create community' })).toHaveAttribute(
        'data-app-link'
      )
      expect(screen.getByText('wizard')).toBeInTheDocument()
    })
  })
})
