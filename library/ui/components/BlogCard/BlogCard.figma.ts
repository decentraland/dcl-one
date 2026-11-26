// url=<DCL_FIGMA_FILE>?node-id=17277-14023
// source=components/BlogCard/BlogCard.tsx
// component=BlogCard
// contract=1
// states=hover
import figma from 'figma'

const instance = figma.selectedInstance
const viewport = instance.getEnum('viewport', { desktop: '', mobile: '' })
const state = instance.getEnum('state', { default: '', hover: '' })

export default {
  example: figma.tsx`<BlogCard title={title} href={href} cover={cover} category={category} metadata={metadata} author={author}${viewport}${state} />`,
  imports: ["import { BlogCard } from '@dcl/one-ui'"],
  id: 'blog-card'
}
