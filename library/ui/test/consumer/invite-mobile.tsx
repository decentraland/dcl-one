import { createRoot } from 'react-dom/client'
import { InviteLanding } from '../../apps/sites/invite/InviteLanding'
import '../../foundations/foundations.css'
import './invite-mobile.module.css'

createRoot(document.getElementById('root')!).render(
  <InviteLanding referrerName="Brai" joinHref="#join" faqsHref="#faqs" />
)
