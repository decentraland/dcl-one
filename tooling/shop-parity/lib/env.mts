/**
 * The environment a reference build or the sites preview gets: what tools need to run, nothing that
 * points at a real service or switches an app's mode, so a capture does not depend on who runs it.
 */
export function childEnv(): NodeJS.ProcessEnv {
  return Object.fromEntries(Object.entries(process.env).filter(([k]) => /^(PATH|HOME|USER|TMPDIR|TMP|TEMP|LANG|LC_ALL|SHELL|npm_config_cache|NODE_EXTRA_CA_CERTS|SSL_CERT_FILE)$/.test(k)));
}
