// TODO(figma-ds): replace with the avatar column, until the Figma-generated avatar render from library/ui/components.
import { useEffect, useState } from 'react'
import styles from './ProfileAvatarBody.module.css'

type ProfileAvatarBodyProps = { imageUrl?: string; name: string }

/** The avatar's full-body snapshot, in the box where the profile site mounts its 3D preview. */
function ProfileAvatarBody({ imageUrl, name }: ProfileAvatarBodyProps) {
  const [failed, setFailed] = useState(false)
  useEffect(() => setFailed(false), [imageUrl])
  return (
    <div className={styles.render}>
      {imageUrl && !failed ? (
        <img
          className={styles.renderImage}
          src={imageUrl}
          alt={`${name}'s avatar`}
          onError={() => setFailed(true)}
        />
      ) : null}
    </div>
  )
}

export { ProfileAvatarBody }
export type { ProfileAvatarBodyProps }
