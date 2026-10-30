BIN="./node_modules/.bin"
BUF_VERSION=1.8.0
OS = $(shell uname -s)
OS_ARCH = $(shell uname -m)
# Wire compatibility is checked against this repository's main branch: its proto tree is
# exported with git archive (no clone, no LFS smudge, works from a worktree).
BUF_AGAINST_REF ?= origin/main
BUF_AGAINST_DIR = .buf-against

node_modules/.bin/buf:
	curl -sSL https://github.com/bufbuild/buf/releases/download/v$(BUF_VERSION)/buf-$(OS)-$(OS_ARCH) -o node_modules/.bin/buf
	chmod +x node_modules/.bin/buf

buf-lint: node_modules/.bin/buf
	./node_modules/.bin/buf lint proto/

buf-build: node_modules/.bin/buf
	./node_modules/.bin/buf build proto/

buf-breaking: node_modules/.bin/buf
	@rm -rf $(BUF_AGAINST_DIR) && mkdir -p $(BUF_AGAINST_DIR)
	@if git -C ../.. cat-file -e '$(BUF_AGAINST_REF):library/protocol/proto' 2>/dev/null; then \
	  git -C ../.. archive '$(BUF_AGAINST_REF)' library/protocol/proto | tar -x -C $(BUF_AGAINST_DIR) --strip-components=2; \
	  ./node_modules/.bin/buf breaking proto/ --against $(BUF_AGAINST_DIR)/proto; \
	else \
	  echo "$(BUF_AGAINST_REF) has no library/protocol/proto; nothing to compare against"; \
	fi

test: buf-lint
	bash scripts/test.sh

all: buf-lint buf-build test

install:
	npm i
	rm -rf proto/google || true
	cp -r node_modules/protobufjs/google proto/google

list-components-ids:
	@bash scripts/list-components-ids.sh

check-component-id:
	@bash scripts/check-component-id.sh $(ID)
