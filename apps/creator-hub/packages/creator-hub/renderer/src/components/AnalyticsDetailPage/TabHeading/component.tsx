import Text from '@ui/atoms/Text';

import './styles.css';

type Props = {
  title: string;
  description: string;
};

export function TabHeading({ title, description }: Props) {
  return (
    <div className="TabHeading">
      <Text
        variant="h5"
        className="Title"
      >
        {title}
      </Text>
      <Text
        variant="body1"
        className="Description"
      >
        {description}
      </Text>
    </div>
  );
}
