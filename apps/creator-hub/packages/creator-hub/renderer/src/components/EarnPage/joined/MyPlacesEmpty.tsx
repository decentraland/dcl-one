import { useEffect, useId, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from 'decentraland-ui2';

import type { Events } from '/shared/types/analytics';
import type { Project } from '/shared/types/projects';

import { analytics } from '#preload';
import { useWorkspace } from '/@/hooks/useWorkspace';
import { addBase64ImagePrefix } from '/@/modules/image';
import { t } from '/@/modules/store/translation/utils';

import modal from '../EarnModal.module.css';
import { ParcelsIcon } from '../icons';

import { ListState } from './parts';
import { findPublishedPaths } from './publishedScenes';
import styles from './MyPlacesEmpty.module.css';

const SHOWN_SCENES = 3;

const trackEmptyStateAction = (action: Events['Creator Fund Empty State Action']['action']) =>
  void analytics.track('Creator Fund Empty State Action', { action });

const newestEditsFirst = (projects: Project[]) =>
  [...projects].sort((a, b) => b.updatedAt - a.updatedAt).slice(0, SHOWN_SCENES);

function useUnpublishedProjects(projects: Project[]) {
  const [checked, setChecked] = useState<{ projects: Project[]; unpublished: Project[] } | null>(
    null,
  );
  useEffect(() => {
    let current = true;
    void findPublishedPaths(projects).then(published => {
      if (!current) return;
      setChecked({
        projects,
        unpublished: newestEditsFirst(projects.filter(project => !published.has(project.path))),
      });
    });
    return () => {
      current = false;
    };
  }, [projects]);
  return checked?.projects === projects ? checked.unpublished : null;
}

/** A local scene as the design-system Scene-Card, with no menu; it opens the scene. */
export function UnpublishedSceneCard({
  project,
  onOpen,
}: {
  project: Project;
  onOpen: (project: Project) => void;
}) {
  const nameId = useId();
  return (
    <button
      className={styles.card}
      type="button"
      aria-labelledby={nameId}
      onClick={() => onOpen(project)}
    >
      <img
        className={styles.cardThumbnail}
        src={project.thumbnail ? addBase64ImagePrefix(project.thumbnail) : undefined}
        alt=""
        width={416}
        height={273}
        loading="lazy"
      />
      <span className={styles.cardText}>
        <span
          className={styles.cardName}
          id={nameId}
          title={project.title}
        >
          {project.title}
        </span>
        <span className={styles.cardParcels}>
          <ParcelsIcon />
          {t('earn.joined.my_places.unpublished.parcels', {
            parcels: project.scene.parcels.length,
          })}
        </span>
      </span>
    </button>
  );
}

function UnpublishedScenes({ projects }: { projects: Project[] }) {
  const { runProject } = useWorkspace();
  const headingId = useId();
  return (
    <section
      className={styles.scenes}
      aria-labelledby={headingId}
    >
      <h3
        className={styles.scenesHeading}
        id={headingId}
      >
        {t('earn.joined.my_places.unpublished.heading')}
      </h3>
      <div className={styles.cards}>
        {projects.map(project => (
          <UnpublishedSceneCard
            key={project.path}
            project={project}
            onOpen={scene => {
              trackEmptyStateAction('open_scene');
              runProject(scene);
            }}
          />
        ))}
      </div>
    </section>
  );
}

function NoScenes() {
  const navigate = useNavigate();
  const { createProject } = useWorkspace();
  return (
    <section className={styles.noScenesState}>
      <h2 className={styles.title}>{t('earn.joined.my_places.no_scenes.title')}</h2>
      <p className={styles.body}>{t('earn.joined.my_places.empty_body')}</p>
      <div className={styles.actions}>
        <Button
          className={modal.modalCta}
          variant="contained"
          color="secondary"
          onClick={() => {
            trackEmptyStateAction('start_from_template');
            navigate('/templates');
          }}
        >
          {t('earn.joined.my_places.start_from_template')}
        </Button>
        <Button
          className={modal.modalCta}
          variant="contained"
          color="primary"
          onClick={() => {
            trackEmptyStateAction('create_scene');
            createProject({});
          }}
        >
          {t('earn.joined.my_places.create_scene')}
        </Button>
      </div>
    </section>
  );
}

export type MyPlacesEmptyProps =
  | { variant: 'no_scenes' }
  | { variant: 'unpublished'; projects: Project[] };

function PublishPrompt({ projects }: { projects: Project[] }) {
  const unpublished = useUnpublishedProjects(projects);
  if (!unpublished) return <ListState kind="loading" />;
  return (
    <section className={styles.unpublishedState}>
      <div className={styles.header}>
        <h2 className={styles.title}>{t('earn.joined.my_places.unpublished.title')}</h2>
        <p className={styles.body}>{t('earn.joined.my_places.empty_body')}</p>
      </div>
      {unpublished.length > 0 && <UnpublishedScenes projects={unpublished} />}
    </section>
  );
}

/** My Places in the current cycle with no places: create a first scene, or publish one. */
export function MyPlacesEmpty(props: MyPlacesEmptyProps) {
  return props.variant === 'no_scenes' ? <NoScenes /> : <PublishPrompt projects={props.projects} />;
}
