import { lazy, Suspense } from 'react'
import { Hero } from './Hero'

// The hero paints first; the rest of the landing streams in behind it as one chunk.
const Why = lazy(() => import('./Why').then(m => ({ default: m.Why })))
const CreateCards = lazy(() => import('./CreateCards').then(m => ({ default: m.CreateCards })))
const LiveScenes = lazy(() => import('./LiveScenes').then(m => ({ default: m.LiveScenes })))
const Connect = lazy(() => import('./Connect').then(m => ({ default: m.Connect })))
const Learn = lazy(() => import('./Learn').then(m => ({ default: m.Learn })))
const FromTheBlog = lazy(() => import('./FromTheBlog').then(m => ({ default: m.FromTheBlog })))
const Faqs = lazy(() => import('./Faqs').then(m => ({ default: m.Faqs })))

const OverviewPage = () => {
  return (
    <div data-testid="overview-page">
      <Hero />
      <Suspense fallback={null}>
        <Why />
        <CreateCards />
        <LiveScenes />
        <Connect />
        <Learn />
        <FromTheBlog />
        <Faqs />
      </Suspense>
    </div>
  )
}

export { OverviewPage }
