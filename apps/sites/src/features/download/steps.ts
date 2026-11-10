import macosBraveStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/macos-brave-step1.webp";
import macosChromeStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/macos-chrome-step1.webp";
import macosFirefoxStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/macos-firefox-step1.webp";
import macosOperaStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/macos-opera-step1.webp";
import macosSafariStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/macos-safari-step1.webp";
import macosStepTwo from "@dcl/one-ui/apps/sites/assets/download-steps/macos-step2.webp";
import stepThree from "@dcl/one-ui/apps/sites/assets/download-steps/step3.webp";
import windowsBraveStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/windows-brave-step1.webp";
import windowsChromeStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/windows-chrome-step1.webp";
import windowsEdgeStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/windows-edge-step1.webp";
import windowsFirefoxStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/windows-firefox-step1.webp";
import windowsOperaStepOne from "@dcl/one-ui/apps/sites/assets/download-steps/windows-opera-step1.webp";
import windowsStepTwo from "@dcl/one-ui/apps/sites/assets/download-steps/windows-step2.webp";

import type { DownloadOs } from "./params";

// The install-step screenshots of source Sites (pages/DownloadSuccess/DownloadSuccess.helpers.ts,
// DownloadSuccess.tsx:253-281). The highlight is the center of the browser's download button in
// each step 1 screenshot, measured on the image, in percent.

export type DownloadBrowser =
  "chrome" | "firefox" | "brave" | "opera" | "edge" | "safari";
export type StepImage = {
  image: string;
  imageFit: "cover" | "contain";
  highlight?: { x: number; y: number };
};

const MACOS_CHROME: StepImage = {
  image: macosChromeStepOne,
  imageFit: "cover",
  highlight: { x: 75.82, y: 47.81 },
};
const WINDOWS_CHROME: StepImage = {
  image: windowsChromeStepOne,
  imageFit: "cover",
  highlight: { x: 75, y: 45.42 },
};
const WINDOWS_EDGE: StepImage = {
  image: windowsEdgeStepOne,
  imageFit: "cover",
  highlight: { x: 75.63, y: 36.98 },
};

const STEP_ONE: Record<
  DownloadOs,
  Partial<Record<DownloadBrowser, StepImage>> & { chrome: StepImage }
> = {
  macos: {
    chrome: MACOS_CHROME,
    firefox: {
      image: macosFirefoxStepOne,
      imageFit: "cover",
      highlight: { x: 77.28, y: 40.21 },
    },
    brave: {
      image: macosBraveStepOne,
      imageFit: "cover",
      highlight: { x: 64.47, y: 46.15 },
    },
    opera: {
      image: macosOperaStepOne,
      imageFit: "cover",
      highlight: { x: 68.78, y: 35.21 },
    },
    safari: {
      image: macosSafariStepOne,
      imageFit: "cover",
      highlight: { x: 49.37, y: 26.25 },
    },
    // No macOS Edge capture was designed: it reuses the Windows one.
    edge: WINDOWS_EDGE,
  },
  windows: {
    chrome: WINDOWS_CHROME,
    firefox: {
      image: windowsFirefoxStepOne,
      imageFit: "cover",
      highlight: { x: 76.46, y: 41.35 },
    },
    brave: {
      image: windowsBraveStepOne,
      imageFit: "cover",
      highlight: { x: 63.64, y: 42.92 },
    },
    opera: {
      image: windowsOperaStepOne,
      imageFit: "cover",
      highlight: { x: 68.4, y: 38.75 },
    },
    edge: WINDOWS_EDGE,
  },
};

/**
 * The browser by name, never by engine: a shared engine says nothing about the toolbar. Brave hides
 * in its UA, so it is read from navigator.brave; the order matters because Edge and Opera also
 * claim Chrome and Chrome claims Safari.
 */
export function detectDownloadBrowser(
  userAgent: string,
  isBrave = false,
): DownloadBrowser {
  if (isBrave) return "brave";
  if (/Edg(e|A|iOS)?\//.test(userAgent)) return "edge";
  if (/OPR\/|Opera/.test(userAgent)) return "opera";
  if (/Firefox\/|FxiOS\//.test(userAgent)) return "firefox";
  if (/Chrome\/|CriOS\//.test(userAgent)) return "chrome";
  if (/Safari\//.test(userAgent)) return "safari";
  return "chrome";
}

export function stepImages(
  os: DownloadOs,
  browser: DownloadBrowser,
): [StepImage, StepImage, StepImage] {
  return [
    STEP_ONE[os][browser] ?? STEP_ONE[os].chrome,
    os === "windows"
      ? { image: windowsStepTwo, imageFit: "cover" }
      : { image: macosStepTwo, imageFit: "contain" },
    { image: stepThree, imageFit: "cover" },
  ];
}
