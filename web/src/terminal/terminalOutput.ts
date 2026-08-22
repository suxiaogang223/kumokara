export const TERMINAL_BINARY_HEADER_BYTES = 24
const MAX_SAFE_SEQUENCE_HIGH = 0x1fffff
const DEFAULT_MAX_BATCH_BYTES = 256 * 1024

export interface TerminalOutputFrame {
  sessionId: string
  seq: number
  data: Uint8Array
}

export function encodeTerminalInputFrame(sessionId: string, data: Uint8Array): Uint8Array {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(sessionId)) {
    throw new Error('Invalid terminal session UUID')
  }

  const frame = new Uint8Array(TERMINAL_BINARY_HEADER_BYTES + data.byteLength)
  const hex = sessionId.replace(/-/g, '')
  for (let index = 0; index < 16; index++) {
    frame[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16)
  }
  // Bytes 16..24 are reserved for a future client sequence. They remain zero.
  frame.set(data, TERMINAL_BINARY_HEADER_BYTES)
  return frame
}

export function decodeTerminalOutputFrame(buffer: ArrayBuffer): TerminalOutputFrame {
  if (buffer.byteLength < TERMINAL_BINARY_HEADER_BYTES) {
    throw new Error(`Terminal output frame is shorter than ${TERMINAL_BINARY_HEADER_BYTES} bytes`)
  }

  const bytes = new Uint8Array(buffer)
  const view = new DataView(buffer)
  const sequenceHigh = view.getUint32(16)
  if (sequenceHigh > MAX_SAFE_SEQUENCE_HIGH) {
    throw new Error('Terminal output sequence exceeds JavaScript safe integer range')
  }

  const hex = Array.from(bytes.subarray(0, 16), (byte) => byte.toString(16).padStart(2, '0')).join('')
  const sessionId = [
    hex.slice(0, 8),
    hex.slice(8, 12),
    hex.slice(12, 16),
    hex.slice(16, 20),
    hex.slice(20),
  ].join('-')
  const seq = sequenceHigh * 0x1_0000_0000 + view.getUint32(20)

  return {
    sessionId,
    seq,
    data: bytes.subarray(TERMINAL_BINARY_HEADER_BYTES),
  }
}

type TerminalWriter = (data: Uint8Array, callback: () => void) => void
type FrameScheduler = (callback: FrameRequestCallback) => number

interface QueuedChunk {
  seq: number
  data: Uint8Array
}

export interface TerminalWriteQueueOptions {
  maxBatchBytes?: number
  requestFrame?: FrameScheduler
  cancelFrame?: (handle: number) => void
  onWriteComplete?: (seq: number) => void
}

/**
 * Coalesces terminal chunks once per animation frame and allows only one
 * xterm write at a time. The write callback is the backpressure boundary:
 * another batch is not submitted until xterm has parsed the previous one.
 */
export class TerminalWriteQueue {
  private readonly pending: QueuedChunk[] = []
  private readonly maxBatchBytes: number
  private readonly requestFrame: FrameScheduler
  private readonly cancelFrame: (handle: number) => void
  private readonly onWriteComplete?: (seq: number) => void
  private pendingBytes = 0
  private frame: number | null = null
  private writing = false
  private disposed = false

  constructor(
    private readonly write: TerminalWriter,
    options: TerminalWriteQueueOptions = {},
  ) {
    this.maxBatchBytes = options.maxBatchBytes ?? DEFAULT_MAX_BATCH_BYTES
    if (this.maxBatchBytes <= 0) throw new Error('maxBatchBytes must be positive')
    this.requestFrame = options.requestFrame ?? window.requestAnimationFrame.bind(window)
    this.cancelFrame = options.cancelFrame ?? window.cancelAnimationFrame.bind(window)
    this.onWriteComplete = options.onWriteComplete
  }

  enqueue(seq: number, data: Uint8Array) {
    if (this.disposed || data.byteLength === 0) return
    this.pending.push({ seq, data })
    this.pendingBytes += data.byteLength
    this.schedule()
  }

  get queuedBytes() {
    return this.pendingBytes
  }

  clear() {
    this.pending.length = 0
    this.pendingBytes = 0
    if (this.frame !== null) this.cancelFrame(this.frame)
    this.frame = null
  }

  dispose() {
    this.disposed = true
    this.clear()
  }

  private schedule() {
    if (this.disposed || this.writing || this.frame !== null || this.pending.length === 0) return
    this.frame = this.requestFrame(() => this.flush())
  }

  private flush() {
    this.frame = null
    if (this.disposed || this.writing || this.pending.length === 0) return

    const chunks: QueuedChunk[] = []
    let batchBytes = 0
    while (this.pending.length > 0) {
      const next = this.pending[0]
      if (chunks.length > 0 && batchBytes + next.data.byteLength > this.maxBatchBytes) break
      chunks.push(this.pending.shift()!)
      batchBytes += next.data.byteLength
    }
    this.pendingBytes -= batchBytes

    const batch = new Uint8Array(batchBytes)
    let offset = 0
    for (const chunk of chunks) {
      batch.set(chunk.data, offset)
      offset += chunk.data.byteLength
    }
    const lastSeq = chunks[chunks.length - 1].seq
    this.writing = true
    this.write(batch, () => {
      this.writing = false
      if (this.disposed) return
      this.onWriteComplete?.(lastSeq)
      this.schedule()
    })
  }
}
