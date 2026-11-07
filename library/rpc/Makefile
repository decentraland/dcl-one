PROTOC ?= protoc
BIN = ./node_modules/.bin
TS_PROTO_PLUGIN = $(BIN)/protoc-gen-dcl_ts_proto

install:
	pnpm install --frozen-lockfile

# The wire protocol is generated from src/protocol/index.proto; index.ts is not tracked.
src/protocol/index.ts: src/protocol/index.proto
	$(PROTOC) \
		--plugin=$(TS_PROTO_PLUGIN) \
		--dcl_ts_proto_opt=esModuleInterop=true \
		--dcl_ts_proto_out=src/protocol \
		-I src/protocol \
		src/protocol/index.proto

# The codegen tests drive the generated client service definitions.
test/codegen/client.ts: test/codegen/client.proto
	$(PROTOC) \
		--plugin=$(TS_PROTO_PLUGIN) \
		--dcl_ts_proto_opt=esModuleInterop=true,returnObservable=false,outputServices=generic-definitions \
		--dcl_ts_proto_out=test/codegen \
		-I test/codegen \
		test/codegen/client.proto

build: src/protocol/index.ts
	rm -rf dist
	mkdir -p dist/protocol
	cp src/protocol/index.proto dist/protocol/
	$(BIN)/tsc -p tsconfig.json

test: src/protocol/index.ts test/codegen/client.ts
	SIMMULATE_JITTER=false $(BIN)/jest --detectOpenHandles --colors --runInBand $(TESTARGS) $(TEST_FILE)
	SIMMULATE_JITTER=true $(BIN)/jest --detectOpenHandles --colors --runInBand $(TESTARGS) $(TEST_FILE)

.PHONY: install build test
