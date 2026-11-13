import { useId } from 'react'
export function PasswordVisibilityIcon({ revealed }: { revealed: boolean }) {
  return (
    <svg width={24} height={24} viewBox="0 0 24 24" fill="none" aria-hidden="true">
      {revealed ? (
        <>
          <path
            d="M7.18643 16.7976C5.41523 15.54 4.01363 13.9042 3.22163 12.8712C3.00016 12.5861 2.87994 12.2353 2.87994 11.8743C2.87994 11.5132 3.00016 11.1625 3.22163 10.8773C4.65491 9.00629 8.09363 5.15381 12.1084 5.15381C13.9093 5.15381 15.5932 5.92853 17.0332 6.95285"
            strokeWidth="1.91917"
            strokeLinecap="round"
            strokeLinejoin="round"
            stroke="currentColor"
          />
          <path
            d="M14.1531 9.84553C13.8865 9.57449 13.5689 9.35892 13.2186 9.21125C12.8683 9.06357 12.4922 8.98673 12.112 8.98513C11.7318 8.98354 11.3551 9.05724 11.0035 9.20197C10.652 9.3467 10.3325 9.5596 10.0637 9.8284C9.79482 10.0972 9.58184 10.4166 9.43703 10.7681C9.29221 11.1196 9.21843 11.4963 9.21993 11.8765C9.22144 12.2567 9.2982 12.6328 9.44579 12.9832C9.59338 13.3335 9.80887 13.6512 10.0799 13.9179"
            strokeWidth="0.959585"
            strokeLinecap="round"
            strokeLinejoin="round"
            stroke="currentColor"
          />
          <path
            d="M4.70374 20.1601L20.0637 4.80005"
            strokeWidth="0.959585"
            strokeLinecap="round"
            strokeLinejoin="round"
            stroke="currentColor"
          />
          <path
            d="M10.1884 18.3099C10.8116 18.4954 11.4581 18.5911 12.1084 18.594C16.1231 18.594 19.5618 14.7415 20.9951 12.8705C21.2165 12.5851 21.3366 12.2341 21.3364 11.8729C21.3362 11.5117 21.2158 11.1608 20.9941 10.8756C20.4904 10.2185 19.95 9.59029 19.3756 8.99402"
            strokeWidth="1.91917"
            strokeLinecap="round"
            strokeLinejoin="round"
            stroke="currentColor"
          />
        </>
      ) : (
        <>
          <path
            d="M20.8904 11.4835C21.3428 12.0787 21.3428 12.8823 20.8904 13.4765C19.4654 15.3475 16.0465 19.2 12.055 19.2C8.0635 19.2 4.64466 15.3475 3.21966 13.4765C2.99955 13.1915 2.88007 12.8409 2.88007 12.48C2.88007 12.1191 2.99955 11.7685 3.21966 11.4835C4.64466 9.61249 8.0635 5.76001 12.055 5.76001C16.0465 5.76001 19.4654 9.61249 20.8904 11.4835V11.4835Z"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            stroke="currentColor"
          />
          <path
            d="M14.9188 12.48C14.9188 14.0706 13.6369 15.36 12.0555 15.36C10.4741 15.36 9.19214 14.0706 9.19214 12.48C9.19214 10.8894 10.4741 9.59998 12.0555 9.59998C13.6369 9.59998 14.9188 10.8894 14.9188 12.48Z"
            strokeLinecap="round"
            strokeLinejoin="round"
            stroke="currentColor"
          />
        </>
      )}
    </svg>
  )
}

