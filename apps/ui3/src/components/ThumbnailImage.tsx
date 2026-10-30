import { useCallback, useEffect, useRef, type ComponentPropsWithoutRef } from "react";

type ThumbnailImageProps = Omit<ComponentPropsWithoutRef<"img">, "onError"> & {
  fallbackSrc: string;
};

/** A plain image with recovery for derivatives that fail before hydration. */
export default function ThumbnailImage({ fallbackSrc, ...props }: ThumbnailImageProps) {
  const image = useRef<HTMLImageElement>(null);
  const restoreOriginal = useCallback((element: HTMLImageElement) => {
    element.removeAttribute("srcset");
    if (element.getAttribute("src") !== fallbackSrc) element.src = fallbackSrc;
  }, [fallbackSrc]);
  useEffect(() => {
    const element = image.current;
    if (element?.complete && element.naturalWidth === 0) restoreOriginal(element);
  }, [props.src, props.srcSet, restoreOriginal]);
  return <img {...props} ref={image} onError={event => restoreOriginal(event.currentTarget)} />;
}
