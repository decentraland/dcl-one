// The minimap panel's and its menu's glyphs, drawn here: the collapse button's double chevron,
// the outlined location pin and the menu's link mark. All take `currentColor`.

/** Points up; the collapse button turns it over while the map is collapsed. */
export function DoubleChevron(): React.JSX.Element {
  return (
    <svg viewBox="0 0 15 19" width="15" height="19" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M4.25 7.25 7.2 4.25l3.05 3" />
      <path d="M4.25 14.25 7.2 11.25l3.05 3" />
    </svg>
  )
}

export function PinOutline(): React.JSX.Element {
  return (
    <svg viewBox="0 0 11.4053 14" width="11.4" height="14" fill="currentColor" fillRule="evenodd" clipRule="evenodd" aria-hidden="true">
      <path d="M9.65045 10.0986C8.7703 11.4282 7.75466 12.6557 7.18241 13.3195C6.40016 14.2268 5.00511 14.2268 4.22285 13.3195C3.65061 12.6557 2.63497 11.4282 1.75481 10.0986C1.31511 9.43438 0.890884 8.71733 0.572279 8.01254C0.264559 7.33184 0 6.54414 0 5.77163C0 2.60426 2.5331 0 5.70263 0C8.87216 0 11.4053 2.60426 11.4053 5.77163C11.4053 6.54414 11.1407 7.33184 10.833 8.01254C10.5144 8.71733 10.0902 9.43438 9.65045 10.0986ZM6.1553 12.434C5.91402 12.7138 5.49124 12.7138 5.24997 12.434C4.13684 11.1428 1.35612 7.72215 1.35612 5.77163C1.35612 3.33301 3.30212 1.35612 5.70263 1.35612C8.10315 1.35612 10.0491 3.33301 10.0491 5.77163C10.0491 7.72215 7.26842 11.1428 6.1553 12.434Z" />
      <path d="M5.70175 7.33219C6.55046 7.33219 7.23846 6.63326 7.23846 5.77109C7.23846 4.90891 6.55046 4.20999 5.70175 4.20999C4.85305 4.20999 4.16505 4.90891 4.16505 5.77109C4.16505 6.63326 4.85305 7.33219 5.70175 7.33219Z" />
    </svg>
  )
}

export function LinkMark(): React.JSX.Element {
  return (
    <svg viewBox="1 1 18 18" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" aria-hidden="true">
      <path d="M8.6 11.4a3.7 3.7 0 0 0 5.2 0l3-3a3.7 3.7 0 0 0-5.2-5.2l-1.1 1.1" />
      <path d="M11.4 8.6a3.7 3.7 0 0 0-5.2 0l-3 3a3.7 3.7 0 0 0 5.2 5.2l1.1-1.1" />
    </svg>
  )
}
