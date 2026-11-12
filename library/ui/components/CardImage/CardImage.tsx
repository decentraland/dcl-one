import { useEffect, useRef, useState, type ReactNode } from 'react'
import { GeneralImage } from '@dcl/one-ui/foundations'
import styles from './CardImage.module.css'
import { hasUnsafeURLCharacters } from './card.helpers'

export type CardImageProps = {
  src?: string | null
  fallbackSrc?: string
  alt: string
  priority?: boolean
  fallback?: ReactNode
  className?: string
}

function imageSource(value: string | null | undefined): string | undefined {
  if (typeof value !== 'string') return undefined
  if (hasUnsafeURLCharacters(value)) return undefined
  const source = value.trim()
  if (!source) return undefined
  if (source.startsWith('/')) return source.startsWith('//') ? undefined : source
  if (!/^https:\/\//i.test(source)) return undefined
  try {
    const url = new URL(source)
    if (url.protocol !== 'https:' || url.username || url.password) return undefined
    return url.href
  } catch {
    return undefined
  }
}

function ImageAttempt({
  src,
  fallbackSrc,
  alt,
  priority = false,
  fallback,
  className
}: CardImageProps) {
  const image = useRef<HTMLImageElement>(null)
  const [failed, setFailed] = useState<readonly string[]>([])
  const [loaded, setLoaded] = useState<string>()
  const candidates = [
    ...new Set(
      [src, fallbackSrc].filter(
        (value): value is string => typeof value === 'string' && !!value.trim()
      )
    )
  ]
  const actual = candidates.find((candidate) => !failed.includes(candidate))
  const state = !candidates.length
    ? 'empty'
    : !actual
      ? 'failed'
      : loaded === actual
        ? 'ready'
        : 'loading'
  useEffect(() => {
    const element = image.current
    if (!actual || !element?.complete) return
    // A cached SSR image may finish before hydration attaches load/error handlers.
    if (element.naturalWidth > 0) setLoaded(actual)
    else setFailed((previous) => [...new Set([...previous, actual])])
  }, [actual])
  return (
    <span
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-state={state}
      role={!actual && alt ? 'img' : undefined}
      aria-label={!actual && alt ? alt : undefined}
      aria-hidden={!actual && !alt ? true : undefined}
    >
      {actual ? (
        <img
          key={actual}
          ref={image}
          className={styles.image}
          src={actual}
          alt={alt}
          loading={priority ? 'eager' : 'lazy'}
          fetchPriority={priority ? 'high' : undefined}
          decoding="async"
          onLoad={() => setLoaded(actual)}
          onError={() => setFailed((previous) => [...new Set([...previous, actual])])}
        />
      ) : (
        (fallback ?? <GeneralImage size={32} aria-hidden="true" />)
      )}
    </span>
  )
}

export function CardImage(props: CardImageProps) {
  const src = imageSource(props.src)
  const fallbackSrc = imageSource(props.fallbackSrc)
  // Isolate normalized source pairs so late old-image events cannot poison a replacement.
  return (
    <ImageAttempt
      key={JSON.stringify([src ?? null, fallbackSrc ?? null])}
      {...props}
      src={src}
      fallbackSrc={fallbackSrc}
    />
  )
}
