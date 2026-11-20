import type { SVGProps } from 'react'

/** Play badge over video thumbnails; the glyph sits inside a 120 box so it renders at two thirds of the icon size. */
export function PlayIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg width="120" height="120" viewBox="0 0 120 120" fill="currentColor" aria-hidden focusable="false" {...props}>
      <path d="M60 16C37.9167 16 20 33.9167 20 56C20 78.0833 37.9167 96 60 96C82.0833 96 100 78.0833 100 56C100 33.9167 82.0833 16 60 16ZM76.9983 59.4183L53.665 73.0833C53.0009 73.334 52.3336 73.5 51.6663 73.5C50.9991 73.5 50.3317 73.4154 49.749 72.9987C48.5837 72.3314 47.8317 70.9154 47.8317 69.5807V42.3349C47.8317 40.8342 48.5804 39.5842 49.749 38.9169C50.9143 38.2496 52.4997 38.2496 53.665 38.9169L76.9983 52.5819C78.1637 53.3306 78.9157 54.4992 78.9157 55.9999C78.9157 57.5005 78.167 58.751 76.9983 59.4183Z" />
    </svg>
  )
}
