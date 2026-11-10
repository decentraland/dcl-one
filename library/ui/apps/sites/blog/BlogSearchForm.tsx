// TODO(figma-ds): replace with the generated SearchField component when the DCL UI library lands
import { useEffect, useId, useRef, type ChangeEvent } from 'react'
import { GeneralDownArrow, GeneralSearch } from '../../../foundations/icons/general'
import { BlogForm } from './BlogRouting'
import { BLOG_HOME, BLOG_QUERY_MAX, type BlogSort } from './blogModel'
import styles from './BlogSearchForm.module.css'

export type BlogSearchFormProps = {
  /** `bar` sits in the tabs menu; `outline` is the full-width mobile field. */
  variant: 'bar' | 'outline'
  query: string
  sort: BlogSort
  /** The category the search stays in; "" searches every post. */
  category: string
  /** Shows the sort menu; otherwise a non-default sort is carried as a hidden field. */
  showSort?: boolean
}

function submitOnChange(event: ChangeEvent<HTMLSelectElement>) {
  event.currentTarget.form?.requestSubmit()
}

/** A GET form to the blog index: works without JavaScript; the sort menu submits on change. */
export function BlogSearchForm({ variant, query, sort, category, showSort }: BlogSearchFormProps) {
  const id = useId()
  const input = useRef<HTMLInputElement>(null)
  const select = useRef<HTMLSelectElement>(null)
  // The fields are uncontrolled; a client-side navigation (a tab, the other form) changes the
  // props without remounting them, so bring them in line with the page shown.
  useEffect(() => {
    if (input.current && input.current.value !== query) input.current.value = query
  }, [query])
  useEffect(() => {
    if (select.current && select.current.value !== sort) select.current.value = sort
  }, [sort])
  return (
    <BlogForm
      className={`${styles.form} ${styles[variant]}`}
      role="search"
      action={BLOG_HOME}
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
      {category ? <input type="hidden" name="category" value={category} /> : null}
      {showSort ? (
        <span className={styles.sort}>
          <label htmlFor={`${id}sort`}>Sort:</label>
          <select
            className={styles.select}
            ref={select}
            id={`${id}sort`}
            name="sort"
            defaultValue={sort}
            onChange={submitOnChange}
          >
            <option value="newest">Newest</option>
            <option value="oldest">Oldest</option>
          </select>
          <GeneralDownArrow className={styles.chevron} size={20} aria-hidden="true" />
        </span>
      ) : sort !== 'newest' ? (
        <input type="hidden" name="sort" value={sort} />
      ) : null}
    </BlogForm>
  )
}
