// Deterministic example data. Every picture is a raster file shipped with the gallery (assets/NOTICE), the kind and
// size of picture the apps pass, so the canvas makes no request outside the gallery.
import avatarFace from './assets/avatar-face.webp'
import careerMondays from './assets/career-mondays.webp'
import creatorFace from './assets/creator-face.webp'
import genesisPlaza from './assets/genesis-plaza.webp'
import gloves from './assets/gloves.webp'
import skaterFace from './assets/skater-face.webp'
import theatre from './assets/theatre.webp'
import unicycle from './assets/unicycle.webp'
import watchParty from './assets/watch-party.webp'

/**
 * A gallery picture as a root-relative URL. The components load pictures only from https or root-relative URLs
 * (`imageSource` in link.helpers.ts), and the gallery is served over http in its build check and under any prefix,
 * so the URL the bundler gives is resolved against the page and kept from its path on.
 */
export function picture(url: string): string {
  return new URL(url, document.baseURI).pathname
}

export const covers = {
  plaza: picture(genesisPlaza),
  festival: picture(careerMondays),
  gallery: picture(watchParty),
  wearable: picture(gloves),
  emote: picture(unicycle),
  blog: picture(theatre)
}

/** The face a profile shows before it has its own. */
export const placeholderFace = picture(creatorFace)

export const people = {
  ada: { name: 'Ada', picture: picture(avatarFace), color: '#ff7439' },
  grace: { name: 'Grace', picture: picture(skaterFace), color: '#5ab7ff' }
}
