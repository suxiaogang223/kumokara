import { bench, describe } from 'vitest'
import { decodeTerminalOutputFrame, TERMINAL_BINARY_HEADER_BYTES } from './terminalOutput'

function binaryFrame(payloadBytes: number) {
  const buffer = new ArrayBuffer(TERMINAL_BINARY_HEADER_BYTES + payloadBytes)
  const bytes = new Uint8Array(buffer)
  bytes.set([
    0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
    0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
  ])
  new DataView(buffer).setBigUint64(16, 42n)
  return buffer
}

const smallFrame = binaryFrame(4 * 1024)
const largeFrame = binaryFrame(64 * 1024)

describe('terminal output binary decoder', () => {
  bench('4 KiB frame', () => {
    decodeTerminalOutputFrame(smallFrame)
  })

  bench('64 KiB frame', () => {
    decodeTerminalOutputFrame(largeFrame)
  })
})
