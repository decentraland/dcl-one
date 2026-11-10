import styles from './SitesNavbar.module.css'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SitesNavbar } from './SitesNavbar'

describe.each(['landing', 'compact'] as const)(
  'when the application uses the generated %s header',
  (variant) => {
    let signIn: ReturnType<typeof vi.fn<() => void>>
    beforeEach(() => {
      signIn = vi.fn<() => void>()
    })
    it('should retain the public logo destination and the sign-in callback', async () => {
      const user = userEvent.setup()
      render(
        <SitesNavbar
          variant={variant}
          home={{ href: '/', label: 'Home' }}
          signIn={{ label: 'Sign In', onClick: signIn }}
        />
      )
      const root = screen.getByRole('navigation', { name: 'Main' })
      if (variant === 'landing') {
        expect(screen.getByRole('banner')).toHaveClass(styles.generatedLanding!)
        expect(root).toHaveClass(styles.generatedTransparent!)
      } else {
        expect(screen.getByRole('banner')).not.toHaveClass(styles.generatedLanding!)
        expect(root).not.toHaveClass(styles.generatedTransparent!)
      }
      expect(screen.getByRole('link', { name: 'Decentraland' })).toHaveAttribute('href', '/')
      await user.click(screen.getByRole('button', { name: 'Sign In' }))
      expect(signIn).toHaveBeenCalledTimes(1)
    })
  }
)
describe('when generated mobile account views are used', () => {
  it.todo(
    'should expose source-owned controls after the accepted variants7/8 limitation is corrected'
  )
  it.todo(
    'should bind real count, balance and avatar props after the generated semantic API is available'
  )
})
