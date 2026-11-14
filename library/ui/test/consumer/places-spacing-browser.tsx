import { createRoot } from 'react-dom/client'
import { NavBar } from '../../components/NavBar/NavBar'
import { ExploreBand } from '../../apps/sites/places/ExploreBand'
import { PlacesPage } from '../../apps/sites/places/PlacesPage'
import '../../foundations/foundations.css'

const live = new URLSearchParams(location.search).has('live')

createRoot(document.getElementById('root')!).render(
  <>
    <NavBar sections={[{ label: 'Places', href: '/places' }]} signIn={{ href: '/sign-in' }} />
    <PlacesPage
      title="Places"
      live={
        live ? (
          <div data-live style={{ height: 100 }}>
            Live section fixture
          </div>
        ) : null
      }
    >
      <ExploreBand
        section="all"
        onSection={() => {}}
        search=""
        onSearch={() => {}}
        category="all"
        categories={[]}
        onCategory={() => {}}
        allCategory="all"
        labels={{
          sections: { all: 'Explore all', favourites: 'Favourites', my: 'My places' },
          sectionsGroup: 'Places sections',
          search: 'Search places',
          searchPlaceholder: 'Search places',
          category: 'Category',
          filters: 'Filters',
          closeFilters: 'Close filters',
          removeFilter: (value) => value
        }}
      >
        <div>Places results fixture</div>
      </ExploreBand>
    </PlacesPage>
  </>
)
