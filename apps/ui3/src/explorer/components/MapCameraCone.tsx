import { useId } from "react";
import "./mapcameracone.css";

/** Clockwise degrees from north, independently of the avatar arrow. */
export default function MapCameraCone({ heading }: { heading?: number | null }) {
  const gradient = useId();
  if (heading == null || !Number.isFinite(heading)) return null;
  return <svg className="map-camera-cone" viewBox="0 0 80 80" aria-hidden="true"
    data-camera-heading={heading} style={{ transform: `translate(-50%, -50%) rotate(${heading}deg)` }}>
    <defs><radialGradient id={gradient} cx="50%" cy="50%" r="50%">
      <stop offset="0" stopColor="#fff" stopOpacity=".42" />
      <stop offset="1" stopColor="#fff" stopOpacity="0" />
    </radialGradient></defs>
    <path d="M40 40 17 8A40 40 0 0 1 63 8Z" fill={`url(#${gradient})`} />
    <path d="M20 12 40 40 60 12" fill="none" stroke="#fff" strokeOpacity=".25" strokeWidth="1" />
  </svg>;
}
