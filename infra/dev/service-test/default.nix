# NixOS VM test of the per-user development service (foundation-dev service).
# A user installs the systemd user unit from a checkout whose flake, lock,
# shell hook and tooling/dev.py are the real ones; the API and Sites are small
# stand-ins (fixture/). Checks: Rust edits rebuild and restart the API inside
# the running service, Sites edits reach a Vite HMR client, a killed supervisor
# is restarted, and down/up/uninstall drive the unit.
{ pkgs, devShell, inputs }:
let
  root = ../../..;
  pnpm = pkgs.pnpm.override { nodejs-slim = pkgs.nodejs-slim_26; };
  checkout = pkgs.runCommand "foundation-dev-service-fixture" { } ''
    cp -r ${./fixture} $out
    chmod -R u+w $out
    mkdir -p $out/infra/dev $out/tooling
    cp ${root + /flake.nix} $out/flake.nix
    cp ${root + /flake.lock} $out/flake.lock
    cp ${root + /infra/dev/shell-hook.sh} $out/infra/dev/shell-hook.sh
    # nix develop also evaluates packages.<system> while resolving the default shell.
    cp -r ${root + /infra/packages} $out/infra/packages
    cp -r ${root + /infra/containers} $out/infra/containers
    cp ${root + /services/catalyst/rust-toolchain.toml} $out/services/catalyst/rust-toolchain.toml
    cp ${root + /tooling/dev.py} $out/tooling/dev.py
  '';
  sitesDeps = pkgs.fetchPnpmDeps {
    pname = "foundation-dev-service-fixture-sites";
    inherit pnpm;
    src = ./fixture/apps/sites;
    fetcherVersion = 4;
    hash = "sha256-vF15XdxMkyECf4O7uNL57COosQix53V+jCflQygdA+o=";
  };
  store = "/home/dev/.local/share/pnpm-store";
in
pkgs.testers.runNixOSTest {
  name = "foundation-dev-service";
  nodes.machine = {
    virtualisation = {
      memorySize = 6144;
      cores = 4;
      diskSize = 8192;
      writableStore = true;
      # nix develop runs offline: the flake inputs and every shell input are present.
      additionalPaths = [ devShell.inputDerivation sitesDeps ]
        ++ map (input: input.outPath) (builtins.attrValues inputs);
    };
    nix.settings = {
      experimental-features = [ "nix-command" "flakes" ];
      substituters = pkgs.lib.mkForce [ ];
      flake-registry = "";
    };
    users.users.dev = { isNormalUser = true; uid = 1000; linger = true; };
    environment.systemPackages = with pkgs; [ git nodejs_26 sqlite zstd curl ];
  };
  testScript = ''
    import shlex

    def su(command):
        return "su - dev -c " + shlex.quote("export XDG_RUNTIME_DIR=/run/user/1000; cd ~/foundation; " + command)

    def dev(command):
        return machine.succeed(su(command))

    def foundation_dev(command, env=""):
        dev(f"{env} nix develop --command foundation-dev {command}")

    def unit(prop):
        return dev(f"systemctl --user show -P {prop} {name}").strip()

    def api(version):
        machine.wait_until_succeeds(f"curl -sf http://127.0.0.1:5210/about | grep -q '\"{version}\"'", timeout=900)

    machine.wait_for_unit("user@1000.service")
    machine.succeed(
        "cp -r ${checkout} /home/dev/foundation",
        "mkdir -p ${store}",
        "tar --zstd -xf ${sitesDeps}/pnpm-store.tar.zst -C ${store}",
        "if [ -f ${store}/v11/index.db.sql ]; then sqlite3 ${store}/v11/index.db < ${store}/v11/index.db.sql; rm ${store}/v11/index.db.sql; fi",
        "chown -R dev:users /home/dev/foundation /home/dev/.local",
        "chmod -R u+w /home/dev/foundation /home/dev/.local",
    )
    dev("git init -q && git add -A && git -c user.name=t -c user.email=t@t commit -qm fixture")
    # The service manager's environment reaches the unit; pnpm installs from the prefetched store.
    dev("systemctl --user set-environment pnpm_config_store_dir=${store} pnpm_config_offline=true "
        "pnpm_config_trust_lockfile=true pnpm_config_pm_on_fail=ignore pnpm_config_package_import_method=copy")

    with subtest("install starts the stack as a user service"):
        foundation_dev("service install", "FOUNDATION_DEV_PROFILE=offline")
        name = dev("ls ~/.config/systemd/user | grep foundation-dev").strip()
        assert dev(f"systemctl --user is-enabled {name}").strip() == "enabled"
        api("one")
        machine.wait_until_succeeds("curl -sf http://127.0.0.1:5212/ | grep -q app", timeout=300)
        supervisor = unit("MainPID")

    with subtest("a Rust edit rebuilds and restarts the API inside the running service"):
        dev("sed -i 's/\"one\"/\"two\"/' services/catalyst/src/main.rs")
        api("two")
        assert unit("MainPID") == supervisor, "the supervisor itself must not restart"

    with subtest("a Sites edit reaches the Vite HMR client"):
        dev("node ${./hmr.mjs} 5212 > /tmp/hmr.log 2>&1 &")
        machine.wait_until_succeeds("grep -q ready /tmp/hmr.log", timeout=120)
        dev("sed -i s/one/two/ apps/sites/src/main.js")
        machine.wait_until_succeeds("grep -q 'hmr update /src/main.js' /tmp/hmr.log", timeout=120)

    with subtest("a killed supervisor is restarted with one PostgreSQL"):
        machine.succeed(f"kill -9 {supervisor}")
        machine.wait_until_succeeds(su(f"systemctl --user show -P NRestarts {name} | grep -qx 1"), timeout=120)
        api("two")
        assert unit("MainPID") != supervisor
        machine.succeed("test $(pgrep -cf '^postgres -D /home/dev/foundation/.cache/dev/postgres') = 1")

    with subtest("down stops the service without a restart; up starts it"):
        foundation_dev("down")
        machine.fail("curl -sf http://127.0.0.1:5210/about")
        machine.sleep(20)
        assert unit("ActiveState") == "inactive", "down must not trigger Restart=on-failure"
        foundation_dev("up")
        api("two")

    with subtest("uninstall removes the unit and keeps the data"):
        foundation_dev("service uninstall")
        machine.fail(f"test -e /home/dev/.config/systemd/user/{name}")
        machine.fail("curl -sf http://127.0.0.1:5210/about")
        machine.succeed("test -f /home/dev/foundation/.cache/dev/postgres/PG_VERSION")
  '';
}
