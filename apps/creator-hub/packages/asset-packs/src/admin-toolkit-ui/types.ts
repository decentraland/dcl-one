import type { Entity } from '@dcl/ecs';
import type { FlattenedTrack, Participant } from './VideoControl/api';
import type { ASK } from './cast';
import type { SceneAdmin } from './ModerationControl';

// Tab enum for type safety
export enum TabType {
  NONE = 'None',
  VIDEO_CONTROL = 'VideoControl',
  SMART_ITEMS_CONTROL = 'SmartItemsControl',
  TEXT_ANNOUNCEMENT_CONTROL = 'TextAnnouncementControl',
  MODERATION_CONTROL = 'ModerationControl',
}

export type SelectedSmartItem = { visible: boolean; selectedAction: string };

export type State = {
  adminToolkitUiEntity: Entity;
  panelOpen: boolean;
  activeTab: TabType;
  videoControl: {
    selectedVideoPlayer: number | undefined;
    selectedTab: 'video-url' | 'live' | 'dcl-cast' | undefined;
    /** The confirmation that is open. */
    ask: keyof typeof ASK | undefined;
    /** The stream key is in clear until this time (ms). */
    keyShownUntil: number;
    /** @deprecated Not set. Kept for a scene's own panel written before `SharePresentationModal` read the key itself. */
    dclCast?: { streamingKey: string };
    participants: Participant[];
    showcase: {
      show: boolean;
      /** @deprecated Not set. A scene's own panel passes `selectedTrack(engine)` to the showcase. */
      activeTrackSid?: string;
      onSelectTrack: ((track: FlattenedTrack) => void) | undefined;
      onSetDefault: (() => void) | undefined;
      onClose: (() => void) | undefined;
    };
    sharePresentation: {
      show: boolean;
      onClose: (() => void) | undefined;
    };
  };
  smartItemsControl: {
    selectedSmartItem: number | undefined;
    smartItems: Map<Entity, SelectedSmartItem>;
  };
  textAnnouncementControl: {
    entity: Entity | undefined;
    text: string | undefined;
    messageRateTracker: Map<string, number>;
    announcements: {
      entity: Entity;
      timestamp: number;
    }[];
    maxAnnouncements: number;
  };
  moderationControl: {
    showModalAdminList: boolean;
    adminToRemove: SceneAdmin | undefined;
    showModalBanList: boolean;
    unbanMessage: string | undefined;
  };
};
