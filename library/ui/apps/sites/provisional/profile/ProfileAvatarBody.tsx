// TODO(figma-ds): replace with the Figma-generated avatar render; the source shows a 3D preview here.
import { useEffect, useState } from 'react'
import styles from './ProfileAvatarBody.module.css'

type ProfileAvatarBodyProps = { imageUrl?: string; name: string }

/** The avatar's full-body snapshot, as the source shows when its 3D preview cannot load. */
function ProfileAvatarBody({ imageUrl, name }: ProfileAvatarBodyProps) {
  const [failed, setFailed] = useState(false)
  useEffect(() => setFailed(false), [imageUrl])
  return (
    <div className={styles.root}>
      {imageUrl && !failed ? (
        <img className={styles.image} src={imageUrl} alt={`${name}'s avatar`} onError={() => setFailed(true)} />
      ) : null}
    </div>
  )
}

export { ProfileAvatarBody }
export type { ProfileAvatarBodyProps }
