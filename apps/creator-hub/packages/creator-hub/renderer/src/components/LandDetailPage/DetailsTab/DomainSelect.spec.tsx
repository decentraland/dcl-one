import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import IconButton from '@ui/atoms/IconButton';
import { SelectOption } from '@ui/atoms/Select';

import { TestProviders } from '../../../../tests/utils/TestProviders';
import { mountThemeStyles } from '../../../../tests/utils/themeStyles';

import { ParcelSelect, Row } from '../access/access.styled';
import { DomainSelect } from './DetailsTab.styled';

const focusVisibleRules = () =>
  [...document.querySelectorAll('style')]
    .flatMap(style => (style.textContent ?? '').split('}'))
    .filter(rule => rule.includes(':focus-visible'));

// The test DOM does not apply `:focus-visible` rules, so the check reads the rules themselves.
const hasFocusRing = (element: Element) =>
  focusVisibleRules()
    .map(rule => rule.replace(/\/\*[\s\S]*?\*\//g, '').split('{'))
    .filter(([, body = '']) => /outline:[^;]*solid/.test(body))
    .flatMap(([selectors]) => selectors!.split(','))
    .filter(selector => selector.includes(':focus-visible'))
    .some(selector => element.matches(selector.replaceAll(':focus-visible', '').trim()));

describe('DomainSelect', () => {
  let unmountThemeStyles: () => void;

  beforeEach(() => {
    unmountThemeStyles = mountThemeStyles();
  });

  afterEach(() => {
    cleanup();
    unmountThemeStyles();
  });

  describe('when it takes focus', () => {
    it('should draw a Snow border, and no keyboard focus ring', async () => {
      render(
        <TestProviders>
          <DomainSelect
            value="plaza"
            aria-label="Domain"
            onChange={() => {}}
          >
            <SelectOption value="plaza">plaza.dcl.eth</SelectOption>
          </DomainSelect>
        </TestProviders>,
      );
      // A role query computes the style, and the test DOM keeps that style through a focus change.
      const combobox = screen.getByLabelText('Domain');

      await act(async () => combobox.focus());

      // Snow comes from the theme stylesheet of the app; the test sets no token.
      expect(getComputedStyle(combobox).borderTopColor.toLowerCase()).toBe('#fcfcfc');
      expect(screen.getByRole('combobox', { name: 'Domain' })).toBe(combobox);
      expect(focusVisibleRules().filter(rule => /63B4F6|99, ?180, ?246/i.test(rule))).toEqual([]);
    });
  });

  describe('when it takes focus in a row of the access list', () => {
    it('should draw no focus ring, and the remove button keeps its ring', () => {
      render(
        <TestProviders>
          <ul>
            <Row>
              <span className="Scope">
                <ParcelSelect
                  value="all"
                  aria-label="Parcels"
                  onChange={() => {}}
                >
                  <SelectOption value="all">All parcels</SelectOption>
                </ParcelSelect>
              </span>
              <IconButton
                className="Trash"
                aria-label="Remove"
              >
                <span />
              </IconButton>
            </Row>
          </ul>
        </TestProviders>,
      );

      expect(hasFocusRing(screen.getByRole('combobox', { name: 'Parcels' }))).toBe(false);
      expect(hasFocusRing(screen.getByRole('button', { name: 'Remove' }))).toBe(true);
    });
  });
});
