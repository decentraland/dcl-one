// Text whose web addresses open in the browser.

import styles from './MapPanel.module.css'

const URL = /(https?:\/\/[^\s<>"']+)/g

export function TextWithLinks({ text }: { text: string }): React.JSX.Element {
  const parts = text.split(URL)
  return (
    <>
      {parts.map((part, i) =>
        i % 2 === 1 ? (
          <a key={i} className={styles.link} href={part} target="_blank" rel="noopener noreferrer">
            {part}
          </a>
        ) : (
          part
        )
      )}
    </>
  )
}
