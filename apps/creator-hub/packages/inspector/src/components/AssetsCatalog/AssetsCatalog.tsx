import React, { useCallback, useEffect, useMemo, useState } from 'react';

import { getAssetsByCategory, getCategory, type AssetPack } from '../../lib/logic/catalog';
import { analytics, Event } from '../../lib/logic/analytics';

import { Header } from './Header';
import { Themes } from './Themes';
import { Categories } from './Categories';

import type { Props } from './types';

import './AssetsCatalog.css';

const AssetsCatalog: React.FC<Props> = ({ catalog, onAddToFilesystem }) => {
  const [selectedTheme, setSelectedTheme] = useState<AssetPack>();
  const [search, setSearch] = useState<string>('');
  const [category, setCategory] = useState<string>();
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());

  const handleThemeChange = useCallback(
    (value?: AssetPack) => setSelectedTheme(value),
    [setSelectedTheme],
  );

  const handleSearchAssets = useCallback(
    (value: string) => {
      setSearch(value);
    },
    [setSearch],
  );

  const handleToggleCategory = useCallback((value: string) => {
    setCollapsed(current => {
      const next = new Set(current);
      if (!next.delete(value)) next.add(value);
      return next;
    });
  }, []);

  const scopeAssets = useMemo(
    () => (selectedTheme ? selectedTheme.assets : catalog.flatMap(theme => theme.assets)),
    [catalog, selectedTheme],
  );

  // The categories the pack being browsed has (or the whole catalog, outside a pack).
  const categories = useMemo(
    () =>
      Array.from(getAssetsByCategory(scopeAssets))
        .filter(([, assets]) => assets.length > 0)
        .map(([name, assets]) => ({ name, count: assets.length })),
    [scopeAssets],
  );

  // A category picked in one pack does not follow into a pack that lacks it.
  const activeCategory = categories.some($ => $.name === category) ? category : undefined;

  const filteredCatalog = useMemo(() => {
    const assets = activeCategory
      ? scopeAssets.filter(asset => getCategory(asset) === activeCategory)
      : scopeAssets;
    const trimmedSearch = search.trim();
    if (!trimmedSearch) return assets;

    const searchLower = trimmedSearch.toLowerCase();

    const starts: AssetPack['assets'] = [];
    const includes: AssetPack['assets'] = [];

    for (const asset of assets) {
      const name = (asset.name || '').toLowerCase();
      const description = (asset.description || '').toLowerCase();
      const tags = (asset.tags || []).map(tag => (tag || '').toLowerCase());

      // Priority 1: name word or tag starts with search term
      const nameStarts = name.split(' ').some(word => word.startsWith(searchLower));
      const tagStarts = tags.some(tag => tag.startsWith(searchLower));

      if (nameStarts || tagStarts) {
        starts.push(asset);
      } else {
        // Priority 2: search term appears anywhere in name, description, or tags
        const nameIncludes = name.includes(searchLower);
        const descriptionIncludes = description.includes(searchLower);
        const tagIncludes = tags.some(tag => tag.includes(searchLower));

        if (nameIncludes || descriptionIncludes || tagIncludes) {
          includes.push(asset);
        }
      }
    }

    // Return high-priority matches first, then lower-priority matches
    return [...starts, ...includes];
  }, [scopeAssets, activeCategory, search]);

  useEffect(() => {
    if (search) {
      analytics.track(Event.SEARCH_ITEM, {
        keyword: search,
        itemsFound: filteredCatalog.length,
        category: selectedTheme?.name,
      });
    }
  }, [search, filteredCatalog]);

  const renderEmptySearch = () => {
    const cta = activeCategory
      ? { text: 'search every category', onClick: () => setCategory(undefined) }
      : selectedTheme
        ? { text: 'search all asset packs', onClick: () => handleThemeChange() }
        : { text: 'upload your own asset by drag & drop' };
    return (
      <div className="empty-search">
        <span>No results for '{search}'.</span>
        <span>
          Try other words or{' '}
          {cta.onClick ? (
            <button
              type="button"
              className="empty-search-cta"
              onClick={cta.onClick}
            >
              {cta.text}
            </button>
          ) : (
            cta.text
          )}
          .
        </span>
      </div>
    );
  };

  if (!catalog) {
    return null;
  }

  return (
    <div className="assets-catalog">
      <Header
        search={search}
        selectedTheme={selectedTheme}
        categories={categories}
        selectedCategory={activeCategory}
        onChangeTheme={handleThemeChange}
        onChangeCategory={setCategory}
        onSearch={handleSearchAssets}
      />
      {search || selectedTheme || activeCategory ? (
        filteredCatalog.length > 0 ? (
          <Categories
            assets={filteredCatalog}
            collapsed={collapsed}
            onToggle={handleToggleCategory}
            onAddToFilesystem={onAddToFilesystem}
          />
        ) : (
          renderEmptySearch()
        )
      ) : (
        <div className="assets-catalog-theme-container">
          <Themes
            catalog={catalog}
            onClick={handleThemeChange}
          />
        </div>
      )}
    </div>
  );
};

export default React.memo(AssetsCatalog);
