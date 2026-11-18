import { useQuery } from '@tanstack/react-query'
import { isCommitteeMember } from '~/lib/curation'

// Same refresh cadence as builder-server's own committee cache.
const COMMITTEE_STALE_MS = 60 * 60_000

const NO_MEMBERS: string[] = []

/** Whether the signed-in wallet sits on the curation committee. Fails closed: loading or errored is not a curator. */
export function useCommittee(address: string | undefined) {
  // Signed-out visitors can't be curators, so they never ask.
  const query = useQuery({
    queryKey: ['committee'],
    // The navbar asks on every page; the builder-server client stays off the overview's first load.
    queryFn: async () => (await import('~/lib/builder')).fetchCommittee(),
    staleTime: COMMITTEE_STALE_MS,
    enabled: !!address
  })
  const members = query.data
  return {
    members: members ?? NO_MEMBERS,
    isCurator: isCommitteeMember(members, address),
    isLoading: query.isLoading
  }
}
