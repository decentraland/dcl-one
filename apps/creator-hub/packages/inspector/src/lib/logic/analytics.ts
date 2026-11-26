import { Analytics as SegmentAnalytics } from '@segment/analytics-node';
import { randomUuid } from '../utils/uuid';
import { getConfig } from './config';

export enum Event {
  ADD_ITEM = 'Add Item',
  ADD_COMPONENT = 'Add Component',
  REMOVE_COMPONENT = 'Remove Component',
  SEARCH_ITEM = 'Search Item',
  SWITCH_BUILDER_MODE = 'Switch Builder Mode',
  SET_GROUND = 'Set Ground',
  LOCK = 'Lock',
  HIDE = 'Hide',
  CREATE_CUSTOM_ITEM = 'Create Custom Item',
  DELETE_CUSTOM_ITEM = 'Delete Custom Item',
  RENAME_CUSTOM_ITEM = 'Rename Custom Item',
  CREATE_TAG = 'Create Tags',
  ASSIGN_TAGS = 'Assign Tags to Entity',
  CREATE_SCRIPT = 'Create Script',
  ADD_SCRIPT = 'Add Script',
  REMOVE_SCRIPT = 'Remove Script',
  RELOAD_SCRIPTS = 'Reload Scripts',
  EDIT_SCRIPT = 'Edit Script',
  OPEN_UI_EDITOR = 'Open UI Editor',
  ADD_UI_LOGIC = 'Add UI Logic',
  CREATE_UI = 'Create UI',
  NEST_UI_COMPONENT = 'Nest UI Component',
  EDIT_UI_PROPERTY = 'Edit UI Property',
  UNDO = 'Undo',
  REDO = 'Redo',
  DUPLICATE_ENTITY = 'Duplicate Entity',
  DELETE_ENTITY = 'Delete Entity',
  ADD_SMART_ITEM_ACTION = 'Add Smart Item Action',
  ADD_SMART_ITEM_TRIGGER = 'Add Smart Item Trigger',
}

export type HistorySource = 'shortcut' | 'toolbar';
export type EntityActionSource = 'shortcut' | 'context_menu';

export type Events = {
  [Event.ADD_ITEM]: {
    itemId?: string;
    itemName: string;
    itemPath: string;
    isSmart: boolean;
    isCustom: boolean;
  };
  [Event.ADD_COMPONENT]: {
    componentName: string;
    itemId?: string;
    itemPath: string;
  };
  [Event.REMOVE_COMPONENT]: {
    componentName: string;
    itemId?: string;
    itemPath?: string;
  };
  [Event.SEARCH_ITEM]: {
    keyword: string;
    itemsFound: number;
    category?: string;
  };
  [Event.SWITCH_BUILDER_MODE]: {
    itemId: string;
    itemName: string;
    isAdvancedView: boolean;
  };
  [Event.SET_GROUND]: {
    itemId?: string;
    itemName: string;
    itemPath: string;
  };
  [Event.LOCK]: {
    value: boolean;
  };
  [Event.HIDE]: {
    value: boolean;
  };
  [Event.CREATE_CUSTOM_ITEM]: {
    itemId: string;
    itemName: string;
    resourceCount: number;
    isSmart: boolean;
  };
  [Event.DELETE_CUSTOM_ITEM]: {
    itemId: string;
  };
  [Event.RENAME_CUSTOM_ITEM]: {
    itemId: string;
  };
  [Event.CREATE_TAG]: {
    tagName: string;
  };
  [Event.ASSIGN_TAGS]: {
    tagsName: string;
    itemId: string;
    itemPath: string;
  };
  [Event.CREATE_SCRIPT]: {
    scriptPath: string;
    scriptName: string;
  };
  [Event.ADD_SCRIPT]: {
    scriptPath: string;
    scriptName: string;
  };
  [Event.REMOVE_SCRIPT]: {
    scriptPath: string;
    scriptName: string;
  };
  [Event.RELOAD_SCRIPTS]: {
    scriptsCount: number;
  };
  [Event.EDIT_SCRIPT]: {
    scriptPath: string;
    scriptName: string;
  };
  [Event.OPEN_UI_EDITOR]: Record<string, never>;
  [Event.ADD_UI_LOGIC]: {
    logicType: 'variable' | 'event' | 'input';
  };
  [Event.CREATE_UI]: Record<string, never>;
  [Event.NEST_UI_COMPONENT]: Record<string, never>;
  [Event.EDIT_UI_PROPERTY]: {
    property: string;
    interactionLayer?: 'base' | 'hover' | 'press' | 'active';
  };
  [Event.UNDO]: {
    source: HistorySource;
  };
  [Event.REDO]: {
    source: HistorySource;
  };
  [Event.DUPLICATE_ENTITY]: {
    entitiesCount: number;
    source: EntityActionSource;
  };
  [Event.DELETE_ENTITY]: {
    entitiesCount: number;
    source: EntityActionSource;
  };
  [Event.ADD_SMART_ITEM_ACTION]: {
    actionType: string;
    itemPath?: string;
  };
  [Event.ADD_SMART_ITEM_TRIGGER]: {
    triggersCount: number;
    itemPath?: string;
  };
};

const noopAnalytics = {
  track() {},
};

class Analytics {
  private static instance: Analytics | null = null;
  private analytics: SegmentAnalytics | { track(): void } = noopAnalytics;
  private userId: string = randomUuid();
  private appId: string | undefined = undefined;

  constructor() {
    if (Analytics.instance) {
      return Analytics.instance;
    }

    const config = getConfig();

    if (config.segmentKey) {
      this.analytics = new SegmentAnalytics({ writeKey: config.segmentKey });

      if (config.segmentUserId) {
        this.userId = config.segmentUserId;
      }

      if (config.segmentAppId) {
        this.appId = config.segmentAppId;
      }

      this.analytics.identify({
        userId: this.userId,
        traits: this.getTraits(),
      });
    }

    Analytics.instance = this;
  }

  getAnalytics() {
    return this.analytics;
  }

  getTraits() {
    let traits: Record<string, unknown> = {};

    if (this.appId) {
      traits = {
        ...traits,
        appId: this.appId,
      };
    }

    return traits;
  }

  getBaseProperties() {
    const config = getConfig();

    let baseProperties = {};

    if (config.projectId) {
      baseProperties = {
        ...baseProperties,
        projectId: config.projectId,
      };
    }

    return baseProperties;
  }

  track<T extends keyof Events>(eventName: T, eventProps: Events[T]) {
    const trackInfo = {
      event: eventName,
      userId: this.userId,
      properties: {
        ...this.getTraits(),
        ...this.getBaseProperties(),
        ...eventProps,
      },
    };

    this.analytics.track(trackInfo);
  }
}

export const analytics = new Analytics();
