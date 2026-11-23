import React, { useMemo } from 'react';
import { cx } from '/@/lib/cx';
import { PlaceIcon as PinIcon, GridViewRoundedIcon as ParcelsIcon } from '@ui/atoms/icons-material';
import { ChainId } from '@dcl/schemas';
import Address from '@ui/atoms/Address';
import Text from '@ui/atoms/Text';
import { addBase64ImagePrefix } from '/@/modules/image';
import { formatWorldSize } from '/@/modules/world';
import { t } from '/@/modules/store/translation/utils';
import type { Project } from '/shared/types/projects';
import { useAuth } from '/@/hooks/useAuth';
import { useSelector } from '#store';
import { coordsToId, RoleType } from '/@/lib/land';
import FallbackThumbnail from '/assets/images/scene-thumbnail-fallback.png';
import './styles.css';

type Props = {
  children: React.ReactNode;
  isWorld: boolean;
  name?: string;
  project: Project;
  className?: string;
};

enum UserRole {
  OWNER = 'owner',
  COLLABORATOR = 'collaborator',
}

const ProjectStepWrapper: React.FC<Props> = ({ isWorld, name, project, children, className }) => {
  const { chainId, wallet, avatar } = useAuth();
  const ensData = useSelector(state => state.ens.data);
  const landData = useSelector(state => state.land.data);

  const userRole: UserRole | null = useMemo(() => {
    if (!wallet) return null;

    if (isWorld && name) {
      const ensEntry = ensData[name];
      if (ensEntry) {
        const isOwner = ensEntry.nftOwnerAddress?.toLowerCase() === wallet.toLowerCase();
        return isOwner ? UserRole.OWNER : UserRole.COLLABORATOR;
      }
    } else if (!isWorld && project.scene.base) {
      const landEntry = landData.find(
        land =>
          land.deployable &&
          ((land.type === 'parcel' && coordsToId(land.x!, land.y!) === project.scene.base) ||
            (land.type === 'estate' &&
              land.parcels?.some(
                parcel => coordsToId(parcel.x!, parcel.y!) === project.scene.base,
              ))),
      );

      if (landEntry) {
        return landEntry.role === RoleType.OWNER ? UserRole.OWNER : UserRole.COLLABORATOR;
      }
    }

    return null;
  }, [wallet, isWorld, project, name, ensData, landData]);

  return (
    <div className="ProjectStepWrapper">
      <div className="ChipsContainer">
        <div className="chip network">
          {chainId === ChainId.ETHEREUM_MAINNET
            ? t('modal.publish_project.deploy.ethereum.mainnet')
            : t('modal.publish_project.deploy.ethereum.testnet')}
        </div>
        {wallet && (
          <div className="chip address">
            <Address
              value={wallet}
              shorten
            />
          </div>
        )}
        {isWorld ? (
          avatar && (
            <div className="chip username">
              {avatar.name}
              {avatar.hasClaimedName ? <i className="verified"></i> : null}
            </div>
          )
        ) : (
          <div className="chip parcel">
            <PinIcon className="pin" />
            {project.scene.base}
          </div>
        )}
        {userRole && (
          <div className="chip role">{t(`modal.publish_project.roles.${userRole}`)}</div>
        )}
      </div>
      <div className="ProjectContainer">
        <div className="info">
          <div
            className="thumbnail"
            data-testid="publish-modal-project-thumbnail"
            style={{
              backgroundImage: `url(${project.thumbnail ? addBase64ImagePrefix(project.thumbnail) : FallbackThumbnail})`,
            }}
          />
          <div className="text">
            <Text variant="h6">{project.title}</Text>
            <Text
              variant="body2"
              className="parcels"
            >
              <ParcelsIcon />
              {t('modal.publish_project.deploy.scene_size', {
                size: formatWorldSize({ width: project.layout.cols, height: project.layout.rows }),
              })}
            </Text>
          </div>
        </div>
        <div className={cx('content', className)}>{children}</div>
      </div>
    </div>
  );
};

export { ProjectStepWrapper };
