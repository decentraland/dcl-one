// from: decentraland-ui2 Logo (the sites report cards' 64 px mark); gradient ids are unique per instance, as upstream's are.
import { useId } from "react";

// [path, fill]: a number names one of GRADIENTS.
const SHAPES: [string, string | number][] = [
  ["M45 89.9999C69.8528 89.9999 89.9999 69.8528 89.9999 45C89.9999 20.1472 69.8528 0 45 0C20.1472 0 0 20.1472 0 45C0 69.8528 20.1472 89.9999 45 89.9999Z", 0],
  ["M18 80.9996C25.515 86.6471 34.875 89.9996 45 89.9996C55.125 89.9996 64.485 86.6471 72 80.9996H18Z", "#FF2D55"],
  ["M9.00008 71.9996C11.5651 75.3971 14.6026 78.4346 18.0001 80.9996H72C75.3975 78.4346 78.435 75.3971 81 71.9996H9.00008Z", "#FFA25A"],
  ["M60.3675 62.9997H3.75757C5.15257 66.2172 6.93007 69.2322 9.00006 71.9997H60.39V62.9997H60.3675Z", "#FFC95B"],
  ["M31.8826 29.2496V62.9996H60.0075L31.8826 29.2496Z", 1],
  ["M3.75757 62.9997H31.8826V29.2497L3.75757 62.9997Z", "white"],
  ["M60.3675 47.25V72H81L60.3675 47.25Z", 2],
  ["M39.7574 72H60.3674V47.25L39.7574 72Z", "white"],
  ["M60.3675 40.4997C66.5807 40.4997 71.6175 35.4629 71.6175 29.2497C71.6175 23.0365 66.5807 17.9997 60.3675 17.9997C54.1543 17.9997 49.1175 23.0365 49.1175 29.2497C49.1175 35.4629 54.1543 40.4997 60.3675 40.4997Z", "#FFC95B"],
  ["M31.8824 22.4997C34.989 22.4997 37.5074 19.9813 37.5074 16.8747C37.5074 13.7681 34.989 11.2497 31.8824 11.2497C28.7758 11.2497 26.2574 13.7681 26.2574 16.8747C26.2574 19.9813 28.7758 22.4997 31.8824 22.4997Z", "#FFC95B"],
];

// [x1, y1, x2, y2, start colour, end colour]
const GRADIENTS: [string, string, string, string, string, string][] = [
  ["45", "-18.6396", "-18.6396", "45", "#FF2D55", "#FFBC5B"],
  ["31.8731", "29.2497", "31.8731", "62.9996", "#A524B3", "#FF2D55"],
  ["60.3605", "47.25", "60.3605", "72", "#A524B3", "#FF2D55"],
];

export default function DclLogo({ className }: { className?: string }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg className={className} width="64" height="64" viewBox="0 0 90 90" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      {SHAPES.map(([d, fill], i) => (
        <path key={i} fillRule="evenodd" clipRule="evenodd" d={d} fill={typeof fill === "number" ? `url(#${id}-${fill})` : fill} />
      ))}
      <defs>
        {GRADIENTS.map(([x1, y1, x2, y2, from, to], i) => (
          <linearGradient key={i} id={`${id}-${i}`} x1={x1} y1={y1} x2={x2} y2={y2} gradientUnits="userSpaceOnUse">
            <stop stopColor={from} />
            <stop offset="1" stopColor={to} />
          </linearGradient>
        ))}
      </defs>
    </svg>
  );
}
