import React, { useMemo } from 'react';
import { MdChevronLeft as BackIcon, MdChevronRight as ChevronIcon } from 'react-icons/md';
import { cx } from '../../../lib/utils/cx';
import { AssetSearchField } from '../../AssetSearchField';
import { Dropdown } from '../../ui/Dropdown';
import type { Props } from './types';

import './Header.css';

const ALL_CATEGORIES = '';

const Header: React.FC<Props> = ({
  selectedTheme,
  search,
  categories,
  selectedCategory,
  onChangeTheme,
  onChangeCategory,
  onSearch,
}) => {
  const isMainHeader = !search && !selectedTheme && !selectedCategory;
  const placeholder = selectedTheme ? `Search ${selectedTheme.name}` : 'Search Asset Packs';

  const goToPacks = () => {
    onSearch('');
    onChangeCategory();
    onChangeTheme();
  };

  const categoryOptions = useMemo(
    () => [
      { value: ALL_CATEGORIES, label: 'All categories' },
      ...categories.map(({ name, count }) => ({
        value: name,
        label: name.charAt(0).toUpperCase() + name.slice(1),
        secondaryText: count.toString(),
      })),
    ],
    [categories],
  );

  const goToPack = () => onSearch('');

  const crumbs: { label: string; onClick?: () => void }[] = [
    { label: 'Asset Packs', onClick: isMainHeader ? undefined : goToPacks },
  ];
  if (selectedTheme) {
    crumbs.push({ label: selectedTheme.name, onClick: search ? goToPack : undefined });
  }
  if (search) crumbs.push({ label: `Results for "${search}"` });

  return (
    <div className="assets-catalog-header">
      <div className="Assets-toolbar">
        {!isMainHeader && (
          <button
            className="Assets-iconButton"
            onClick={search && selectedTheme ? goToPack : goToPacks}
            title="Back"
            aria-label="Back"
          >
            <BackIcon />
          </button>
        )}
        <AssetSearchField
          className="assets-catalog-header-search"
          value={search}
          placeholder={placeholder}
          onChange={onSearch}
          debounceMs={300}
        />
        <Dropdown
          className="assets-catalog-header-category"
          aria-label="Category"
          options={categoryOptions}
          value={selectedCategory ?? ALL_CATEGORIES}
          onChange={event => onChangeCategory((event.target.value as string) || undefined)}
        />
        {!isMainHeader && (
          <nav
            className="Assets-crumbs inline assets-catalog-header-title"
            aria-label="Asset pack path"
          >
            {crumbs.map((crumb, index) => (
              <React.Fragment key={crumb.label}>
                {index > 0 && <ChevronIcon />}
                <button
                  className={cx({ current: index === crumbs.length - 1 })}
                  onClick={crumb.onClick}
                  disabled={!crumb.onClick}
                >
                  {crumb.label}
                </button>
              </React.Fragment>
            ))}
          </nav>
        )}
      </div>
    </div>
  );
};

export default React.memo(Header);
