import React, { useMemo } from 'react';
import { IoIosArrowDown as OpenIcon, IoIosArrowForward as ClosedIcon } from 'react-icons/io';
import { getAssetsByCategory } from '../../../lib/logic/catalog';
import { Assets } from '../Assets';
import type { Props } from './types';
import './Categories.css';

const Categories: React.FC<Props> = ({ assets, collapsed, onToggle, onAddToFilesystem }) => {
  const assetsByCategory = useMemo(() => getAssetsByCategory(assets), [assets]);

  return (
    <div className="assets-catalog-categories">
      {Array.from(assetsByCategory, ([category, categoryAssets]) => {
        if (!categoryAssets.length) return null;
        const isOpen = !collapsed.has(category);
        return (
          <div
            className="assets-catalog-category"
            key={category}
            data-test-id={category}
          >
            <h4 className="category-name">
              <button
                type="button"
                aria-expanded={isOpen}
                onClick={() => onToggle(category)}
              >
                {isOpen ? <OpenIcon /> : <ClosedIcon />}
                <span>
                  {category} <span className="category-count">({categoryAssets.length})</span>
                </span>
              </button>
            </h4>
            {isOpen && (
              <Assets
                assets={categoryAssets}
                onAddToFilesystem={onAddToFilesystem}
              />
            )}
          </div>
        );
      })}
    </div>
  );
};

export default React.memo(Categories);
