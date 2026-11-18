import { useTranslation } from '~/intl'
import shared from '~/styles/shared.module.css'

/** Shown in place of every route while the `maintenance` flag is on. */
const MaintenancePage = () => {
  const { t } = useTranslation()
  return (
    <section className={shared.panel} data-testid="maintenance-page">
      <h2 className={shared.panelTitle}>{t('maintenance_page.title')}</h2>
      <p className={shared.panelText}>{t('maintenance_page.description')}</p>
    </section>
  )
}

export { MaintenancePage }
