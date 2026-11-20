import Lottie from 'lottie-react'
import animationData from './confettiAnimation.json'

// Its own module so the whole of lottie-web and the animation stay out of the entry chunk; a static import
// here also lets the bundler settle lottie-react's CommonJS default export, which a dynamic import of the
// package itself hands back as a namespace object in dev.
export default function LottieBurst() {
  return <Lottie animationData={animationData} loop={1} />
}
