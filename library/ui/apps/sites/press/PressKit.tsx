import { Button } from '../../../components/Button/Button'
import { textClasses } from '../../../text-styles'
import decentralandLogo from '../assets/dcl-logo.svg'
import styles from './PressKit.module.css'

type PressKitProps = {
  title: string
  description: string
  contactEmail: string
  downloadLabel: string
  downloadHref: string
}

export function PressKit({
  title,
  description,
  contactEmail,
  downloadLabel,
  downloadHref
}: PressKitProps) {
  return (
    <main className={styles.root}>
      <img className={styles.logo} src={decentralandLogo} alt="" width={48} height={48} />
      <h1 className={`${textClasses.headingLg} ${styles.title}`}>{title}</h1>
      <p className={`${textClasses.bodyLg} ${styles.description}`}>
        {description}
        <a className={styles.contact} href={`mailto:${contactEmail}`}>
          {contactEmail}
        </a>
        .
      </p>
      <Button size="md" href={downloadHref} target="_blank" rel="noopener noreferrer">
        {downloadLabel}
      </Button>
    </main>
  )
}
