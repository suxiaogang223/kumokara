const SUPPORTED_PROTOCOLS = new Set(['http:', 'https:', 'ws:', 'wss:'])

function isLoopback(hostname: string) {
  const normalized = hostname.replace(/^\[|\]$/g, '').toLowerCase()
  return normalized === 'localhost'
    || normalized === '::1'
    || normalized.startsWith('127.')
}

export function normalizeServerUrl(value: string, allowInsecureRemote = false) {
  const input = value.trim()
  if (!input) throw new Error('Enter a Kumokara server address')

  const withProtocol = /^[a-z][a-z\d+.-]*:\/\//i.test(input) ? input : `https://${input}`
  const url = new URL(withProtocol)
  if (!SUPPORTED_PROTOCOLS.has(url.protocol)) {
    throw new Error('Use an http, https, ws, or wss server address')
  }
  if (url.username || url.password) {
    throw new Error('Credentials must not be embedded in the server address')
  }
  if (url.search || url.hash) {
    throw new Error('The server address cannot contain a query or fragment')
  }

  const insecure = url.protocol === 'http:' || url.protocol === 'ws:'
  if (insecure && !isLoopback(url.hostname) && !allowInsecureRemote) {
    throw new Error('Remote servers require TLS. Use https:// or wss://')
  }

  url.pathname = url.pathname.replace(/\/+$/, '') || '/'
  return url.toString().replace(/\/$/, '')
}

export function websocketUrl(serverUrl: string) {
  const url = new URL(serverUrl)
  if (!SUPPORTED_PROTOCOLS.has(url.protocol)) throw new Error('Unsupported server protocol')
  if (url.protocol === 'http:') url.protocol = 'ws:'
  if (url.protocol === 'https:') url.protocol = 'wss:'
  url.pathname = `${url.pathname.replace(/\/+$/, '')}/api/ws`
  url.search = ''
  url.hash = ''
  return url.toString()
}
