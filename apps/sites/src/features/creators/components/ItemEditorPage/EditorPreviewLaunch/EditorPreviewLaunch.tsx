import { useState } from 'react'
import { JumpInIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { track } from '~/lib/analytics'
import { previewCollection } from '~/lib/explorer'
import styles from './EditorPreviewLaunch.module.css'
import { Button } from '~/components/Button'

type Props = {
  collectionId: string
  itemCount: number
  /** A curator reviewing the collection; their launches are tracked apart from the creators'. */
  reviewing: boolean
}

/** Preview in World from the editor's preview bar, for creators editing and curators reviewing alike. */
export function EditorPreviewLaunch({ collectionId, itemCount, reviewing }: Props) {
  const { t } = useTranslation()
  const [isLaunching, setLaunching] = useState(false)

  return (
    <Button
      className={styles.launch}
      type="button"
      variant="dark"
      disabled={itemCount === 0}
      loading={isLaunching}
      data-testid="editor-preview"
      onClick={() => {
        setLaunching(true)
        void previewCollection(collectionId)
          .then(launched =>
            reviewing
              ? track('Review Preview In World', { collectionId, itemCount, launched })
              : track('Preview In World', { collectionId, itemCount, launched, trigger: 'editor' })
          )
          .finally(() => setLaunching(false))
      }}
    >
      {t('preview_collection_button.label')}
      <JumpInIcon />
    </Button>
  )
}
