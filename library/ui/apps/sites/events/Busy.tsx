import { Spinner } from '../../../components/Spinner/Spinner'
import styles from './Busy.module.css'

type BusyLayout = 'inline' | 'page'

/**
 * An Events wait: the library spinner, named by its words, with the words beside it. The region is a
 * polite status, so the wait is announced as well as drawn. `page` stacks and centres it in the content
 * area; `inline` keeps it on one line where the work happens.
 */
function Busy({ layout = 'inline', children }: { layout?: BusyLayout; children: string }) {
  return (
    <p className={styles.busy} data-layout={layout} role="status">
      <Spinner variant="light" label={children} />
      <span>{children}</span>
    </p>
  )
}

export { Busy, type BusyLayout }
