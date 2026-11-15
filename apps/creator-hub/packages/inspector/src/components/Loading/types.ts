export type SpinnerSize =
  | 'mini'
  | 'tiny'
  | 'small'
  | 'medium'
  | 'large'
  | 'big'
  | 'huge'
  | 'massive';

export interface SpinnerProps {
  size?: SpinnerSize;
}

export interface Props extends SpinnerProps {
  dimmer?: boolean;
}
