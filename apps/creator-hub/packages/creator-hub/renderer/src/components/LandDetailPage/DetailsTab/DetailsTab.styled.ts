import { createElement } from 'react';
import type { ComponentProps } from 'react';
import { cx } from '/@/lib/cx';

import { ExternalLink } from '../../ExternalLink';
import { FormTextInput } from '../../TextInput';
import { classed } from '../classed';
import { DomainSelect } from './DomainSelect';

import styles from './DetailsTab.module.css';

type InputProps = ComponentProps<typeof FormTextInput>;

// A component of the old library outside this folder still builds its link from these values.
type LinkTheme = {
  palette: { primary: { main: string }; info: { light: string } };
  typography: { pxToRem: (size: number) => string };
};

const primaryLinkStyles = ({ theme }: { theme: LinkTheme }) => ({
  alignSelf: 'flex-start',
  color: theme.palette.primary.main,
  fontSize: theme.typography.pxToRem(14),
  fontWeight: 600,
  lineHeight: 'normal',
  textDecoration: 'underline',
  textUnderlineOffset: '3px',
  textDecorationThickness: '1px',
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.info.light}`,
    outlineOffset: 2,
  },
});

function ClaimLink(props: ComponentProps<typeof ExternalLink>) {
  return createElement(ExternalLink, {
    ...props,
    className: cx(styles.primaryLink, props.className),
  });
}

function Counter({
  over,
  className,
  ...rest
}: ComponentProps<'span'> & { over: 'over' | 'within' }) {
  return createElement('span', {
    ...rest,
    className: cx(styles.counter, className),
    'data-over': over === 'over' || undefined,
  });
}

function DescriptionInput(props: InputProps) {
  return createElement(FormTextInput, {
    ...props,
    className: cx(styles.descriptionInput, props.className),
  });
}

function SetName(props: InputProps) {
  return createElement(FormTextInput, {
    ...props,
    className: cx(styles.setName, props.className),
  });
}

const DomainHelp = classed('p', styles.domainHelp);
const EmptyValue = classed('span', styles.emptyValue);
const ErrorText = classed('span', styles.errorText);
const Field = classed('div', styles.field);
const FieldTitle = classed('label', styles.fieldTitle);
const Help = classed('span', styles.help);
const InputSlot = classed('div', styles.inputSlot);
const ReadOnly = classed('dl', styles.readOnly);
const Root = classed('div', styles.root);

export {
  ClaimLink,
  Counter,
  DescriptionInput,
  DomainHelp,
  DomainSelect,
  EmptyValue,
  ErrorText,
  Field,
  FieldTitle,
  Help,
  InputSlot,
  primaryLinkStyles,
  ReadOnly,
  Root,
  SetName,
};
