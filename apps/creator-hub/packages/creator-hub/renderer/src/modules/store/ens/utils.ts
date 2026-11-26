import { ENSProvider, USER_PERMISSIONS, type ContributableDomain } from './types';

export function isValidENSName(name: string): boolean {
  return /^[a-zA-Z0-9]+(\.[a-zA-Z0-9]+)?\.eth$/.test(name);
}

export function getEnsProvider(subdomain: string): ENSProvider {
  if (subdomain.endsWith('.dcl.eth')) {
    return ENSProvider.DCL;
  }

  return ENSProvider.ENS;
}

/** The Worlds the wallet can stream to but not publish to, from its contributable domains. */
export function getStreamOnlyWorldNames(domains: ContributableDomain[]): string[] {
  return domains
    .filter(
      domain =>
        domain.user_permissions.includes(USER_PERMISSIONS.STREAMING) &&
        !domain.user_permissions.includes(USER_PERMISSIONS.DEPLOYMENT),
    )
    .map(domain => domain.name.toLowerCase());
}
