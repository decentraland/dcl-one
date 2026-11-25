import { getActiveVideoStreams } from '~system/CommsApi';

// The tracks of the Speakers showcase (a per-track source, which only the Unity explorer plays).
export type FlattenedTrack = {
  sid: string;
  identity: string;
  sourceType: number;
  name: string;
  customName: string;
};

export type Participant = {
  name: string;
  identity: string;
  tracks: FlattenedTrack[];
};

export const SOURCE_TYPE_LABELS: Record<number, string> = {
  1: 'Camera',
  2: 'Screen',
  3: 'Presentation',
};

export const getSourceLabel = (sourceType: number) => SOURCE_TYPE_LABELS[sourceType] ?? 'Unknown';

export function isPresentationBot(name: string): boolean {
  return name.startsWith('presentation-bot');
}

export function groupTracksByParticipant(tracks: FlattenedTrack[]): Participant[] {
  const map = new Map<string, Participant>();
  for (const track of tracks) {
    const existing = map.get(track.name);
    if (existing) {
      existing.tracks.push(track);
    } else {
      map.set(track.name, { name: track.name, identity: track.identity, tracks: [track] });
    }
  }
  return Array.from(map.values());
}

function getDisplayName(name: string): string {
  if (isPresentationBot(name)) return 'Presentation';
  return name;
}

export async function getActiveStreams(): Promise<FlattenedTrack[] | undefined> {
  const runtimeStreams = await getActiveVideoStreams({});
  if (!runtimeStreams) return;
  return runtimeStreams.streams.map(
    (stream: { trackSid: string; identity: string; sourceType: number; name?: string }) => {
      const rawName = stream.name ?? stream.identity;
      const displayName = getDisplayName(rawName);
      return {
        sid: stream.trackSid,
        identity: stream.identity,
        sourceType: stream.sourceType,
        name: rawName,
        customName: `${displayName} - ${getSourceLabel(stream.sourceType)}`,
      };
    },
  );
}
