import type { ComponentType } from 'react';
import { cx } from '/@/lib/cx';

import {
  CalendarIcon,
  JumpInIcon,
  LinkIcon,
  PublicIcon,
  StorageIcon,
  SwapIcon,
  UnpublishIcon,
} from '../Icons';
import type { CommonProps } from '../Icons/types';

import styles from './cardMenuGlyphs.module.css';

const sized = (Icon: ComponentType<CommonProps>, sizeClass: string) =>
  function Glyph({ className }: CommonProps) {
    return <Icon className={cx(sizeClass, className)} />;
  };

const CalendarGlyph = sized(CalendarIcon, styles.s16);
const JumpInGlyph = sized(JumpInIcon, styles.jumpIn);
const LinkGlyph = sized(LinkIcon, styles.s16);
const PlaceGlyph = sized(PublicIcon, styles.s16);
const StorageGlyph = sized(StorageIcon, styles.s16);
const SwapGlyph = sized(SwapIcon, styles.s16);
const UnpublishGlyph = sized(UnpublishIcon, styles.s22);

export {
  CalendarGlyph,
  JumpInGlyph,
  LinkGlyph,
  PlaceGlyph,
  StorageGlyph,
  SwapGlyph,
  UnpublishGlyph,
};
