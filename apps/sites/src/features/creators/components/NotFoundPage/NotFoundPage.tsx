import { useEffect } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '~/components/Button'
import { useTranslation } from '~/intl'
import { track } from '~/lib/analytics'
import styles from './NotFoundPage.module.css'

const NotFoundPage = () => {
  const { t } = useTranslation()
  useEffect(() => {
    track('Not found page')
  }, [])
  return (
    <section className={styles.panel} data-testid="not-found-page">
      <h2 className={styles.title}>{t('not_found_page.title')}</h2>
      <p className={styles.text}>{t('not_found_page.description')}</p>
      <Button as={Link} to="/" data-testid="not-found-home">
        {t('not_found_page.go_home')}
      </Button>
    </section>
  )
}

export { NotFoundPage }
