// url=<DCL_FIGMA_FILE>?node-id=17277-14026
// source=components/BlogBanner/BlogBanner.tsx
// component=BlogBanner
// contract=1
import figma from 'figma'

const instance = figma.selectedInstance
const viewport = instance.getEnum('viewport', {
  'large-desktop': '',
  desktop: '',
  tablet: '',
  mobile: ''
})

export default {
  example: figma.tsx`<BlogBanner title={title} href={href} cover={cover} category={category} metadata={metadata} summary={summary} actionLabel={actionLabel}${viewport} />`,
  imports: ["import { BlogBanner } from '@dcl/one-ui'"],
  id: 'blog-banner'
}
