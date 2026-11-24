import { AnimatedBackground } from '@dcl/one-ui'
import type { GalleryEntry } from '../registry'
import layout from './layout.module.css'

export const backgroundEntries: GalleryEntry[] = [
  {
    name: 'AnimatedBackground',
    group: 'Backgrounds',
    summary:
      'The WebGL purple vignette with the drifting logo pattern. It fills its positioned parent and shows the still image without WebGL or with reduced motion.',
    components: ['AnimatedBackground'],
    examples: [
      {
        id: 'default',
        title: 'Default pattern tiling',
        render: () => (
          <div className={layout.backdrop}>
            <AnimatedBackground />
          </div>
        )
      },
      {
        id: 'tiling',
        title: 'Larger pattern',
        note: 'patternTiling={1}, as in the Shop promotion banner.',
        render: () => (
          <div className={layout.backdrop}>
            <AnimatedBackground patternTiling={1} />
          </div>
        )
      }
    ]
  }
]
