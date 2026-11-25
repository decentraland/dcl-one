PROTOBUF_VERSION = 21.12
UNAME_S := $(shell uname)
UNAME_M := $(shell uname -m)

ifeq ($(UNAME_S),Darwin)
    ifeq ($(UNAME_M),arm64)
        PROTOBUF_ZIP = protoc-$(PROTOBUF_VERSION)-osx-universal_binary.zip
    else
        PROTOBUF_ZIP = protoc-$(PROTOBUF_VERSION)-osx-x86_64.zip
    endif
else ifeq ($(UNAME_S),Linux)
    ifeq ($(UNAME_M),aarch64)
        PROTOBUF_ZIP = protoc-$(PROTOBUF_VERSION)-linux-aarch_64.zip
    else
        PROTOBUF_ZIP = protoc-$(PROTOBUF_VERSION)-linux-x86_64.zip
    endif
else
    PROTOBUF_ZIP = protoc-$(PROTOBUF_VERSION)-win64.zip
endif

# Set CREATOR_HUB_PROTOC to generate with an installed protoc (the Nix shell's protoc 21.12).
# Not PROTOC, which prost-build and other tools read.
PROTOC = $(or $(CREATOR_HUB_PROTOC),node_modules/.bin/protobuf/bin/protoc)
INSPECTOR_PATH = packages/inspector
CH_PATH = packages/creator-hub
ASSET_PACKS_PATH = packages/asset-packs
# The Bevy editor-agent scene: a separate SDK7 project (NOT a workspace) with its
# own node_modules pinned to the engine's companion SDK. The inspector build
# exports it into public/bevy-agent, so it must be installed + built first.
BEVY_AGENT_PATH = packages/inspector/agents/bevy
# One install covers every workspace package; CI installs exactly what the lock records.
PNPM_INSTALL = pnpm install $(if $(CI),--frozen-lockfile)

install:
	$(PNPM_INSTALL)
	make install-protoc
	make install-bevy-agent
	make fetch-devtools

install-asset-packs:
	$(PNPM_INSTALL)

install-inspector:
	$(PNPM_INSTALL)
	make install-protoc
	make install-bevy-agent

install-bevy-agent:
	cd $(BEVY_AGENT_PATH); $(PNPM_INSTALL)

install-creator-hub:
	$(PNPM_INSTALL)
	make fetch-devtools

fetch-devtools:
	node $(CH_PATH)/scripts/fetch-devtools.js

