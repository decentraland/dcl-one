// Whether this HUD runs the in-world Shop: it does, unless the entry url asks for the catalogue page it
// replaced (`?shop=old`, kept while that page is retired). `?shop=new`, from before it was the default,
// still opens it. Read once, from the entry url, before the engine's url sync rewrites it.
export const IN_WORLD_SHOP = new URLSearchParams(location.search).get('shop') !== 'old'
