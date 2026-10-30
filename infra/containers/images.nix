{ pkgs, catalyst, sites, gateway, revision }:
let
  inherit (pkgs) lib;
  accounts = pkgs.runCommand "foundation-container-accounts" { } ''
    mkdir -p "$out/etc"
    printf 'root:x:0:0:root:/root:/sbin/nologin\nfoundation:x:1000:1000:Foundation:/data:/sbin/nologin\n' > "$out/etc/passwd"
    printf 'root:x:0:\nfoundation:x:1000:\n' > "$out/etc/group"
    printf 'hosts: files dns\npasswd: files\ngroup: files\n' > "$out/etc/nsswitch.conf"
  '';
  image = name: package: config: pkgs.dockerTools.buildLayeredImage {
    inherit name;
    tag = "nix";
    contents = [ package pkgs.cacert accounts ];
    # Give each runtime store path its own stable layer, including Sites'
    # production dependencies and docs. Avoid fromImage: dockerTools would
    # include the runtime closure again on top of that base image.
    maxLayers = 120;
    extraCommands = ''mkdir -p tmp data; chmod 1777 tmp'';
    fakeRootCommands = ''chown 1000:1000 data'';
    config = {
      User = "1000:1000";
      WorkingDir = "/data";
      Env = [ "HOME=/data" "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt" ];
      Labels = {
        "org.opencontainers.image.source" = "https://github.com/decentraland/dcl-one";
        "org.opencontainers.image.revision" = revision;
        "org.opencontainers.image.licenses" = "AGPL-3.0";
      };
    } // config;
  };
in {
  image-gateway = image "foundation-gateway" gateway {
    Entrypoint = [ "${lib.getExe gateway}" ];
    Env = [
      "HOME=/data" "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      "PATH=${lib.makeBinPath [ pkgs.python3 pkgs.nginx ]}"
    ];
    ExposedPorts."8080/tcp" = { };
    StopSignal = "SIGQUIT";
  };
  image-apis = image "foundation-apis" catalyst {
    Entrypoint = [ "${lib.getExe catalyst}" ];
    Cmd = [ "serve" ];
    Env = [
      "HOME=/data" "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      "HTTP_SERVER_HOST=0.0.0.0" "HTTP_SERVER_PORT=5210"
      "STORAGE_ROOT_FOLDER=/data/content" "AB_CDN_CACHE_DIR=/data/ab-cdn"
      "PROFILE_IMAGES_CACHE_DIR=/data/profile-images" "BVIMPOSTERS_STORE_ROOT=/data/imposters"
    ];
    ExposedPorts."5210/tcp" = { };
  };
  image-sites = image "foundation-sites" sites {
    Entrypoint = [ "${lib.getExe sites}" ];
    Env = [
      "HOME=/data" "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      "PATH=${lib.makeBinPath [ pkgs.nodejs-slim_26 ]}:/bin"
      "HOST=0.0.0.0" "PORT=5212" "NODE_ENV=production" "SITES_STATE_DIR=/data"
    ];
    ExposedPorts."5212/tcp" = { };
  };
}
