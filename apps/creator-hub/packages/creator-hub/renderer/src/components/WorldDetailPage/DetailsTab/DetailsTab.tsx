import { useRef } from 'react';
import IconButton from '@ui/atoms/IconButton';

import { SceneCategory, type WorldSettings } from '/@/lib/worlds';
import { t } from '/@/modules/store/translation/utils';

import { EditIcon } from '../../Icons';
import { DESCRIPTION_MAX, NAME_MAX } from '../../LandDetailPage/DetailsTab/details';
import { DetailsField } from '../../LandDetailPage/DetailsTab/DetailsField';
import { Field, FieldTitle } from '../../LandDetailPage/DetailsTab/DetailsTab.styled';
import { Loader } from '../../Loader';
import { FieldSelect, OptionCheck, SelectOption } from '../FieldSelect';
import { useWorldSettings } from '../useWorldSettings';
import { useWorldSettingsDraft } from '../useWorldSettingsDraft';

import styles from './DetailsTab.module.css';

const MAX_CATEGORIES = 3;
const CATEGORIES = Object.values(SceneCategory);

type Draft = {
  title: string;
  description: string;
  categories: SceneCategory[];
  thumbnail?: string;
};

const toDraft = (settings: WorldSettings): Draft => ({
  title: settings.title ?? '',
  description: settings.description ?? '',
  categories: settings.categories ?? [],
});

const categoryLabel = (category: SceneCategory) =>
  t(`world_detail.details.categories_options.${category}`);

const tooLong = (value: string, max: number) =>
  value.length > max ? { key: 'land_detail.details.too_long' as const, max } : null;

function changesOf(draft: Draft, saved: WorldSettings): Partial<WorldSettings> {
  const changes: Partial<WorldSettings> = {};
  if (draft.title !== (saved.title ?? '')) changes.title = draft.title;
  if (draft.description !== (saved.description ?? '')) changes.description = draft.description;
  if (draft.categories.join() !== (saved.categories ?? []).join())
    changes.categories = draft.categories.length ? draft.categories : null;
  if (draft.thumbnail) changes.thumbnail = draft.thumbnail;
  return changes;
}

function DetailsForm({ worldName, saved }: { worldName: string; saved: WorldSettings }) {
  const fileInput = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useWorldSettingsDraft({
    worldName,
    saved,
    toDraft,
    changesOf,
    valid: current =>
      !tooLong(current.title, NAME_MAX) && !tooLong(current.description, DESCRIPTION_MAX),
  });
  const errors = {
    name: tooLong(draft.title, NAME_MAX),
    description: tooLong(draft.description, DESCRIPTION_MAX),
  };

  const pickThumbnail = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => setDraft(current => ({ ...current, thumbnail: reader.result as string }));
    reader.readAsDataURL(file);
    event.target.value = '';
  };

  const thumbnail = draft.thumbnail ?? saved.thumbnail;

  return (
    <div className={styles.columns}>
      <div className={styles.form}>
        <DetailsField
          label={t('world_detail.details.name')}
          value={draft.title}
          max={NAME_MAX}
          error={errors.name}
          placeholder={t('land_detail.details.name_placeholder')}
          multiline="single"
          onChange={title => setDraft(current => ({ ...current, title }))}
        />
        <DetailsField
          label={t('world_detail.details.description')}
          value={draft.description}
          max={DESCRIPTION_MAX}
          error={errors.description}
          placeholder={t('world_detail.details.description_placeholder')}
          multiline="multi"
          onChange={description => setDraft(current => ({ ...current, description }))}
        />
        <Field>
          <FieldTitle as="span">{t('world_detail.details.categories')}</FieldTitle>
          <FieldSelect<SceneCategory>
            multiple
            value={draft.categories}
            aria-label={t('world_detail.details.categories')}
            menuClassName={styles.menu}
            renderValue={selected => selected.map(categoryLabel).join(', ')}
            onChange={categories => setDraft(current => ({ ...current, categories }))}
          >
            {CATEGORIES.map(category => (
              <SelectOption
                key={category}
                value={category}
                label={categoryLabel(category)}
                disabled={
                  draft.categories.length >= MAX_CATEGORIES && !draft.categories.includes(category)
                }
              >
                <OptionCheck checked={draft.categories.includes(category)} />
                {categoryLabel(category)}
              </SelectOption>
            ))}
          </FieldSelect>
        </Field>
      </div>
      <div className={styles.thumbnailColumn}>
        <h3 className={styles.thumbnailTitle}>{t('world_detail.details.thumbnail')}</h3>
        <p className={styles.thumbnailHint}>{t('world_detail.details.thumbnail_hint')}</p>
        <div className={styles.thumbnailBox}>
          {thumbnail ? (
            <img
              src={thumbnail}
              alt={t('world_detail.details.thumbnail')}
            />
          ) : (
            <div className={styles.noImage}>{t('world_detail.details.no_image')}</div>
          )}
          <IconButton
            className={styles.editButton}
            aria-label={t('world_detail.details.change_thumbnail')}
            onClick={() => fileInput.current?.click()}
          >
            <EditIcon />
          </IconButton>
          <input
            ref={fileInput}
            type="file"
            accept="image/png,image/jpeg"
            hidden
            onChange={pickThumbnail}
          />
        </div>
      </div>
    </div>
  );
}

/** The World's name, description, categories and thumbnail, saved off-chain from the shell's SAVE. */
export function DetailsTab({ worldName }: { worldName: string }) {
  const settings = useWorldSettings(worldName);
  if (settings === 'failed') return null;
  if (settings === 'loading') return <Loader size={70} />;
  return (
    <DetailsForm
      worldName={worldName}
      saved={settings}
    />
  );
}
