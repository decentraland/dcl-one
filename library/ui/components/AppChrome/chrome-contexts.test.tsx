import { renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { describe, expect, it } from 'vitest'

import { ChromeAuthContext, useChromeAuth, type ChromeAuth } from './chrome-auth'
import { ChromeNavContext, useChromeNav } from './chrome-nav'

describe('useChromeAuth', () => {
  it('reads a signed-out account with no actions outside a provider', () => {
    const { result } = renderHook(() => useChromeAuth())
    expect(result.current).toEqual({
      signedIn: false,
      account: '',
      mana: '',
      name: '',
      committee: false,
      avatarUrl: undefined,
      avatarColor: undefined,
      onSignIn: undefined,
      onSignOut: undefined,
      onSwitchAccount: undefined,
      fetchNotifications: undefined
    })
  })

  it("reads the host's value inside a provider", () => {
    const auth: ChromeAuth = {
      signedIn: true,
      account: '0xabc',
      mana: '1',
      name: 'Ann',
      committee: true
    }
    const wrapper = ({ children }: { children: ReactNode }) => (
      <ChromeAuthContext.Provider value={auth}>{children}</ChromeAuthContext.Provider>
    )
    expect(renderHook(() => useChromeAuth(), { wrapper }).result.current).toBe(auth)
  })
})

describe('useChromeNav', () => {
  it('has no navigator, link or top bar outside a provider', () => {
    const { result } = renderHook(() => useChromeNav())
    expect(result.current).toEqual({ navigate: undefined })
  })

  it("reads the host's navigator inside a provider", () => {
    const navigate = () => true
    const wrapper = ({ children }: { children: ReactNode }) => (
      <ChromeNavContext.Provider value={{ navigate, headerLayout: 'native' }}>
        {children}
      </ChromeNavContext.Provider>
    )
    expect(renderHook(() => useChromeNav(), { wrapper }).result.current).toEqual({
      navigate,
      headerLayout: 'native'
    })
  })
})
