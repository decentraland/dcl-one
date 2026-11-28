import type { ComponentProps } from 'react'
import type { Item } from '@/data/lib/creators/items'
import { useItemFileSrc } from '../../hooks/useItemFileSrc'
import { ItemThumbnail } from './ItemThumbnail'

type StoredFile = {
  item: Pick<Item, 'isPublished'>
  /** The stored file's hash; none shows the bare frame. */
  hash?: string | null
}

/** An `ItemThumbnail` of an item's stored file, read signed while the item is a draft. */
export function ItemFileThumbnail({ item, hash, ...props }: StoredFile & Omit<ComponentProps<typeof ItemThumbnail>, 'src'>) {
  return <ItemThumbnail {...props} src={useItemFileSrc(item, hash)} />
}

/** A bare image of an item's stored file, read signed while the item is a draft; nothing until it loads. */
export function ItemFileImage({ item, hash, className }: StoredFile & { className?: string }) {
  const src = useItemFileSrc(item, hash)
  return src ? <img className={className} src={src} alt="" /> : null
}
