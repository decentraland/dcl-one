import { useQuery } from '@tanstack/react-query'
import { fetchLatestBlogPosts } from '~/lib/blog'

/** The newest blog posts; editorial cadence, so one fetch per visit. */
export function useLatestBlogPosts() {
  return useQuery({
    queryKey: ['latest-blog-posts'],
    queryFn: fetchLatestBlogPosts,
    staleTime: 30 * 60_000,
    retry: 1,
    meta: { reportOnlyHttpErrors: true }
  })
}
