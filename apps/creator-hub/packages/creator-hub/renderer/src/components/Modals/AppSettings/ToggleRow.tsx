import { useId } from 'react';
import Text from '@ui/atoms/Text';
import Toggle from '@ui/atoms/Toggle';

/** A switch with its text at the right. A click on the text also flips the switch. */
export function ToggleRow({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  const labelId = useId();
  return (
    <label className="SettingsToggleRow">
      <Toggle
        checked={checked}
        ariaLabelledBy={labelId}
        onChange={onChange}
      />
      <Text
        as="span"
        variant="body1"
        id={labelId}
      >
        {label}
      </Text>
    </label>
  );
}
