import { type ReactNode } from 'react';
import { flatten } from '/@/modules/store/translation/flatten';
import Text from '@ui/atoms/Text';

import { en } from '/@/modules/store/translation/locales';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { getCurrentLocale, t } from '/@/modules/store/translation/utils';

import { Actions, Description, PageFallback } from './parts';

const english = flatten<typeof en, Record<string, string>>(en);

/**
 * The app-level boundary sits outside `TranslationProvider`, so it can catch an error
 * thrown before any locale was loaded; `t()` would throw on the missing locale.
 */
export function fallbackText(id: TranslationPath): string {
  return getCurrentLocale() ? t(id) : english[id];
}

type Props = {
  description: string;
  testId: string;
  /** The recovery buttons, in display order. */
  children: ReactNode;
};

export function ErrorFallback({ description, testId, children }: Props) {
  return (
    <PageFallback
      role="alert"
      data-testid={testId}
    >
      <Text
        variant="h5"
        tone="primary"
      >
        {fallbackText('error_boundary.title')}
      </Text>
      <Description variant="body1">{description}</Description>
      <Actions>{children}</Actions>
    </PageFallback>
  );
}
