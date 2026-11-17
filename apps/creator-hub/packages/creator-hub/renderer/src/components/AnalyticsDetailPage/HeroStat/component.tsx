import {
  ArrowDownwardIcon as ArrowDownIcon,
  ArrowUpwardIcon as ArrowUpIcon,
} from '/@/components/Icons/material';
import Text from '@ui/atoms/Text';

import './styles.css';

type Props = {
  value: string;
  /** Small unit set beside the value, e.g. "min". */
  unit?: string;
  /** Change against the previous period, already formatted. */
  delta?: { text: string; direction: 'up' | 'down' | 'flat' };
};

/** The figure a card leads with, plus how it moved. */
export function HeroStat({ value, unit, delta }: Props) {
  return (
    <div className="HeroStat">
      <div className="Value">
        <Text
          variant="h4"
          as="p"
          className="Figure"
        >
          {value}
        </Text>
        {unit && (
          <Text
            variant="body1"
            className="Unit"
          >
            {unit}
          </Text>
        )}
      </div>
      {delta && delta.direction !== 'flat' && (
        <div className={`Delta ${delta.direction === 'up' ? 'Up' : 'Down'}`}>
          {delta.direction === 'up' ? (
            <ArrowUpIcon fontSize="small" />
          ) : (
            <ArrowDownIcon fontSize="small" />
          )}
          <Text variant="body2">{delta.text}</Text>
        </div>
      )}
    </div>
  );
}
