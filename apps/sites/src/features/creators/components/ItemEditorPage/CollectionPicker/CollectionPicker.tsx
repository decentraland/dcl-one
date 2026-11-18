import { useState } from 'react'
import { AddIcon, ArrowBackIosNewIcon as BackIcon, SearchIcon } from '@/features/creators/components/Icons'
import { Button } from '@/features/creators/components/Button'
import { CollectionNameModal } from '@/features/creators/components/CollectionNameModal'
import { CollectionStatusPill } from '@/features/creators/components/CollectionStatusPill'
import { Pagination } from '@/features/creators/components/Pagination'
import { useTranslation } from '@/data/lib/creators/intl'
import { useSaveCollection } from '@/features/creators/hooks/useCollection'
import { useCollections } from '@/features/creators/hooks/useCollections'
import { CollectionStatusFilter, type Collection } from '@/data/lib/creators/collections'
import { buildNewCollection } from '@/data/lib/creators/saveCollection'
import { cx } from '@/data/lib/creators/cx'
import sidebar from '../ItemsSidebar/ItemsSidebar.module.css'
import shared from '@/features/creators/styles/shared.module.css'
import editor from '../ItemEditorPage.module.css'
import { Link } from 'react-router'

// The picker's rows are shorter than the collections page's cards, so it fits more per page.
const PICKER_PAGE_SIZE = 12

type Props = {
  address: string
  onPick: (collection: Collection) => void
  testId?: string
}

/** Shown when the editor is opened without a collection: the signer's standard collections to choose from. */
export function CollectionPicker({ address, onPick, testId = 'collection-picker' }: Props) {
  const { t } = useTranslation()
  const [search, setSearch] = useState('')
  const [page, setPage] = useState(1)
  const [isCreateOpen, setCreateOpen] = useState(false)
  const saveCollection = useSaveCollection(address)
  const query = useCollections(address, {
    page,
    search,
    status: CollectionStatusFilter.ALL,
    limit: PICKER_PAGE_SIZE
  })
  const collections = query.data?.results ?? []
  const pages = query.data?.pages ?? 1

  function closeCreate() {
    setCreateOpen(false)
    saveCollection.reset()
  }

  return (
    <aside className={sidebar.wrap} data-testid={testId}>
      <div className={sidebar.header}>
        <div className={sidebar.headerRow}>
          <Link
            className={sidebar.iconButton}
            to="/create/collections"
            aria-label={t('item_editor.picker.back')}
            data-testid={`${testId}-back`}
          >
            <BackIcon fontSize="small" />
          </Link>
          <div className={sidebar.collectionName}>
            <h2 className={sidebar.collectionTitle}>{t('item_editor.picker.title')}</h2>
          </div>
        </div>
        <div className={shared.searchBox} style={{ width: '100%', height: 40 }}>
          <SearchIcon fontSize="small" />
          <input
            value={search}
            placeholder={t('item_editor.picker.search')}
            aria-label={t('item_editor.picker.search')}
            data-testid={`${testId}-search`}
            onChange={event => {
              setSearch(event.target.value)
              setPage(1)
            }}
          />
        </div>
        <Button
          type="button"
          variant="secondary"
          size="sm"
          data-testid={`${testId}-create`}
          onClick={() => setCreateOpen(true)}
        >
          <AddIcon fontSize="small" />
          {t('item_editor.picker.create')}
        </Button>
      </div>
      <div className={sidebar.list}>
        {query.isLoading ? (
          <div className={sidebar.rows} aria-busy="true" data-testid={`${testId}-loading`}>
            {Array.from({ length: PICKER_PAGE_SIZE }, (_, index) => (
              <div className={cx(sidebar.skeletonRow, 'skeleton')} key={index} />
            ))}
          </div>
        ) : query.isError ? (
          <p className={editor.stateText} data-testid={`${testId}-error`}>
            {t('item_editor.picker.error')}
          </p>
        ) : collections.length === 0 ? (
          <p className={sidebar.empty} data-testid={`${testId}-empty`}>
            {t('item_editor.picker.empty')}
          </p>
        ) : (
          <div className={sidebar.rows}>
            {collections.map(collection => (
              <div className={sidebar.row} key={collection.id} data-testid={`${testId}-row-${collection.id}`}>
                <button
                  className={sidebar.rowButton}
                  type="button"
                  data-testid={`${testId}-pick-${collection.id}`}
                  onClick={() => onPick(collection)}
                >
                  <span className={sidebar.rowName} title={collection.name}>
                    {collection.name}
                  </span>
                  <CollectionStatusPill collection={collection} />
                </button>
              </div>
            ))}
          </div>
        )}
        {pages > 1 && (
          <div className={sidebar.paginationWrap}>
            <Pagination page={page} pages={pages} onPageChange={setPage} />
          </div>
        )}
      </div>
      {isCreateOpen && (
        <CollectionNameModal
          variant="create"
          isPending={saveCollection.isPending}
          error={saveCollection.error?.message ?? null}
          onSubmit={name =>
            saveCollection.mutate(buildNewCollection(name, address), {
              // A collection created here is the one the creator wants to fill: open it right away.
              onSuccess: created => {
                closeCreate()
                onPick(created)
              }
            })
          }
          onClose={closeCreate}
        />
      )}
    </aside>
  )
}
