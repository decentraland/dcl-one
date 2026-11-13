export const FEEDBACK_MESSAGE_MAX_LENGTH = 4096;

export const FEEDBACK_SOURCES = ['publish-failure', 'logo-menu'] as const;

export type FeedbackSource = (typeof FEEDBACK_SOURCES)[number];

export type FeedbackDelivery = 'sent' | 'queued' | 'unknown';

export type ContactMethod = 'discord' | 'email';

export type FeedbackContact = { method: ContactMethod; value: string };

export type FeedbackReport = {
  message: string;
  contact?: FeedbackContact;
  includeLogs: boolean;
  associatedEventId?: string;
  source: FeedbackSource;
};
