import path from 'path'

export const PROTO_COMPILER_PATH =
  process.env.SDK_PROTOC || path.resolve(process.cwd(), 'node_modules/.bin/protobuf/bin/protoc')
export const TS_PROTO_PLUGIN_PATH = path.resolve(process.cwd(), 'node_modules/.bin/protoc-gen-dcl_ts_proto')