install-skills:
	pnpm dlx skills@1.7.0 experimental_install -y
	mkdir -p .claude/skills
	for s in .agents/skills/*/; do \
		[ -d "$$s" ] || continue; \
		n=$$(basename $$s); \
		[ -e .claude/skills/$$n ] || ln -sfn ../../.agents/skills/$$n .claude/skills/$$n; \
	done
	rm -rf .claude/skills/web-design-guidelines
	ln -s ../../.ai/skills/web-design-guidelines .claude/skills/web-design-guidelines

get-protobuf-version:
	@echo $(PROTOBUF_VERSION)

install-protoc: $(PROTOC)

$(PROTOC):
	mkdir -p node_modules/.bin/protobuf
	@echo "Downloading protoc $(PROTOBUF_VERSION) for $(UNAME_S) $(UNAME_M)..."
	@echo "Target file: $(PROTOBUF_ZIP)"
	rm -f $(PROTOBUF_ZIP)
	curl -fsSL --retry 3 --retry-delay 2 -o $(PROTOBUF_ZIP) \
		https://github.com/protocolbuffers/protobuf/releases/download/v$(PROTOBUF_VERSION)/$(PROTOBUF_ZIP) || \
		(echo "Failed to download protoc. File size: $$(stat -c%s $(PROTOBUF_ZIP) 2>/dev/null || stat -f%z $(PROTOBUF_ZIP) 2>/dev/null || echo 'N/A')"; exit 1)
	@echo "Downloaded file size: $$(stat -c%s $(PROTOBUF_ZIP) 2>/dev/null || stat -f%z $(PROTOBUF_ZIP) 2>/dev/null || echo 'N/A') bytes"
	@file $(PROTOBUF_ZIP) || true
	unzip -o $(PROTOBUF_ZIP) -d node_modules/.bin/protobuf
	rm $(PROTOBUF_ZIP)
	chmod +x $(PROTOC)

protoc: $(PROTOC)
	mkdir -p $(INSPECTOR_PATH)/src/lib/data-layer/proto/gen
	$(PROTOC) \
		--plugin=node_modules/.bin/protoc-gen-dcl_ts_proto \
		--dcl_ts_proto_opt=esModuleInterop=true,returnObservable=false,outputServices=generic-definitions,fileSuffix=.gen,oneof=unions,useMapType=true \
		--dcl_ts_proto_out=$(INSPECTOR_PATH)/src/lib/data-layer/proto/gen \
		--proto_path=$(INSPECTOR_PATH)/src/lib/data-layer/proto \
		$(INSPECTOR_PATH)/src/lib/data-layer/proto/*.proto

build:
	make build-asset-packs
	make build-inspector
	make build-creator-hub

build-asset-packs:
	cd $(ASSET_PACKS_PATH); pnpm run build;

build-inspector:
	make build-bevy-agent
	cd $(INSPECTOR_PATH); pnpm run build;

build-bevy-agent: install-bevy-agent
	cd $(BEVY_AGENT_PATH); pnpm run build;

build-creator-hub:
	cd $(CH_PATH); pnpm run build;

init:
	make clean
	make install
	make protoc
	make build

lint:
	pnpm run lint

lint-fix:
	pnpm run lint:fix

lint-packages:
	node scripts/check-dep-versions.js

typecheck:
	pnpm run typecheck

test:
	pnpm run test

validate-asset-packs:
	cd $(ASSET_PACKS_PATH); pnpm run validate

upload-asset-packs:
	cd $(ASSET_PACKS_PATH); pnpm run upload


test-e2e:
	make test-inspector-e2e
	make test-creator-hub-e2e

test-inspector-e2e:
	cd $(INSPECTOR_PATH)/; pnpm run test:e2e

test-creator-hub-e2e:
	cd $(CH_PATH); pnpm run test:e2e

# CI variant: runs test:e2e:ci, which skips the `pnpm run build` that test:e2e runs
# but still runs electron-builder to package the app the suite launches.
# Contract: the caller MUST ensure packages/creator-hub/{main,preload,renderer}/dist
# already exist. CI provides them via the download-build action (built once by the
# build job); locally, run `make build-creator-hub` first. Without those the packaging
# step has nothing to bundle and the suite fails.
test-creator-hub-e2e-ci:
	cd $(CH_PATH); pnpm run test:e2e:ci

test-creator-hub-e2e-ci-live:
	cd $(CH_PATH); pnpm run test:e2e:ci:live

format:
	pnpm run format

format-fix:
	pnpm run format:fix

deep-clean:
	rm -rf node_modules/ \
		$(INSPECTOR_PATH)/node_modules/ \
		$(CH_PATH)/node_modules/ \
		$(ASSET_PACKS_PATH)/node_modules/ \
		$(INSPECTOR_PATH)/src/lib/data-layer/proto/gen/ \
	make clean

clean:
	@echo "> Cleaning all folders"
	@rm -rf coverage/
	@rm -rf $(INSPECTOR_PATH)/public/*.js $(INSPECTOR_PATH)/public/*.d.ts $(INSPECTOR_PATH)/public/*.map $(INSPECTOR_PATH)/public/*.css
	@rm -rf $(CH_PATH)/main/dist/
	@rm -rf $(CH_PATH)/preload/dist/
	@rm -rf $(CH_PATH)/renderer/dist/
	@rm -rf $(ASSET_PACKS_PATH)/dist/
	@rm -rf $(ASSET_PACKS_PATH)/bin/
