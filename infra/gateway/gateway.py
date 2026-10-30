#!/usr/bin/env python3
"""Render the container gateway from validated runtime settings and the route contract."""
import argparse
import ipaddress
import json
import os
from pathlib import Path
import re
import subprocess


def words(value):
    return [word for word in re.split(r"[\s,]+", value.strip()) if word]


def networks(value):
    result = []
    for word in words(value):
        if not re.fullmatch(r"[0-9a-fA-F:.]+(?:/[0-9]+)?", word):
            raise ValueError("Expected IP addresses or CIDRs without zone IDs")
        result.append(str(ipaddress.ip_network(word, strict=False)))
    return result


def hostname(value):
    if len(value) > 253 or not all(re.fullmatch(r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?", part)
                                  for part in value.split(".")):
        raise ValueError("Expected a lowercase DNS hostname without a scheme, port or path")
    return value


def upstream(value):
    match = re.fullmatch(r"(\[[0-9a-fA-F:]+\]|[a-z0-9.-]+):([0-9]+)", value)
    if not match or not 1 <= int(match[2]) <= 65535:
        raise ValueError("Upstreams must be host:port or [IPv6]:port")
    host, port = match.groups()
    if host.startswith("["):
        ipaddress.IPv6Address(host[1:-1])
    else:
        hostname(host)
    return f"{host}:{int(port)}"


def site_prefixes(files):
    prefixes = set()
    for name in files:
        parts = re.sub(r"\.tsx?$", "", name).split(".")
        if parts[0].startswith(("$", "_")):
            continue
        parts = parts[:2] if parts[0] == "api" else parts[:1]
        parts = [re.split(r"[_$]", part)[0] for part in parts]
        if all(parts):
            prefixes.add("/" + "/".join(parts))
    return sorted(prefixes)


def settings(env, resolv_conf="/etc/resolv.conf"):
    domain = hostname(env.get("GATEWAY_DOMAIN", "localhost"))
    aliases = [hostname(host) for host in words(env.get("GATEWAY_HOST_ALIASES", ""))]
    dns = words(env.get("GATEWAY_RESOLVERS", ""))
    if not dns:
        dns = [parts[1] for line in Path(resolv_conf).read_text().splitlines()
               if len(parts := line.split()) >= 2 and parts[0] == "nameserver"]
    if not dns:
        raise ValueError("No DNS resolver; configure GATEWAY_RESOLVERS")
    resolvers = []
    for value in dns:
        if not re.fullmatch(r"[0-9a-fA-F:.]+", value):
            raise ValueError("Resolvers must be IP addresses without zone IDs")
        address = ipaddress.ip_address(value)
        resolvers.append(f"[{address}]" if address.version == 6 else str(address))
    storage = env.get("GATEWAY_STORAGE_ROOT", "")
    if storage and not re.fullmatch(r"/[a-zA-Z0-9_./-]+", storage):
        raise ValueError("GATEWAY_STORAGE_ROOT must be an absolute path without whitespace")
    return {
        "domain": domain, "hosts": aliases, "resolvers": " ".join(resolvers),
        "apis": [upstream(value) for value in words(env.get("GATEWAY_APIS_UPSTREAMS", "apis:5210"))],
        "sites": [upstream(value) for value in words(env.get("GATEWAY_SITES_UPSTREAMS", "sites:5212"))],
        "admins": networks(env.get("GATEWAY_ADMIN_CIDRS", "")),
        "trusted": networks(env.get("GATEWAY_TRUSTED_PROXY_CIDRS", "")),
        "storage": storage.rstrip("/"),
    }


def render(table, cfg, mime):
    if not cfg["apis"] or not cfg["sites"]:
        raise ValueError("Both API and Sites upstreams are required")
    prefixes = site_prefixes(table["sites"])
    for prefix in prefixes:
        if prefix in ("/ws", "/hot-scenes", "/content") or any(
                prefix == path or prefix.startswith(path + "/") for path in table["internal"]):
            raise ValueError(f"Sites claims a reserved API path: {prefix}")
    domain = re.escape(cfg["domain"])
    host_aliases = {"peer": "", "worlds": "", "abcdn": "/ab-cdn", "abgen-cdn": "/ab-cdn",
                    "asset-bundle-registry-abgen": "/asset-bundle-registry",
                    "rpc-social-service-ea": "/social-rpc", "market": "/shop",
                    "storage": "/world-storage", "comms-gatekeeper-local": "/comms-gatekeeper"}
    alias_moves, settled = [], []
    for name, target in host_aliases.items():
        alias_moves.extend([f'"~^gateway:/{name}/?$" "{target}/";',
                            f'"~^gateway:/{name}(?<gateway_rest>/.*)$" "{target}$gateway_rest";'])
        settled.append(f'"~^{name}:{re.escape(target)}(/|$)" "";'
                       if target else f'"~^{name}:" "";')
    apex_hosts = " ".join(f'{host} "";' for host in [cfg["domain"], *cfg["hosts"]])
    domain_names = " ".join(dict.fromkeys([cfg["domain"], "~^[a-z0-9-]+\\." + domain + "$", *cfg["hosts"]]))
    admin_geo = " ".join(f'{net} "admitted";' for net in cfg["admins"])
    trusted_geo = " ".join(f"{net} 1;" for net in cfg["trusted"])
    real_ip = "\n".join(f"set_real_ip_from {net};" for net in cfg["trusted"])
    blocked = "|".join(re.escape(path.lstrip("/")) for path in table["internal"])
    upstreams = ""
    for name in ("apis", "sites"):
        servers = "\n".join(f"server {host} resolve max_fails=2 fail_timeout=10s;" for host in cfg[name])
        upstreams += f"upstream {name} {{ zone {name} 64k; {servers} keepalive 32; }}\n"

    def location(key, body):
        return f"location {key} {{\n{body}\n}}\n"

    def proxy(target, extra=""):
        # Headers are inherited from http; never add a partial proxy_set_header
        # list in a location, which would discard that trusted set.
        return f"proxy_pass http://{target};\n{extra}"

    def pair(path, target, final=False):
        return (location("= " + path, proxy(target))
                + location(("^~ " if final else "") + path + "/", proxy(target)))

    security_headers = ('add_header X-Content-Type-Options nosniff always;\n'
                        'add_header Referrer-Policy strict-origin-when-cross-origin always;\n'
                        'add_header Cross-Origin-Opener-Policy $coop_fallback always;')
    locations = location("= /_gateway/healthz", 'access_log off; return 200 "ok\\n";')
    # The operator gate is applied in the server rewrite phase, before any
    # location or .data regex, and again after a root alias internal redirect.
    for path in ("/admin", "/server"):
        locations += pair(path, "sites" if path in prefixes else "apis", True)
    for row in table["node"]:
        # The query-based places social cards own only their exact endpoints;
        # /places/place/<position> and /places/world/<name> are Sites pages.
        locations += (location("= " + row["path"], proxy("apis"))
                      if row.get("exact") else pair(row["path"], "apis", True))
    for row in table["aliases"]:
        rewrite = row.get("rewrite") or "^(.*)$ " + row["to"].replace("$uri", "$1")
        # These aliases belong to APIs. Re-selecting a location after the
        # rewrite could send /social/... to the Sites /social page instead.
        locations += location(row["key"], f"rewrite {rewrite} break;\n" + proxy("apis"))
    # Worlds deploys reach this match already rewritten under their host prefix.
    deploy = "|".join(re.escape(path) for path in [*table["deploy"], "/worlds-content-server/entities"])
    locations += location(f'~ "^({deploy})/*$"', proxy("apis",
        "client_max_body_size 200m; client_body_timeout 300s; limit_req zone=deploy burst=4 nodelay;"))
    locations += location("~ ^/comms-gatekeeper/reports/players/[^/]+/evidence/", proxy("apis",
        "client_max_body_size 11m;"))
    locations += location("~ ^(/builder-api)?/v1/(projects/[^/]+/(blobs/[^/]+|manifest|files)|items/[^/]+/files)/*$",
                          proxy("apis", "client_max_body_size 21m;"))
    locations += location('~ "^/explorer/0x[0-9a-fA-F]{40}/(wearables|emotes)$"', proxy("apis"))
    # Keep the Sites .data regex after every API regex.
    locations += location(r"~ \.data$", proxy("sites"))
    for prefix in prefixes:
        if prefix not in ("/admin", "/server"):
            locations += pair(prefix, "sites")
    for path in ("/", "/favicon.ico", "/dcl-sdk-types.json", "/docs/index.md", "/_root.data",
                 "/terms", "/privacy", "/content-policy", "/ethics", "/rewards", "/rewards-terms",
                 "/referral", "/referral-terms", "/security", "/brand"):
        locations += location("= " + path, proxy("sites"))
    for path in ("/assets/", "/fonts/", "/errors/", "/template-bundles/"):
        locations += location("^~ " + path, proxy("sites"))
    locations += location("/content/", proxy("apis", "limit_req zone=content burst=1000 nodelay;"))
    storage = 'return 404;'
    if cfg["storage"]:
        # A host rewrite also marks a request internal in nginx. Require an
        # upstream response header as well; client headers cannot supply it.
        storage = (f'internal; alias {cfg["storage"]}/contents/; sendfile on; etag off;\n'
                   'if ($upstream_http_x_accel_redirect = "") { return 404; }\n'
                   'add_header ETag $upstream_http_etag always;\n'
                   'add_header Access-Control-Allow-Origin "*" always;\n'
                   'add_header Cache-Control "public, max-age=31536000, immutable";\n'
                   + security_headers)
    locations += location("= /__protected_storage", "return 404;")
    locations += location("^~ /__protected_storage/", storage)
    locations += location("/", proxy("apis"))
    # $uri is normalized by nginx (percent decoding, dot segments and repeated
    # slashes). Case-insensitive gates also cover React Router's matching rules.
    return f"""
worker_processes auto;
pid /tmp/gateway/nginx.pid;
error_log /dev/stderr warn;
events {{ worker_connections 1024; }}
http {{
    include {mime};
    server_tokens off;
    access_log /dev/stdout;
    client_body_temp_path /tmp/gateway/client;
    proxy_temp_path /tmp/gateway/proxy;
    fastcgi_temp_path /tmp/gateway/fastcgi;
    uwsgi_temp_path /tmp/gateway/uwsgi;
    scgi_temp_path /tmp/gateway/scgi;
    resolver {cfg["resolvers"]} valid=10s;
    resolver_timeout 5s;
    {upstreams}
    {real_ip}
    real_ip_header X-Forwarded-For;
    real_ip_recursive on;
    geo $realip_remote_addr $trusted_peer {{ default 0; {trusted_geo} }}
    map "$trusted_peer:$http_x_forwarded_proto" $forwarded_scheme {{
        default $scheme;
        "1:https" https;
        "1:http" http;
    }}
    geo $admin_gate {{ default ""; {admin_geo} }}
    map $http_upgrade $connection_keepalive {{ default upgrade; "" ""; }}
    map $upstream_http_access_control_allow_origin $cors_fallback {{
        "" "*";
        default "";
    }}
    map $upstream_http_cross_origin_opener_policy $coop_fallback {{
        "" "same-origin";
        default "";
    }}
    map $host $gateway_subdomain {{
        default "";
        {apex_hosts}
        "~^(?<gateway_sub>[a-z0-9-]+)\\.{domain}$" $gateway_sub;
    }}
    map $gateway_subdomain $gateway_prefix {{
        default /$gateway_subdomain;
        "" "";
        {" ".join(f'{key} "{value}";' for key, value in host_aliases.items())}
    }}
    map "$gateway_subdomain:$uri" $gateway_uri {{
        volatile;
        default "";
        "~^[^:]*:/__protected_storage(/|$)" "";
        {" ".join(alias_moves)}
        "~^gateway:" "";
        {" ".join(settled)}
        "~^([^:]+):/\\1(/|$)" "";
        "~^[^:]+:/$" $gateway_prefix/;
        "~^[^:]+:(?<gateway_path>/.*)$" $gateway_prefix$gateway_path;
    }}
    map "$gateway_subdomain:$request_uri" $original_path {{
        default $request_uri;
        "~^gateway:/[^/?]+(?<gateway_signed>/.*)$" $gateway_signed;
    }}
    map $uri $internal_path {{
        volatile;
        default 0;
        "~*^/({blocked})(/|\\\\.|$)" 1;
    }}
    map $uri $operator_path {{
        volatile;
        default 0;
        "~*^/(admin|server)(\\\\W|$)" 1;
        "~*^/metabase(/|\\\\.|$)" 1;
        "~*^/telemetry/dash/(sql|flag|group/target)(\\\\.data)?/?$" 1;
        "~*^/telemetry/dash/admin(/|\\\\.|$)" 1;
    }}
    map "$operator_path:$admin_gate" $operator_denied {{ volatile; default 0; "1:" 1; }}
    limit_req_zone $binary_remote_addr zone=read:10m rate=30r/s;
    limit_req_zone $binary_remote_addr zone=content:10m rate=100r/s;
    limit_req_zone $binary_remote_addr zone=deploy:10m rate=2r/s;
    limit_req_status 429;
    proxy_http_version 1.1;
    proxy_read_timeout 600s;
    proxy_buffering off;
    proxy_set_header Host $http_host;
    proxy_set_header X-Forwarded-Host $http_host;
    proxy_set_header X-Forwarded-Proto $forwarded_scheme;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header CF-Connecting-IP $remote_addr;
    proxy_set_header True-Client-IP "";
    proxy_set_header Forwarded "";
    proxy_set_header X-Catalyst-Admin-Gate $admin_gate;
    proxy_set_header x-original-path $original_path;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection $connection_keepalive;
    {security_headers}
    add_header Access-Control-Allow-Origin $cors_fallback always;
    server {{
        listen 8080 default_server;
        server_name _;
        location = /_gateway/healthz {{ access_log off; return 200 "ok\\n"; }}
        location / {{ return 404; }}
    }}
    server {{
        listen 8080;
        server_name {domain_names};
        if ($uri ~ "[?]") {{ return 400; }}
        if ($gateway_uri) {{ rewrite ^ $gateway_uri; }}
        if ($internal_path) {{ return 404; }}
        if ($operator_denied) {{ return 403; }}
        client_max_body_size 1m;
        limit_req zone=read burst=60 nodelay;
        {locations}
    }}
}}
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--table", required=True, type=Path)
    parser.add_argument("--nginx", required=True)
    parser.add_argument("--mime", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        text = render(json.loads(args.table.read_text()), settings(os.environ), args.mime)
    except ValueError as error:
        parser.error(str(error))
    folder = Path("/tmp/gateway")
    folder.mkdir(mode=0o700, exist_ok=True)
    conf = folder / "nginx.conf"
    conf.write_text(text)
    command = [args.nginx, "-p", str(folder), "-c", str(conf)]
    subprocess.run([*command, "-t"], check=True)
    if not args.check:
        os.execv(args.nginx, [*command, "-g", "daemon off;"])


if __name__ == "__main__":
    main()
