import { afterEach, describe, expect, it, vi } from 'vitest'
import { createRequestId } from './requestId'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('createRequestId', () => {
  it('uses the native UUID generator when it is available', () => {
    const randomUUID = vi.fn(() => '12345678-1234-4123-8123-123456789abc')
    vi.stubGlobal('crypto', { randomUUID })

    expect(createRequestId()).toBe('12345678-1234-4123-8123-123456789abc')
    expect(randomUUID).toHaveBeenCalledOnce()
  })

  it('creates a UUID v4 with getRandomValues outside a secure context', () => {
    const getRandomValues = vi.fn((bytes: Uint8Array) => {
      bytes.fill(0)
      return bytes
    })
    vi.stubGlobal('crypto', { getRandomValues })

    expect(createRequestId()).toBe('00000000-0000-4000-8000-000000000000')
    expect(getRandomValues).toHaveBeenCalledOnce()
  })

  it('does not fall back to a predictable random source', () => {
    vi.stubGlobal('crypto', {})

    expect(() => createRequestId()).toThrow(
      'Cryptographically secure random numbers are unavailable',
    )
  })
})
