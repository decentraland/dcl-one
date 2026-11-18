import { ChevronRightIcon } from '~/components/Icons'
import { useLatestBlogPosts } from '~/hooks/useLatestBlogPosts'
import { useTranslation } from '~/intl'
import { blogUrl } from '~/lib/blog'
import { contentfulImage } from '~/lib/contentfulImage'
import { OverviewSection, trackClick } from '~/lib/overviewAnalytics'
import { formatLongDate } from '~/lib/time'
import { useLocale } from '~/store/locale'
import { AnimatedSection } from '../AnimatedSection'
import { highlight } from '../highlight'
import { cx } from '~/lib/cx'
import overview from '../OverviewPage.module.css'
import styles from './FromTheBlog.module.css'

const FromTheBlog = () => {
  const { t, rich } = useTranslation()
  const locale = useLocale(s => s.locale)
  const { data: posts } = useLatestBlogPosts()

  // Nothing to show while loading, when the CMS fails or has no posts.
  if (!posts || posts.length === 0) return null

  return (
    <AnimatedSection section={OverviewSection.BLOG}>
      <section className={styles.section} data-testid="overview-blog">
        <h2 className={cx(overview.sectionTitle, styles.title)}>{rich('overview.blog.title', highlight)}</h2>
        <div className={styles.posts}>
          {posts.map(post => (
            <a
              className={styles.postCard}
              key={post.id}
              href={post.url}
              data-testid="overview-blog-card"
              data-place={OverviewSection.BLOG}
              data-title={post.title}
              onClick={trackClick}
            >
              <div className={styles.postImage}>
                {post.imageUrl && <img src={contentfulImage(post.imageUrl, { width: 380 })} alt="" loading="lazy" />}
              </div>
              <div className={styles.postInfo}>
                <div className={styles.postMeta}>
                  {post.categoryTitle && <span className={styles.postCategory}>{post.categoryTitle}</span>}
                  <span className={styles.postDate}>{formatLongDate(post.publishedDate, locale)}</span>
                </div>
                <h3 className={styles.postTitle}>{post.title}</h3>
              </div>
            </a>
          ))}
        </div>
        <div className={overview.centered}>
          <a
            className={overview.viewAllLink}
            href={blogUrl()}
            data-testid="overview-blog-all"
            data-place={OverviewSection.BLOG}
            data-title="view-all"
            onClick={trackClick}
          >
            {t('overview.blog.view_all')}
            <ChevronRightIcon />
          </a>
        </div>
      </section>
    </AnimatedSection>
  )
}

export { FromTheBlog }
