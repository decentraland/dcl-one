import { useEffect, useId, useState } from 'react'
import { Button } from '../../../components/Button/Button'
import { SocialLink, SocialLinkedin, SocialX } from '../../../foundations/icons/social'
import { blogShareLinks } from './blogModel'
import styles from './BlogShare.module.css'

export type BlogShareProps = { title: string; url: string }

type CopyState = 'idle' | 'copied' | 'failed'

const MESSAGE: Record<CopyState, string> = {
  idle: '',
  copied: 'Link copied',
  failed: 'Could not copy the link'
}

/** Share on X or LinkedIn (new tab), or copy the post's link. */
export function BlogShare({ title, url }: BlogShareProps) {
  const labelId = useId()
  const [copy, setCopy] = useState<CopyState>('idle')
  const links = blogShareLinks(title, url)

  useEffect(() => {
    if (copy === 'idle') return
    const timer = setTimeout(() => setCopy('idle'), 3000)
    return () => clearTimeout(timer)
  }, [copy])

  const copyLink = () => {
    const clipboard = typeof navigator === 'undefined' ? undefined : navigator.clipboard
    if (!clipboard) {
      setCopy('failed')
      return
    }
    clipboard.writeText(url).then(
      () => setCopy('copied'),
      () => setCopy('failed')
    )
  }

  return (
    <div className={styles.share} role="group" aria-labelledby={labelId}>
      <span className={styles.label} id={labelId}>
        Share
      </span>
      <Button
        variant="outline"
        iconOnly
        round={34}
        iconSize={16}
        href={links.x}
        target="_blank"
        rel="noopener noreferrer"
        aria-label="Share on X (opens in a new tab)"
      >
        <SocialX aria-hidden="true" />
      </Button>
      <Button
        variant="outline"
        iconOnly
        round={34}
        iconSize={16}
        href={links.linkedin}
        target="_blank"
        rel="noopener noreferrer"
        aria-label="Share on LinkedIn (opens in a new tab)"
      >
        <SocialLinkedin aria-hidden="true" />
      </Button>
      <Button
        variant="outline"
        iconOnly
        round={34}
        iconSize={16}
        aria-label="Copy link"
        onClick={copyLink}
      >
        <SocialLink aria-hidden="true" />
      </Button>
      <span className={styles.status} role="status">
        {MESSAGE[copy]}
      </span>
    </div>
  )
}
