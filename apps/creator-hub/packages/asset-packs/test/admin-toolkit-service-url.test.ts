import { describe, expect, it } from 'vitest';
import type { PBRealmInfo } from '~system/Runtime';
import { serviceUrl } from '../src/admin-toolkit-ui/fetch-utils';

const realm = (baseUrl: string, networkId = 1) => ({ baseUrl, networkId }) as PBRealmInfo;

describe('serviceUrl', () => {
  it("gives a Decentraland realm the service's own host, on its network's domain", () => {
    expect(
      serviceUrl('comms-gatekeeper', '/scene-admin', realm('https://peer.decentraland.org')),
    ).toBe('https://comms-gatekeeper.decentraland.org/scene-admin');
    expect(
      serviceUrl(
        'cast-presenter-service',
        '/presentations',
        realm('https://worlds-content-server.decentraland.zone/world/x.dcl.eth', 11155111),
      ),
    ).toBe('https://cast-presenter-service.decentraland.zone/presentations');
  });

  it('keeps the Decentraland host while the realm is not known yet', () => {
    expect(serviceUrl('comms-gatekeeper', '/scene-bans', undefined)).toBe(
      'https://comms-gatekeeper.decentraland.org/scene-bans',
    );
  });

  it("gives any other realm the service under the realm's origin", () => {
    expect(
      serviceUrl('comms-gatekeeper', '/scene-stream-access', realm('https://example.org')),
    ).toBe('https://example.org/comms-gatekeeper/scene-stream-access');
    expect(
      serviceUrl('comms-gatekeeper', '/cast/presenters', realm('https://gateway.example.org/peer')),
    ).toBe('https://gateway.example.org/comms-gatekeeper/cast/presenters');
    expect(
      serviceUrl(
        'cast-presenter-service',
        '/presentations',
        realm('http://127.0.0.1:5212', 11155111),
      ),
    ).toBe('http://127.0.0.1:5212/cast-presenter-service/presentations');
  });

  it('does not take a host that only ends in a Decentraland name for one', () => {
    expect(
      serviceUrl('comms-gatekeeper', '/scene-admin', realm('https://notdecentraland.org')),
    ).toBe('https://notdecentraland.org/comms-gatekeeper/scene-admin');
  });
});
