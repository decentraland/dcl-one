// TODO(figma-ds): replace with the generated SearchField component when the DCL UI library lands
import { useEffect, useId, useRef } from 'react'
import { GeneralSearch } from '../../../foundations/icons/general'
import { BlogForm } from './BlogRouting'
import { BLOG_QUERY_MAX, BLOG_SEARCH } from './blogModel'
import styles from './BlogSearchForm.module.css'

export type BlogSearchFormProps = {
  /** `bar` sits in the tabs menu; `outline` is the full-width mobile field. */
  variant: 'bar' | 'outline'
  query: string
}

/** A GET form to the blog search: works without JavaScript. */
export function BlogSearchForm({ variant, query }: BlogSearchFormProps) {
  const id = useId()
  const input = useRef<HTMLInputElement>(null)
  // The field is uncontrolled; a client-side navigation (a tab, the other form) changes the
  // props without remounting it, so bring it in line with the page shown.
  useEffect(() => {
    if (input.current && input.current.value !== query) input.current.value = query
  }, [query])
  return (
    <BlogForm
      className={`${styles.form} ${styles[variant]}`}
      role="search"
      action={BLOG_SEARCH}
      method="get"
    >
      <label className={styles.srOnly} htmlFor={`${id}q`}>
        Search articles
      </label>
      <span className={styles.field}>
        <GeneralSearch className={styles.icon} size={20} aria-hidden="true" />
        <input
          className={styles.input}
          ref={input}
          id={`${id}q`}
          type="search"
          name="q"
          defaultValue={query}
          placeholder="Search articles..."
          maxLength={BLOG_QUERY_MAX}
          autoComplete="off"
        />
      </span>
    </BlogForm>
  )
}
