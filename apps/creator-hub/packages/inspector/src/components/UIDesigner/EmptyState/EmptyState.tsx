import React from 'react';

import './EmptyState.css';

interface EmptyStateProps {
  /** `canvas`: a canvas first-run state; `panel`: the right panel's Empty; `inline`: a hint inside a panel. */
  variant: 'canvas' | 'panel' | 'inline';
  icon?: React.ReactNode;
  title: string;
  message?: React.ReactNode;
  action?: React.ReactNode;
}

/** Shared empty / first-run state for the UI Designer: icon, title, guidance, optional CTA. */
export const EmptyState: React.FC<EmptyStateProps> = ({
  variant,
  icon,
  title,
  message,
  action,
}) => {
  const Title = variant === 'canvas' ? 'h2' : 'p';
  return (
    <div className={`ui-designer-empty-state ui-designer-empty-state--${variant}`}>
      {icon ? (
        <div
          className="ui-designer-empty-state-icon"
          aria-hidden="true"
        >
          {icon}
        </div>
      ) : null}
      <Title className="ui-designer-empty-state-title">{title}</Title>
      {message ? <p className="ui-designer-empty-state-message">{message}</p> : null}
      {action ? <div className="ui-designer-empty-state-action">{action}</div> : null}
    </div>
  );
};

/** Names a real control inside empty-state copy so the instruction points at something findable. */
export const EmptyStateChip: React.FC<{ tone: 'muted' | 'strong'; children: React.ReactNode }> = ({
  tone,
  children,
}) => (
  <span className={`ui-designer-empty-state-chip ui-designer-empty-state-chip--${tone}`}>
    {children}
  </span>
);

export default EmptyState;
