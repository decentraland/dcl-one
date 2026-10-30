import { DCL_LOGOMARK_PATHS } from "./dclLogomarkPaths";
type DclLogomarkProps = {
  size?: number;
  className?: string;
};

export default function DclLogomark({ size = 40, className = "" }: DclLogomarkProps) {
  return (
    <svg viewBox="0 0 54 54" width={size} height={size} fill="currentColor" aria-hidden="true" className={className}>
      {DCL_LOGOMARK_PATHS.map((d) => <path key={d} d={d} />)}
    </svg>
  );
}
