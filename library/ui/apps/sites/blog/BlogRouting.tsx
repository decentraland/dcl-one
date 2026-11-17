import {
  createContext,
  useContext,
  type AnchorHTMLAttributes,
  type ComponentType,
  type FormHTMLAttributes,
  type ReactNode
} from 'react'

/** An in-site link: always an `<a href>`, so it works before hydration and without JavaScript. */
export type BlogLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  href: string
  /** Keeps the scroll position on a client-side navigation ("Load more"). */
  preventScrollReset?: boolean
}

/** The search form: a GET form to `action`, so it works before hydration and without JavaScript. */
export type BlogFormProps = Omit<
  FormHTMLAttributes<HTMLFormElement>,
  'action' | 'method' | 'encType'
> & {
  action: string
  method: 'get'
}

/**
 * How the blog navigates. The host app supplies its router's link and form so moving between
 * categories, searches and posts stays on the client; the defaults are plain elements.
 */
export type BlogRouting = {
  Link: ComponentType<BlogLinkProps>
  Form: ComponentType<BlogFormProps>
}

function PlainLink({ preventScrollReset, ...props }: BlogLinkProps) {
  void preventScrollReset
  return <a {...props} />
}
const PlainForm = (props: BlogFormProps) => <form {...props} />

const RoutingContext = createContext<BlogRouting>({ Link: PlainLink, Form: PlainForm })

export function BlogRoutingProvider({
  routing,
  children
}: {
  routing: BlogRouting
  children: ReactNode
}) {
  return <RoutingContext.Provider value={routing}>{children}</RoutingContext.Provider>
}

export function BlogLink(props: BlogLinkProps) {
  const { Link } = useContext(RoutingContext)
  return <Link {...props} />
}

export function BlogForm(props: BlogFormProps) {
  const { Form } = useContext(RoutingContext)
  return <Form {...props} />
}
