import { createRoot } from 'react-dom/client'
import { ProfilePic, Tag } from '@dcl/one-ui'
import '@dcl/one-ui/foundations.css'

// A 1x1 white picture, so a loaded picture is measured without the network.
const picture =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='

createRoot(document.getElementById('root')!).render(
  <main
    style={{
      display: 'flex',
      flexDirection: 'column',
      gap: 16,
      padding: 16,
      background: '#161518'
    }}
  >
    <ProfilePic
      id="pic-56"
      name="Jane Doe"
      alt=""
      src={picture}
      size={56}
      ring={false}
      fallback="gradient"
    />
    <ProfilePic id="pic-56-initials" name="Jane Doe" size={56} ring={false} fallback="gradient" />
    <ProfilePic id="pic-28" name="Jane Doe" alt="" src={picture} size={28} fallback="gradient" />
    <ProfilePic id="pic-28-blank" name="Jane Doe" alt="" size={28} fallback="gradient" />
    <ProfilePic id="pic-72-default" name="Jane Doe" src={picture} />
    <div id="column" style={{ display: 'flex', flexDirection: 'column', width: 320 }}>
      <Tag id="tag-md" variant="category" tone="updates">
        Announcements
      </Tag>
    </div>
    <Tag id="tag-communities" variant="category" tone="communities">
      Community Highlights
    </Tag>
    <Tag id="tag-about" variant="category">
      About Decentraland
    </Tag>
    <Tag id="tag-lg" variant="category" size="lg" tone="ruby">
      Announcements
    </Tag>
  </main>
)
