export type ButtonMetrics = {
  color: string
  background: string
  fontSize: string
  fontWeight: string
  opacity: string
  x: number
  y: number
  width: number
  height: number
  outline: string
  outlineColor: string
  outlineOffset: string
  shadow: string
  disabled: boolean
  busy: string | null
  focused: boolean
  hovered: boolean
  pressed: boolean
}
export type TouchFeedback = {
  metrics: ButtonMetrics
  trusted: boolean
  fromTouch: boolean
  pointerType: string
}
export function buttonMetrics(button: HTMLButtonElement): ButtonMetrics {
  const style = getComputedStyle(button),
    rect = button.getBoundingClientRect()
  return {
    color: style.color,
    background: style.backgroundColor,
    fontSize: style.fontSize,
    fontWeight: style.fontWeight,
    opacity: style.opacity,
    x: rect.x,
    y: rect.y,
    width: rect.width,
    height: rect.height,
    outline: style.outlineStyle,
    outlineColor: style.outlineColor,
    outlineOffset: style.outlineOffset,
    shadow: style.boxShadow,
    disabled: button.disabled,
    busy: button.getAttribute('aria-busy'),
    focused: document.activeElement === button,
    hovered: button.matches(':hover'),
    pressed: button.matches(':active')
  }
}
