.PHONY: check owners bevy-web bevy-headless catalyst-build catalyst-test sites-install sites-build

NODE_CRATES = --workspace --exclude dcl-scene-cli --exclude dcl-scene-authoring --exclude dcl-scene-build --exclude dcl-preview-tunnel --exclude dcl-preview-tunnel-protocol

check:
	bash services/catalyst/scripts/check-module-dag.sh
	python3 tooling/ui_standards.py

owners:
	python3 tooling/owners.py

bevy-web:
	$(MAKE) -C apps/bevy-explorer web

bevy-headless:
	$(MAKE) -C apps/bevy-explorer headless

catalyst-build:
	cd services/catalyst && cargo build --locked --bin catalyst

catalyst-test:
	cd services/catalyst && cargo test --locked $(NODE_CRATES)

sites-install:
	pnpm --dir apps/ui3 install --frozen-lockfile
	pnpm --dir apps/sites install --frozen-lockfile

sites-build:
	pnpm --dir apps/sites build
