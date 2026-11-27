import { createRoot } from 'react-dom/client'
import { InviteLanding } from '../../apps/sites/invite/InviteLanding'
import '../../foundations/foundations.css'
import './invite-mobile.module.css'

// `?avatar=tall` puts a box far taller than any phone in the avatar slot, as a renderer whose canvas outgrew its
// container would.
const tall = new URLSearchParams(location.search).get('avatar') === 'tall'

createRoot(document.getElementById('root')!).render(
  <InviteLanding
    referrerName="Brai"
    joinHref="#join"
    faqsHref="#faqs"
    avatar={tall ? <div style={{ height: 3000 }} /> : undefined}
  />
)
