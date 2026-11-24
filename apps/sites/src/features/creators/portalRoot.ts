const ID = 'creators-overlays'

/**
 * Where the creators overlays (dialogs, menus, option lists) are drawn: one element on `<body>`, outside the
 * page, that carries the creators base styles the page's own root has.
 */
export function portalRoot(): HTMLElement {
  let root = document.getElementById(ID)
  if (!root) {
    root = document.createElement('div')
    root.id = ID
    root.setAttribute('data-creators', 'overlays')
    document.body.appendChild(root)
  }
  return root
}
