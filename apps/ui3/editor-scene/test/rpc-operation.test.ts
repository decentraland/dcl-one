import assert from 'node:assert/strict'
import { test } from 'node:test'
import { runRpcOperation, type RpcOutcome } from '../src/rpc-operation'
import { createEditorScheduler, runEditorTick } from '../src/page-ui-scheduler'

test('a timed-out write replies promptly but cannot overlap the next queued edit', async () => {
  let finish!: () => void
  let expire!: (error: Error) => void
  const writing = new Promise<void>(resolve => { finish = resolve })
  const deadline = new Promise<never>((_, reject) => { expire = reject })
  const replies: RpcOutcome[] = []
  const order: string[] = []
  const scheduler = createEditorScheduler<string, never>({
    intervalMs: 1, recover: async () => {},
    handle: async message => {
      order.push(message)
      if (message === 'restore') await runRpcOperation(
        () => writing.then(() => { order.push('restore finished') }),
        () => deadline, reply => replies.push(reply)
      )
    }
  })
  scheduler.enqueue('restore')
  scheduler.enqueue('next edit')
  const draining = scheduler.run()
  expire(new Error('deadline'))
  await new Promise(resolve => setImmediate(resolve))
  assert.deepEqual(replies, [{ ok: false, error: 'Error: deadline' }])
  assert.deepEqual(order, ['restore'])
  finish()
  await draining
  assert.deepEqual(order, ['restore', 'restore finished', 'next edit'])
  assert.equal(replies.length, 1, 'late completion cannot report success after failure')
})

test('successful calls and synchronous/asynchronous failures reply once', async () => {
  const replies: RpcOutcome[] = []
  await runRpcOperation(() => 42, null, reply => replies.push(reply))
  await runRpcOperation(() => { throw new Error('sync') }, null, reply => replies.push(reply))
  await runRpcOperation(() => Promise.reject(new Error('async')), null, reply => replies.push(reply))
  assert.deepEqual(replies, [{ ok: true, result: 42 }, { ok: false, error: 'Error: sync' }, { ok: false, error: 'Error: async' }])
})

test('a failed drag checkpoint still samples and recovers the replacement on the next tick', async () => {
  const order: string[] = []
  let healthy = false
  let now = 1
  const scheduler = createEditorScheduler<never, string>({
    intervalMs: 1,
    handle: async () => {},
    recover: async runtime => { order.push(`recover ${runtime}`); healthy = true }
  })
  const notify = async () => {
    if (!healthy) { order.push('checkpoint failed'); throw new Error('runtime replaced during drag') }
    order.push('drag acknowledged')
  }
  const sample = () => scheduler.sample(now++, async () => { order.push('sample'); return 'replacement' })
  await assert.rejects(runEditorTick(scheduler.run, notify, sample), /runtime replaced during drag/)
  assert.deepEqual(order, ['checkpoint failed', 'sample'])
  await new Promise(resolve => setImmediate(resolve))
  await runEditorTick(scheduler.run, notify, sample)
  assert.deepEqual(order, ['checkpoint failed', 'sample', 'recover replacement', 'drag acknowledged', 'sample'])
})

test('a scheduler failure samples the runtime without sending a successful notification', async () => {
  const order: string[] = []
  await assert.rejects(runEditorTick(
    async () => { throw new Error('scene recovery still pending') },
    () => { order.push('ack') },
    () => { order.push('sample') }
  ), /scene recovery still pending/)
  assert.deepEqual(order, ['sample'])
})
