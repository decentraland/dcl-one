// TODO(figma-ds): replace with the Figma-generated avatar render; the source shows a 3D preview here.
import { useBrokenImage } from '../useBrokenImage'
import styles from './ProfileAvatarBody.module.css'

type ProfileAvatarBodyProps = { imageUrl?: string; name: string }

/** The avatar's full-body snapshot, as the source shows when its 3D preview cannot load. */
function ProfileAvatarBody({ imageUrl, name }: ProfileAvatarBodyProps) {
  const { broken, ref, onError } = useBrokenImage(imageUrl)
  return (
    <div className={styles.root}>
      {imageUrl && !broken ? (
        <img
          ref={ref}
          className={styles.image}
          src={imageUrl}
          alt={`${name}'s avatar`}
          onError={onError}
        />
      ) : null}
    </div>
  )
}

export { ProfileAvatarBody }
export type { ProfileAvatarBodyProps }
