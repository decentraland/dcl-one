import { useId } from 'react';
import { cx } from '/@/lib/cx';
import Dialog from '@ui/atoms/Dialog';
import IconButton from '@ui/atoms/IconButton';
import { CloseRoundedIcon } from '@ui/atoms/icons-material';
import { Tab, Tabs } from '@ui/atoms/Tabs';
import Text from '@ui/atoms/Text';

import { t } from '/@/modules/store/translation/utils';

import './styles.css';

export type Props<T> = {
  open: boolean;
  onClose: () => void;
  title: string;
  icon?: React.ReactNode;
  tabs: { value: T; label: string }[];
  showTabs?: boolean;
  activeTab: T;
  onTabClick: (tab: T) => void;
  children: React.ReactNode;
  className?: string;
  orientation?: 'horizontal' | 'vertical';
};

function TabsModal<T>({
  open,
  onClose,
  title,
  icon,
  tabs = [],
  showTabs = true,
  activeTab,
  onTabClick,
  children,
  className = '',
  orientation = 'vertical',
}: Props<T>) {
  const titleId = useId();
  const handleChange = (value: string) => {
    const tab = tabs.find(item => String(item.value) === value);
    if (tab) onTabClick(tab.value);
  };

  return (
    <Dialog
      bare
      open={open}
      size="small"
      className="TabsModalPaper"
      backdropClassName="TabsModalBackdrop"
      aria-labelledby={titleId}
    >
      <div className={`TabsModal ${className}`}>
        <div className="TabsModalHeader">
          <div className="TabsModalTitle">
            {icon}
            <Text
              id={titleId}
              variant="h6"
            >
              {title}
            </Text>
          </div>
          <IconButton
            aria-label={t('modal.close')}
            onClick={onClose}
          >
            <CloseRoundedIcon
              fontSize="medium"
              style={{ color: 'var(--white)' }}
            />
          </IconButton>
        </div>
        <div
          className={cx(
            'TabsModalLayout',
            orientation === 'horizontal' ? 'Horizontal' : 'Vertical',
          )}
        >
          {showTabs && (
            <Tabs
              className="TabsList"
              scrollable
              orientation={orientation}
              value={String(activeTab)}
              onChange={handleChange}
            >
              {tabs.map(tab => (
                <Tab
                  key={`tab-${tab.value}`}
                  className="TabsModalTab"
                  value={String(tab.value)}
                  label={tab.label}
                />
              ))}
            </Tabs>
          )}
          <div className="TabsModalContent">{children}</div>
        </div>
      </div>
    </Dialog>
  );
}

export { TabsModal };
