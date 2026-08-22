import { describe, expect, it } from 'vitest'
import { normalizeServerUrl, websocketUrl } from './connection'

describe('normalizeServerUrl', () => {
  it('defaults remote hostnames to TLS', () => {
    expect(normalizeServerUrl('kumokara.example.com')).toBe('https://kumokara.example.com')
  })

  it('allows plaintext loopback servers', () => {
    expect(normalizeServerUrl('http://127.0.0.1:9876')).toBe('http://127.0.0.1:9876')
    expect(normalizeServerUrl('ws://localhost:9876')).toBe('ws://localhost:9876')
  })

  it('requires an explicit opt-in for plaintext remote servers', () => {
    expect(() => normalizeServerUrl('http://192.0.2.10:9876')).toThrow(/require TLS/)
    expect(normalizeServerUrl('http://192.0.2.10:9876', true)).toBe('http://192.0.2.10:9876')
  })

  it('rejects credentials, queries, and unsupported protocols', () => {
    expect(() => normalizeServerUrl('https://user:secret@example.com')).toThrow(/Credentials/)
    expect(() => normalizeServerUrl('https://example.com?token=secret')).toThrow(/query/)
    expect(() => normalizeServerUrl('file:///tmp/socket')).toThrow(/http/)
  })
})

describe('websocketUrl', () => {
  it('maps server URLs to the control endpoint and preserves a path prefix', () => {
    expect(websocketUrl('https://example.com/kumokara')).toBe('wss://example.com/kumokara/api/ws')
    expect(websocketUrl('http://127.0.0.1:9876')).toBe('ws://127.0.0.1:9876/api/ws')
  })
})
