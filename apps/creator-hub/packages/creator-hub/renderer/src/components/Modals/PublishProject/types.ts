import { type Project } from '/shared/types/projects';

export type Coordinate = { x: number; y: number };

export type DeploymentMetadata = {
  isMultiScene?: boolean;
  /** The world being published to. The project prop can still hold the name from before the
   * publish step chose it. */
  worldName?: string;
};

export type Props = {
  open: boolean;
  project: Project;
  initialStep?: Step;
  disableGoBack?: boolean;
  previousStep?: Step;
  deploymentMetadata?: DeploymentMetadata;
  onClose: () => void;
  onBack?: () => void;
  onStep: (
    step: Step,
    opts?: { resetHistory?: boolean; deploymentMetadata?: DeploymentMetadata },
  ) => void;
};

export type Step =
  | 'initial'
  | 'alternative-servers'
  | 'publish-to-world'
  | 'publish-to-land'
  | 'deploy';

export const COLORS = Object.freeze({
  occupiedParcel: '#774642',
  freeParcel: '#ff9990',
  selected: '#ff9990',
  selectedStroke: '#ff0044',
  indicator: '#716b7abb',
  indicatorStroke: '#3a3541',
});
