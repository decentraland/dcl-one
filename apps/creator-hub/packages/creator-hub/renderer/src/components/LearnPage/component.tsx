import { useNavigate } from 'react-router';
import { cx } from '/@/lib/cx';
import Button from '@ui/atoms/Button';
import Text from '@ui/atoms/Text';
import { t } from '/@/modules/store/translation/utils';
import { tutorials } from '/@/modules/tutorials';
import { openLearnResource } from '/@/lib/openLearnResource';

import { Container } from '../Container';
import { Navbar, NavbarItem } from '../Navbar';
import { Image } from '../Image';
import styles from './component.module.css';

import './styles.css';

function Video(props: { id: string; list: string; title: string }) {
  const url = `https://youtu.be/${props.id}?list=${props.list}`;
  return (
    <div
      className="link"
      onClick={() => openLearnResource('learn', 'video', props.title, url)}
    >
      <div className="thumbnail-wrapper">
        <Image
          src={`https://img.youtube.com/vi/${props.id}/0.jpg`}
          alt={props.title}
        />
      </div>
      <span className="title">{props.title}</span>
    </div>
  );
}

function Link(props: { url: string; title: string }) {
  return (
    <div
      className="link"
      onClick={() => openLearnResource('learn', 'doc', props.title, props.url)}
    >
      <i className="icon" />
      <span className="title">{props.title}</span>
    </div>
  );
}

export function LearnPage() {
  const navigate = useNavigate();
  return (
    <main className="LearnPage">
      <Navbar active={NavbarItem.LEARN} />
      <Container>
        <div className="content">
          <Text
            variant="h3"
            className={styles.title}
          >
            {t('learn.header.title')}
          </Text>
          <div className="sections">
            <div className="section videos">
              <div
                className="header clickable"
                onClick={() => navigate('/learn/videos')}
              >
                <i className="image"></i>
                <span className="title">{t('learn.header.videos')}</span>
              </div>
              <div className="content">
                {tutorials.map(video => (
                  <Video
                    key={video.id}
                    id={video.id}
                    list={video.list}
                    title={video.title}
                  />
                ))}
              </div>
              <Button
                className={cx('see-all', styles.seeAll)}
                variant="text"
                onClick={() => navigate('/learn/videos')}
              >
                {t('learn.see_all')}
              </Button>
            </div>
            <div className="section docs">
              <div
                className="header clickable"
                onClick={() => navigate('/learn/docs')}
              >
                <i className="image"></i>
                <span className="title">{t('learn.header.docs')}</span>
              </div>
              <div className="content">
                <Link
                  url="https://docs.decentraland.org/creator/"
                  title="Let's build the metaverse together"
                />
                <Link
                  url="https://docs.decentraland.org/creator/scenes-sdk7/getting-started/sdk-101"
                  title="About SDK"
                />
                <Link
                  url="https://docs.decentraland.org/creator/scenes-sdk7/getting-started/dev-workflow"
                  title="Development Workflow"
                />
                <Link
                  url="https://docs.decentraland.org/creator/wearables/wearables-overview/"
                  title="Wearable Overview"
                />
                <Link
                  url="https://docs.decentraland.org/creator/emotes/emotes-overview/"
                  title="Emotes Overview"
                />
                <Link
                  url="https://docs.decentraland.org/creator/wearables-and-emotes/manage-collections/creating-collection/"
                  title="Creating a Collection"
                />
              </div>
              <Button
                className={cx('see-all', styles.seeAll)}
                variant="text"
                onClick={() => navigate('/learn/docs')}
              >
                {t('learn.see_all')}
              </Button>
            </div>
            <div className="section more">
              <div className="header">
                <i className="image"></i>
                <span className="title">{t('learn.header.more')}</span>
              </div>
              <div className="content">
                <Link
                  url="https://studios.decentraland.org/"
                  title="Decentraland Studios"
                />
                <Link
                  url="https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Scene"
                  title="Example Scenes"
                />
                <Link
                  url="https://docs.decentraland.org/contributor/"
                  title="Open Protocol Docs"
                />
              </div>
            </div>
          </div>
        </div>
      </Container>
    </main>
  );
}