const errorViews = {
  explorer: {
    size: 15,
    outlineMask: { x: '0.0405884', y: '0.767822', width: '15', height: '13' },
    lineMask: { x: '6.09018', y: '4.84686', width: '3', height: '5' },
    triangle:
      'M1.96282 12.616H13.1184C13.5223 12.616 13.772 12.1755 13.5645 11.8289L7.98673 2.51297C7.78486 2.1758 7.29632 2.1758 7.09444 2.51297L1.51667 11.8289C1.30915 12.1755 1.55884 12.616 1.96282 12.616Z',
    triangleStroke: '2.08',
    line: 'M7.60643 5.28574C7.8736 5.28574 8.09018 5.50232 8.09018 5.76949L8.09018 8.36311C8.09018 8.63028 7.8736 8.84686 7.60643 8.84686C7.33926 8.84686 7.12268 8.63028 7.12268 8.36311L7.12268 5.76949C7.12268 5.50232 7.33926 5.28574 7.60643 5.28574Z',
    lineStroke: '0.52',
    dot: 'M8.22009 10.5093C8.22009 10.1703 7.9453 9.89554 7.60634 9.89554C7.26737 9.89554 6.99259 10.1703 6.99259 10.5093C6.99259 10.8483 7.26737 11.123 7.60634 11.123C7.9453 11.123 8.22009 10.8483 8.22009 10.5093Z',
    dotStroke: '0.26'
  },
  web: {
    size: 16,
    outlineMask: { x: '0.0432129', y: '0.885681', width: '16', height: '14' },
    lineMask: { x: '6.62964', y: '4.43665', width: '3', height: '6' },
    triangle:
      'M2.29119 13.4571H13.7952C14.3132 13.4571 14.6333 12.8923 14.3672 12.448L8.61519 2.841C8.35638 2.40873 7.73005 2.40873 7.47123 2.841L1.71921 12.448C1.45316 12.8923 1.77327 13.4571 2.29119 13.4571Z',
    triangleStroke: '2.66667',
    line: 'M8.11364 5.63812C8.39862 5.63812 8.62964 5.86914 8.62964 6.15411L8.62964 8.92065C8.62964 9.20563 8.39862 9.43665 8.11364 9.43665C7.82866 9.43665 7.59764 9.20562 7.59764 8.92065L7.59764 6.15411C7.59764 5.86914 7.82866 5.63812 8.11364 5.63812Z',
    lineStroke: '0.666667',
    dot: 'M8.79608 11.21C8.79608 10.8329 8.49044 10.5273 8.11341 10.5273C7.73639 10.5273 7.43075 10.8329 7.43075 11.21C7.43075 11.587 7.73639 11.8926 8.11341 11.8926C8.49044 11.8926 8.79608 11.587 8.79608 11.21Z',
    dotStroke: '0.333333'
  }
}
export function InputErrorIcon({ surface }: { surface: 'explorer' | 'web' }) {
  const id = useId().replace(/[^\w-]/g, '')
  const view = errorViews[surface]
  return (
    <svg
      width={view.size}
      height={view.size}
      viewBox={`0 0 ${view.size} ${view.size}`}
      fill="none"
      aria-hidden="true"
    >
      <mask id={`${id}-outline`} maskUnits="userSpaceOnUse" {...view.outlineMask} fill="black">
        <rect {...view.outlineMask} fill="white" />
        <path d={view.triangle} />
      </mask>
      <path
        d={view.triangle}
        strokeWidth={view.triangleStroke}
        stroke="currentColor"
        mask={`url(#${id}-outline)`}
      />
      <use href={`#${id}-line`} fill="currentColor" />
      <mask id={`${id}-line-mask`} maskUnits="userSpaceOnUse" {...view.lineMask} fill="black">
        <rect {...view.lineMask} fill="white" />
        <use href={`#${id}-line`} />
      </mask>
      <path
        d={view.line}
        strokeWidth={view.lineStroke}
        strokeLinecap="round"
        strokeLinejoin="round"
        stroke="currentColor"
        mask={`url(#${id}-line-mask)`}
      />
      <path
        d={view.dot}
        fill="currentColor"
        strokeWidth={view.dotStroke}
        strokeLinecap="round"
        strokeLinejoin="round"
        stroke="currentColor"
      />
      <defs>
        <path id={`${id}-line`} d={view.line} />
      </defs>
    </svg>
  )
}
