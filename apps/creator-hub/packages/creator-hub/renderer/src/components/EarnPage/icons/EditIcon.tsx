import type { CommonProps } from '../../Icons/types';

const PATH =
  'M18.9429 7.5C19.4143 7.50007 19.8409 7.67653 20.177 7.95654L20.3162 8.08228L21.9141 9.67651C22.2674 10.0288 22.5 10.5102 22.5 11.051C22.4999 11.5851 22.2723 12.0598 21.9263 12.4109L21.9275 12.4121L19.8096 14.5862L19.7961 14.5728L11.9226 22.5H7.5V18.0896L7.86377 17.7246L16.377 9.15405L16.4514 9.22852L17.5562 8.0957L17.5696 8.08228C17.923 7.72994 18.4041 7.5 18.9429 7.5ZM19.4336 8.96729C19.1429 8.67747 18.7428 8.67754 18.4521 8.96729L17.2168 10.2356L19.7961 12.8076L21.0315 11.5393C21.3222 11.2495 21.3222 10.8514 21.0315 10.5615L19.4336 8.96729ZM8.75 21.25H11.4026L19.0332 13.5693L16.3806 10.9241L8.75 18.6047V21.25Z';

/** Figma's EARN glyph, coloured by CSS `color`. */
export function EditIcon({ className }: CommonProps) {
  return (
    <svg
      className={className}
      width="30"
      height="30"
      viewBox="0 0 30 30"
      fill="none"
      aria-hidden
      focusable="false"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path
        d={PATH}
        fill="currentColor"
      />
    </svg>
  );
}
