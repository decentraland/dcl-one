// What an item page's discover() works out when the reference names the item's collection and creator as
// text rather than linking them: pure parsing, so node --test covers it without a browser.

const ADDRESS = "0x[0-9a-fA-F]{40}";

/** The collection page of the item at `pathname` (`/shop/item/<contract>/<id>`): its contract's page, or null. */
export function collectionOfItem(pathname: string): string | null {
  const contract = new RegExp(`^/shop/item/(${ADDRESS})/`).exec(pathname)?.[1];
  return contract ? `/shop/collection/${contract.toLowerCase()}` : null;
}

/** The creator's items page for a profile link (`…/profile/<address>…`), or null. */
export function creatorOfProfile(href: string): string | null {
  const address = new RegExp(`/profile/(${ADDRESS})(?![0-9a-fA-F])`).exec(href)?.[1];
  return address ? `/shop/items/creator/${address.toLowerCase()}` : null;
}
