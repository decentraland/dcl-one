import { useId, useState, type ComponentType } from "react";

import {
  GeneralCalendar,
  GeneralDownArrow,
  GeneralGift,
  GeneralLive,
  GeneralMarketplace,
  GeneralPublic,
  GeneralTip,
  GeneralVote,
  MenusFriends,
  ShopCredits,
  type IconProps,
  Switch,
} from "@dcl/one-ui";

import {
  isTypeEmailEnabled,
  SUBSCRIPTION_GROUPS,
  type SubscriptionDetails,
  type SubscriptionGroupKey,
} from "@/data/lib/account/notifications";

import { GROUP_LABELS, TYPE_LABELS } from "./copy";
import styles from "./NotificationGroupAccordion.module.css";

/** The design system's closest single-colour icon to each of decentraland.org's notification category icons. */
const GROUP_ICONS: Record<SubscriptionGroupKey, ComponentType<IconProps>> = {
  marketplace: GeneralMarketplace,
  worlds: GeneralPublic,
  credits: ShopCredits,
  streaming: GeneralLive,
  events: GeneralCalendar,
  rewards: GeneralGift,
  dao: GeneralVote,
  tips: GeneralTip,
  referral: MenusFriends,
};

type NotificationGroupAccordionProps = {
  group: SubscriptionGroupKey;
  details: SubscriptionDetails;
  disabled: boolean;
  onToggleType: (type: string, enabled: boolean) => void;
};

/** One notification category: a header that opens the list of its types, each with its email switch. */
export function NotificationGroupAccordion({
  group,
  details,
  disabled,
  onToggleType,
}: NotificationGroupAccordionProps) {
  const contentId = useId();
  const [expanded, setExpanded] = useState(false);
  const Icon = GROUP_ICONS[group];

  return (
    <div className={styles.root}>
      <h3 className={styles.heading}>
        <button
          type="button"
          className={styles.header}
          aria-expanded={expanded}
          aria-controls={contentId}
          onClick={() => setExpanded((open) => !open)}
        >
          <span className={styles.label}>
            <Icon className={styles.icon} size={20} aria-hidden="true" />
            {GROUP_LABELS[group]}
          </span>
          <GeneralDownArrow className={styles.chevron} size={20} aria-hidden="true" />
        </button>
      </h3>
      <div id={contentId} className={styles.content} hidden={!expanded}>
        {expanded ? (
          <ul className={styles.types}>
            {SUBSCRIPTION_GROUPS[group].map((type) => (
              <li key={type}>
                <Switch
                  className={styles.typeSwitch}
                  label={TYPE_LABELS[type] ?? type}
                  checked={isTypeEmailEnabled(details, type)}
                  disabled={disabled}
                  onCheckedChange={(enabled) => onToggleType(type, enabled)}
                />
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </div>
  );
}
