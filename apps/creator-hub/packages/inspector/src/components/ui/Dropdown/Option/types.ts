import type { Props as TooltipProps } from '../..//InfoTooltip/types';

export type Props = {
  value?: string | number | readonly string[];
  label?: string;
  /** Shown in place of `label` when this option is the selected value in the trigger. */
  selectedLabel?: string;
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
  secondaryText?: string;
  description?: string;
  error?: boolean;
  selected?: boolean;
  disabled?: boolean;
  className?: string;
  header?: string;
  minWidth?: number;
  leftContent?: React.ReactNode;
  isField?: boolean;
  tooltip?: TooltipProps | string;
  onClick?: (event: any, value: any) => void;
};
