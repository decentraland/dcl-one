/** Joins the truthy class names, so a component can merge its module class with a consumer's `className`. */
export function cx(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(' ')
}
