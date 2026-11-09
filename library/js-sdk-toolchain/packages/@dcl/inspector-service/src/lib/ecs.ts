// The `@dcl/ecs` this bundle runs on, for the Node tooling that feeds or reads its engines
// (`dcl-scene-cli`'s data-layer host and composite tools): one copy of the ecs, so the entities,
// components and byte buffers those tools create are the ones `createDataLayerHost` and the
// composite dumpers expect. The two serialization helpers the package does not export from its
// entry point are added.
export * from '@dcl/ecs';
export { ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer';
export { readMessage } from '@dcl/ecs/dist/serialization/crdt/message';
