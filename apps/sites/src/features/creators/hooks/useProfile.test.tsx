import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type ReactNode } from 'react'
import { profileQuery, useProfile, useProfiles, type ProfileAvatar } from './useProfile'

const WITH_PROFILE = '0x00000000000000000000000000000000000000aa'
const WITHOUT_PROFILE = '0x00000000000000000000000000000000000000bb'
const avatar: ProfileAvatar = { name: 'creator', hasClaimedName: true, ethAddress: WITH_PROFILE }

let client: QueryClient
let fetchMock: ReturnType<typeof vi.fn>

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  fetchMock = vi.fn((url: string) =>
    Promise.resolve(
      url.endsWith(WITH_PROFILE)
        ? new Response(JSON.stringify({ avatars: [avatar] }), { status: 200 })
        : new Response('{"error":"Not found"}', { status: 404 })
    )
  )
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('when the wallet has a profile', () => {
  it('should return its first avatar', async () => {
    const { result } = renderHook(() => useProfile(WITH_PROFILE), { wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(avatar)
  })
})

describe('when the wallet has no profile', () => {
  it('should succeed with no data instead of failing the query', async () => {
    const { result } = renderHook(() => useProfile(WITHOUT_PROFILE), { wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toBeUndefined()
  })

  it('should resolve to null when fetched directly, so a caller awaiting it does not reject', async () => {
    await expect(client.fetchQuery(profileQuery(WITHOUT_PROFILE))).resolves.toBeNull()
  })
})

describe('when the Catalyst fails to answer', () => {
  beforeEach(() => {
    fetchMock.mockResolvedValue(new Response('Service Unavailable', { status: 503 }))
  })

  it('should fail the query instead of caching the wallet as one without a profile', async () => {
    const { result } = renderHook(() => useProfile(WITH_PROFILE), { wrapper })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.data).toBeUndefined()
  })
})

describe('when the profile has no avatars', () => {
  beforeEach(() => {
    fetchMock.mockResolvedValue(new Response(JSON.stringify({ avatars: [] }), { status: 200 }))
  })

  it('should succeed with no data', async () => {
    const { result } = renderHook(() => useProfile(WITH_PROFILE), { wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toBeUndefined()
  })
})

describe('when several wallets are looked up and only some have a profile', () => {
  it('should return the avatar for each wallet that has one and undefined for the rest', async () => {
    const { result } = renderHook(() => useProfiles([WITH_PROFILE, WITHOUT_PROFILE]), { wrapper })

    await waitFor(() => expect(result.current[0]).toEqual(avatar))
    expect(result.current[1]).toBeUndefined()
  })
})
