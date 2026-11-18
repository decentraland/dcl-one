import { useEffect } from 'react'
import { useQueryClient, type QueryClient } from '@tanstack/react-query'
import { createCollectionForumPost, createCurationForumReply, fetchCollection } from '~/lib/builder'
import { type Collection } from '~/lib/collections'
import {
  markForumPostRecovered,
  postAssigneeToForum,
  postCollectionToForum,
  shouldRecoverForumPost,
  type ForumPostSource
} from '~/lib/forumPost'
import { type Item } from '~/lib/items'
import { profileQuery } from '~/hooks/useProfile'

/** Opens the collection's topic, writing its link into the cached collection. */
export function postToForum(
  queryClient: QueryClient,
  address: string,
  collection: Collection,
  source: ForumPostSource
): Promise<void> {
  const key = ['collection', address, collection.id]
  return postCollectionToForum(collection, source, {
    createPost: id => createCollectionForumPost(address, id),
    fetchForumLink: async id => (await fetchCollection(address, id)).forumLink,
    exclusive: async (lockKey, task) => {
      if (navigator.locks) await navigator.locks.request(lockKey, task)
      else await task()
    },
    onPosted: forumLink => {
      if (forumLink) queryClient.setQueryData<Collection>(key, current => current && { ...current, forumLink })
      else void queryClient.invalidateQueries({ queryKey: key })
      // Lists show the link too (curation's Discussion column).
      void queryClient.invalidateQueries({ queryKey: ['curation-collections'] })
      void queryClient.invalidateQueries({ queryKey: ['collections'] })
    }
  })
}

/** Replies on the collection's topic with its new assignee. */
export function postAssignee(
  queryClient: QueryClient,
  address: string,
  collection: Collection,
  assignee: string | null
): Promise<void> {
  return postAssigneeToForum(collection, assignee, {
    claimedName: async curator => {
      const profile = await queryClient.fetchQuery(profileQuery(curator))
      return profile?.hasClaimedName ? profile.name : undefined
    },
    createReply: (collectionId, raw) => createCurationForumReply(address, collectionId, { raw })
  })
}

/** Creates the topic the publishing tab never did (see `shouldRecoverForumPost`). */
export function useForumPostRecovery(address: string | undefined, collection: Collection | undefined, items: Item[]) {
  const queryClient = useQueryClient()
  const needsPost = !!collection && shouldRecoverForumPost(collection, items, address)

  useEffect(() => {
    if (!needsPost || !address || !collection) return
    markForumPostRecovered(collection.id)
    void postToForum(queryClient, address, collection, 'recovery')
    // Keyed on the id: a refetched collection object must not post twice.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [needsPost, address, collection?.id, queryClient])
}
