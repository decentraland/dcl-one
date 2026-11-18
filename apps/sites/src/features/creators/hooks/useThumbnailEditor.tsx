import { useEffect, useRef, useState, type ReactNode } from 'react'
import {
  ThumbnailModal,
  getThumbnailErrorMessage,
  thumbnailPatchFromFile,
  type ThumbnailPatch
} from '~/components/ThumbnailModal'
import { useItemContents } from '~/hooks/usePublishCollection'
import { useTranslation } from '~/intl'
import { pickFile } from '~/lib/filePicker'
import { ItemType, type Item } from '~/lib/items'
import { useNotifications } from '~/lib/notifications'
import { isImageWearableContents } from '~/lib/wearableCategories'

/** What gets a new thumbnail: a saved item (its files load when the modal opens) or an in-memory draft. */
export type ThumbnailSubject =
  { kind: 'item'; item: Item } | { kind: 'files'; id: string; type: ItemType | null; contents: Record<string, Blob> }

function subjectInfo(subject: ThumbnailSubject) {
  return subject.kind === 'item'
    ? { type: subject.item.type, contents: subject.item.contents }
    : { type: subject.type, contents: subject.contents }
}

/** Texture-only wearables have no 3D model to pose, so they take a PNG straight from disk. */
function isTextureOnlySubject(subject: ThumbnailSubject): boolean {
  const { type, contents } = subjectInfo(subject)
  return type !== ItemType.EMOTE && isImageWearableContents(contents)
}

/**
 * The one way to change an item's thumbnail: `edit(subject)` opens the file picker for texture-only
 * wearables and the posing modal for everything else; render `modal` once. A rejected `onSave` keeps the modal open
 * with its error (a toast for a picked texture).
 */
export function useThumbnailEditor(
  onSave: (patch: ThumbnailPatch, subject: ThumbnailSubject) => void | Promise<void>
): { edit: (subject: ThumbnailSubject) => void; isOpen: boolean; modal: ReactNode } {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const [open, setOpen] = useState<ThumbnailSubject | null>(null)
  // Saves land after async work (resize, a modal session), so they go to the caller's latest callback.
  const onSaveRef = useRef(onSave)
  useEffect(() => {
    onSaveRef.current = onSave
  })
  const itemContents = useItemContents(open?.kind === 'item' ? open.item : null)

  async function pickTexture(subject: ThumbnailSubject) {
    const file = await pickFile({ accept: 'image/png' })
    if (!file) return
    let patch: ThumbnailPatch
    try {
      patch = await thumbnailPatchFromFile(subject.kind === 'files' ? subject.contents : {}, file)
    } catch (error) {
      const { key, params } = getThumbnailErrorMessage(error)
      showToast(t(key, params), { type: 'error' })
      return
    }
    try {
      await onSaveRef.current(patch, subject)
    } catch (error) {
      const { key, params } = getThumbnailErrorMessage(error)
      showToast(t(key, params), { type: 'error' })
    }
  }

  function edit(subject: ThumbnailSubject) {
    if (isTextureOnlySubject(subject)) void pickTexture(subject)
    else setOpen(subject)
  }

  const modal = open && (
    <ThumbnailModal
      type={subjectInfo(open).type}
      contents={open.kind === 'files' ? open.contents : (itemContents.data ?? null)}
      loadError={open.kind === 'item' && itemContents.isError}
      onClose={() => setOpen(null)}
      onSave={async patch => {
        await onSaveRef.current(patch, open)
        setOpen(null)
      }}
    />
  )

  return { edit, isOpen: open !== null, modal }
}
