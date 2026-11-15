import { useState } from 'react';
import Collapse from '@ui/atoms/Collapse';
import IconButton from '@ui/atoms/IconButton';
import Text from '@ui/atoms/Text';
import { ExpandMoreIcon } from '@ui/atoms/icons-material';

import type { Props } from './types';

import './styles.css';

export function ExpandMore({ title, text }: Props) {
  const [expanded, setExpanded] = useState(false);

  const handleToggle = () => {
    setExpanded(!expanded);
  };

  return (
    <div className="ExpandMore">
      <div className="header">
        <IconButton
          className="ExpandMoreToggle"
          onClick={handleToggle}
          aria-expanded={expanded}
          aria-label={title || 'show more'}
        >
          <ExpandMoreIcon />
        </IconButton>
        {title && (
          <Text
            className="title"
            variant="subtitle1"
            as="h6"
          >
            {title}
          </Text>
        )}
      </div>
      <Collapse open={expanded}>{expanded && <Text variant="body2">{text}</Text>}</Collapse>
    </div>
  );
}
