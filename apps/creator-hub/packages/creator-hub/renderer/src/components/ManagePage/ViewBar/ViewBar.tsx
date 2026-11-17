import { useId } from 'react';
import { cx } from '/@/lib/cx';
import { SelectOption } from '@ui/atoms/Select';
import { CheckIcon, SearchIcon } from '/@/components/Icons/material';

import { t } from '/@/modules/store/translation/utils';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { FilterBy, type ManageTab, SortBy } from '/shared/types/manage';

import { ChevronDownIcon } from '../../Icons';
import { useArrowKeyed } from '../../Dropdown/useArrowKeyed';
import { Search } from '../../Search';
import { Select } from '../../Select';
import { Bar } from './Bar';
import { ManageTabs } from './ManageTabs';

import styles from './ViewBar.module.css';

const ALL = 'all';

type ViewValue = FilterBy | typeof ALL;

const VIEW_OPTIONS: Array<{ value: ViewValue; label: TranslationPath }> = [
  { value: ALL, label: 'manage.filters.all' },
  { value: FilterBy.PUBLISHED, label: 'manage.filters.published' },
  { value: FilterBy.UNPUBLISHED, label: 'manage.filters.unpublished' },
];

export const SORT_OPTIONS: Array<{ value: SortBy; label: TranslationPath }> = [
  { value: SortBy.LATEST, label: 'manage.sort.latest' },
  { value: SortBy.NAME, label: 'manage.sort.name' },
];

type Option<T> = { value: T; label: TranslationPath };

type SelectFieldProps<T extends string> = {
  label: TranslationPath;
  value: T;
  options: Option<T>[];
  onChange: (value: T) => void;
};

/** The View-bar search box, its icon at the start. */
export function SearchField({
  defaultValue,
  onSearch,
}: {
  defaultValue?: string;
  onSearch: (query: string) => void;
}) {
  return (
    <Search
      className={styles.searchBox}
      type="search"
      placeholder={t('manage.search')}
      defaultValue={defaultValue}
      startAdornment={<SearchIcon aria-hidden />}
      endAdornment={null}
      inputProps={{ 'aria-label': t('manage.search') }}
      onChange={onSearch}
    />
  );
}

/** A labelled select in the DS Dropdown-Input look. */
export function SelectField<T extends string>({
  label,
  value,
  options,
  onChange,
}: SelectFieldProps<T>) {
  const labelId = useId();
  const { arrowKeyedClass, onKeyDownCapture, resetArrowKeyed } = useArrowKeyed();
  const labelOf = (selected: T) => t(options.find(option => option.value === selected)!.label);

  return (
    <div
      className={styles.field}
      onKeyDownCapture={onKeyDownCapture}
    >
      <span
        id={labelId}
        className={styles.fieldLabel}
      >
        {t(label)}
      </span>
      <Select
        className={styles.viewSelect}
        labelId={labelId}
        value={value}
        IconComponent={ChevronDownIcon}
        renderValue={selected => labelOf(selected as T)}
        onChange={event => onChange(event.target.value as T)}
        onOpen={resetArrowKeyed}
        MenuProps={{ className: cx(styles.menu, arrowKeyedClass) }}
      >
        {options.map(option => (
          <SelectOption
            key={option.value}
            value={option.value}
          >
            {option.value === value && <CheckIcon aria-hidden />}
            {t(option.label)}
          </SelectOption>
        ))}
      </Select>
    </div>
  );
}

type Props = {
  labelledBy: string;
  tab: ManageTab;
  view: FilterBy | undefined;
  sortBy: SortBy;
  searchQuery: string;
  onTabChange: (tab: ManageTab) => void;
  onViewChange: (view: FilterBy | undefined) => void;
  onSortChange: (sortBy: SortBy) => void;
  onSearch: (query: string) => void;
};

/** The Manage filter row: WORLDS | LANDS switch, View, Sort by and Search. */
export function ManageViewBar(props: Props) {
  return (
    <Bar>
      <ManageTabs
        value={props.tab}
        onChange={props.onTabChange}
        labelledBy={props.labelledBy}
      />
      <>
        <SelectField
          label="manage.view"
          value={props.view ?? ALL}
          options={VIEW_OPTIONS}
          onChange={value => props.onViewChange(value === ALL ? undefined : value)}
        />
        <SelectField
          label="manage.sort_by"
          value={props.sortBy}
          options={SORT_OPTIONS}
          onChange={props.onSortChange}
        />
        <SearchField
          defaultValue={props.searchQuery}
          onSearch={props.onSearch}
        />
      </>
    </Bar>
  );
}
