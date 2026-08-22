import { describe, expect, it, vi } from 'vitest'
import {
  decodeTerminalOutputFrame,
  encodeTerminalInputFrame,
  TERMINAL_BINARY_HEADER_BYTES,
  TerminalWriteQueue,
} from './terminalOutput'

function binaryFrame(payload: Uint8Array, seq = 42) {
  const buffer = new ArrayBuffer(TERMINAL_BINARY_HEADER_BYTES + payload.byteLength)
  const bytes = new Uint8Array(buffer)
  bytes.set([
    0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
    0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
  ])
  new DataView(buffer).setBigUint64(16, BigInt(seq))
  bytes.set(payload, TERMINAL_BINARY_HEADER_BYTES)
  return buffer
}

describe('decodeTerminalOutputFrame', () => {
  it('decodes the UUID, sequence, and raw bytes without UTF-8 conversion', () => {
    const frame = decodeTerminalOutputFrame(binaryFrame(new Uint8Array([0xff, 0x00, 0x61])))

    expect(frame.sessionId).toBe('550e8400-e29b-41d4-a716-446655440000')
    expect(frame.seq).toBe(42)
    expect([...frame.data]).toEqual([0xff, 0x00, 0x61])
  })

  it('rejects malformed and unsafe frames', () => {
    expect(() => decodeTerminalOutputFrame(new ArrayBuffer(23))).toThrow(/shorter/)

    const unsafe = binaryFrame(new Uint8Array([1]))
    new DataView(unsafe).setUint32(16, 0x20_0000)
    expect(() => decodeTerminalOutputFrame(unsafe)).toThrow(/safe integer/)
  })
})

describe('encodeTerminalInputFrame', () => {
  it('encodes raw input behind the shared binary header', () => {
    const frame = encodeTerminalInputFrame(
      '550e8400-e29b-41d4-a716-446655440000',
      new Uint8Array([0xff, 0x00, 0x61]),
    )

    expect([...frame.subarray(0, 16)]).toEqual([
      0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
      0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
    ])
    expect([...frame.subarray(16, 24)]).toEqual(new Array(8).fill(0))
    expect([...frame.subarray(24)]).toEqual([0xff, 0x00, 0x61])
  })

  it('rejects a non-UUID session id', () => {
    expect(() => encodeTerminalInputFrame('not-a-uuid', new Uint8Array([1]))).toThrow(/UUID/)
  })
})

describe('TerminalWriteQueue', () => {
  it('batches chunks and waits for xterm to finish before writing again', () => {
    const frames: FrameRequestCallback[] = []
    const completions: Array<() => void> = []
    const writes: number[][] = []
    const completed = vi.fn()
    const queue = new TerminalWriteQueue(
      (data, callback) => {
        writes.push([...data])
        completions.push(callback)
      },
      {
        requestFrame: (callback) => {
          frames.push(callback)
          return frames.length
        },
        cancelFrame: () => {},
        onWriteComplete: completed,
      },
    )

    queue.enqueue(1, new Uint8Array([1, 2]))
    queue.enqueue(2, new Uint8Array([3]))
    expect(frames).toHaveLength(1)
    frames.shift()!(0)
    expect(writes).toEqual([[1, 2, 3]])

    queue.enqueue(3, new Uint8Array([4]))
    expect(frames).toHaveLength(0)
    completions.shift()!()
    expect(completed).toHaveBeenCalledWith(2)
    expect(frames).toHaveLength(1)
    frames.shift()!(0)
    expect(writes).toEqual([[1, 2, 3], [4]])
  })

  it('caps each batch without splitting a PTY chunk', () => {
    const frames: FrameRequestCallback[] = []
    const completions: Array<() => void> = []
    const writes: number[][] = []
    const queue = new TerminalWriteQueue(
      (data, callback) => {
        writes.push([...data])
        completions.push(callback)
      },
      {
        maxBatchBytes: 3,
        requestFrame: (callback) => {
          frames.push(callback)
          return frames.length
        },
        cancelFrame: () => {},
      },
    )

    queue.enqueue(1, new Uint8Array([1, 2]))
    queue.enqueue(2, new Uint8Array([3, 4]))
    frames.shift()!(0)
    expect(writes).toEqual([[1, 2]])
    expect(queue.queuedBytes).toBe(2)
    completions.shift()!()
    frames.shift()!(0)
    expect(writes).toEqual([[1, 2], [3, 4]])
  })
})
