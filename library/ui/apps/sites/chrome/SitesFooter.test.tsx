import { render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { ChromeNavContext } from '../../../components/AppChrome/chrome-nav'
import SitesFooter from './SitesFooter'

const docsUrl = (path = '') => `https://docs.example.com/${path}`
const subscribe = () => Promise.resolve()
// jsdom cannot follow a link; this runs after React's handlers, so it does not hide a navigation from them.
const stayOnPage = (event: MouseEvent) => event.preventDefault()
beforeEach(() => window.addEventListener('click', stayOnPage))
afterEach(() => window.removeEventListener('click', stayOnPage))

describe('SitesFooter', () => {
  let user: UserEvent
  let onSocialClick: Mock<(platform: string) => void>
  let onLinkClick: Mock<(label: string) => void>
  let navigate: Mock<(href: string) => boolean>

  beforeEach(() => {
    user = userEvent.setup()
    onSocialClick = vi.fn()
    onLinkClick = vi.fn()
    navigate = vi.fn(() => true)
    render(
      <ChromeNavContext.Provider value={{ navigate }}>
        <SitesFooter
          docsUrl={docsUrl}
          onSubscribe={subscribe}
          onSocialClick={onSocialClick}
          onLinkClick={onLinkClick}
        />
      </ChromeNavContext.Provider>
    )
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  const footer = () => within(screen.getByRole('contentinfo'))

  it('should report a social icon by its platform name', async () => {
    await user.click(footer().getAllByRole('link', { name: 'Discord' })[0]!)
    expect(onSocialClick).toHaveBeenCalledExactlyOnceWith('Discord')
    expect(onLinkClick).not.toHaveBeenCalled()
  })

  it('should report an external link by its label', async () => {
    await user.click(footer().getAllByRole('link', { name: 'Vote' })[0]!)
    expect(onLinkClick).toHaveBeenCalledExactlyOnceWith('Vote')
  })

  it('should report an app link by its label and still navigate in the app', async () => {
    await user.click(footer().getAllByRole('link', { name: 'Contact Support' })[0]!)
    expect(onLinkClick).toHaveBeenCalledExactlyOnceWith('Contact Support')
    expect(navigate).toHaveBeenCalledWith('/help')
  })

  it('should report nothing for a legal link', async () => {
    await user.click(footer().getByRole('link', { name: 'Privacy Policy' }))
    expect(onLinkClick).not.toHaveBeenCalled()
    expect(onSocialClick).not.toHaveBeenCalled()
  })
})

describe('SitesFooter without click callbacks', () => {
  it('should still navigate an app link', async () => {
    const user = userEvent.setup()
    const navigate = vi.fn(() => true)
    render(
      <ChromeNavContext.Provider value={{ navigate }}>
        <SitesFooter docsUrl={docsUrl} onSubscribe={subscribe} />
      </ChromeNavContext.Provider>
    )
    await user.click(within(screen.getByRole('contentinfo')).getAllByRole('link', { name: 'Download Decentraland' })[0]!)
    expect(navigate).toHaveBeenCalledWith('/download')
  })
})
