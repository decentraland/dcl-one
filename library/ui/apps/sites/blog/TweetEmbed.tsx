import { useEffect, useRef, useState } from 'react'

// An X post, rendered by X's own widgets script, as the source does (components/Blog/RichText/
// TwitterEmbed.tsx): the script is loaded once per page, on the first embed that needs it.
const WIDGETS_SRC = 'https://platform.twitter.com/widgets.js'

type Widgets = {
  createTweet: (
    id: string,
    element: HTMLElement,
    options?: { theme?: 'light' | 'dark' }
  ) => Promise<HTMLElement | undefined>
}
type TwitterGlobal = { widgets?: Widgets }

let loading: Promise<void> | null = null

function loadWidgets(): Promise<void> {
  // The script may already be there (another embed, or the page loaded it): no second load.
  if ((window as unknown as { twttr?: TwitterGlobal }).twttr?.widgets) return Promise.resolve()
  if (loading) return loading
  loading = new Promise((resolve, reject) => {
    const script = document.createElement('script')
    script.src = WIDGETS_SRC
    script.async = true
    script.onload = () => resolve()
    script.onerror = () => {
      loading = null
      reject(new Error('Failed to load the X widgets script'))
    }
    document.head.appendChild(script)
  })
  return loading
}

export function TweetEmbed({ tweetId, className }: { tweetId: string; className?: string }) {
  const host = useRef<HTMLSpanElement>(null)
  const [pending, setPending] = useState(true)

  useEffect(() => {
    let cancelled = false
    loadWidgets()
      .then(() => {
        const element = host.current
        const widgets = (window as unknown as { twttr?: TwitterGlobal }).twttr?.widgets
        if (cancelled || !element || !widgets) return
        return widgets.createTweet(tweetId, element, { theme: 'dark' })
      })
      .catch(() => undefined)
      .finally(() => {
        if (!cancelled) setPending(false)
      })
    return () => {
      cancelled = true
    }
  }, [tweetId])

  // Phrasing elements only: the embed sits inside the post's <p> (as source renders it), and a <div>
  // there would end the paragraph in the HTML parser and break hydration. React never renders into
  // the host, which X's script fills, so the loading text is a sibling React owns.
  return (
    <span className={className} data-tweet-id={tweetId}>
      {pending ? <span aria-busy="true">Loading tweet…</span> : null}
      <span ref={host} />
    </span>
  )
}
