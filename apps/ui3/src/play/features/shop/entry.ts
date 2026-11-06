// Whether this HUD runs the in-world Shop, which replaces the catalogue page while it is being built:
// only when the entry url asks for it (`?shop=new`). Read once, from the entry url, before the engine's
// url sync rewrites it.
export const IN_WORLD_SHOP = new URLSearchParams(location.search).get('shop') === 'new'
