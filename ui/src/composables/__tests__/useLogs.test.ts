/**
 * Unit tests for useLogs secret redaction and sessionStorage hardening (TASK-101 / SEC-017)
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import {
  useLogs,
  resetLogs,
  redactSecretString,
  redactSensitiveData,
  redactLogEntry,
  persistToSessionStorage,
  SESSION_STORAGE_KEY,
  type LogEntry
} from '../useLogs'

describe('useLogs Secret Redaction & SessionStorage Hardening (SEC-017)', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    sessionStorage.clear()
    resetLogs()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  describe('redactSecretString', () => {
    it('redacts Bearer tokens in headers and text', () => {
      const input = 'Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.t-ID'
      const output = redactSecretString(input)
      expect(output).not.toContain('eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.t-ID')
      expect(output).toContain('Bearer [REDACTED]')
    })

    it('redacts basic authentication credentials', () => {
      const input = 'Authorization: Basic dXNlcjpwYXNzd29yZDEyMw=='
      const output = redactSecretString(input)
      expect(output).not.toContain('dXNlcjpwYXNzd29yZDEyMw==')
      expect(output).toContain('Basic [REDACTED]')
    })

    it('redacts credentials embedded in URLs', () => {
      const input = 'Request sent to https://alice:secret_pass_999@api.spotify.com/v1/tracks'
      const output = redactSecretString(input)
      expect(output).not.toContain('secret_pass_999')
      expect(output).toContain('https://alice:[REDACTED]@api.spotify.com/v1/tracks')
    })

    it('redacts query and key-value secrets (api_key, token, password, secret, cookie)', () => {
      const input1 = 'GET /search?api_key=AIzaSySecretApiKey123&query=jazz'
      expect(redactSecretString(input1)).toBe('GET /search?api_key=[REDACTED]&query=jazz')

      const input2 = 'Connection failed with password="my_super_secret_pwd" for user admin'
      expect(redactSecretString(input2)).toBe('Connection failed with password="[REDACTED]" for user admin')

      const input3 = 'token: secret-token-xyz-888'
      expect(redactSecretString(input3)).toBe('token: [REDACTED]')

      const input4 = 'Headers: Cookie: session_id=session_val_12345; user=john'
      expect(redactSecretString(input4)).toBe('Headers: Cookie: [REDACTED]')
    })

    it('redacts standalone JWT tokens', () => {
      const jwt = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c'
      const input = `User payload with jwt=${jwt}`
      const output = redactSecretString(input)
      expect(output).not.toContain(jwt)
      expect(output).toContain('[REDACTED')
    })
  })

  describe('redactSensitiveData', () => {
    it('redacts sensitive object keys in details', () => {
      const details = {
        endpoint: '/api/v1/auth',
        token: 'secret_token_123',
        apiKey: 'secret_key_456',
        password: 'cleartext_password',
        client_secret: 'oauth_secret_789',
        nested: {
          access_token: 'nested_secret_token',
          safeField: 'normal value'
        }
      }

      const redacted = redactSensitiveData(details)
      expect(redacted.token).toBe('[REDACTED]')
      expect(redacted.apiKey).toBe('[REDACTED]')
      expect(redacted.password).toBe('[REDACTED]')
      expect(redacted.client_secret).toBe('[REDACTED]')
      expect(redacted.nested.access_token).toBe('[REDACTED]')
      expect(redacted.nested.safeField).toBe('normal value')
      expect(redacted.endpoint).toBe('/api/v1/auth')
    })

    it('redacts secrets within strings in non-sensitive keys', () => {
      const details = {
        errorMessage: 'Failed when calling endpoint with Bearer my_raw_bearer_token'
      }

      const redacted = redactSensitiveData(details)
      expect(redacted.errorMessage).toBe('Failed when calling endpoint with Bearer [REDACTED]')
    })

    it('handles arrays and circular references safely', () => {
      const circularObj: any = { name: 'circular test' }
      circularObj.self = circularObj

      const result = redactSensitiveData(circularObj)
      expect(result.self).toBe('[CIRCULAR]')

      const arrayData = [
        { token: 'token1' },
        { message: 'Authorization: Bearer tok2' }
      ]
      const arrayResult = redactSensitiveData(arrayData)
      expect(arrayResult[0].token).toBe('[REDACTED]')
      expect(arrayResult[1].message).toContain('Bearer [REDACTED]')
    })
  })

  describe('SessionStorage Persistence Hardening', () => {
    it('redacts secrets before persisting to sessionStorage via persistToSessionStorage', () => {
      const entries: LogEntry[] = [
        {
          id: 'log-1',
          time: '12:00:00',
          level: 'error',
          provider: 'Spotify',
          category: 'Auth',
          message: 'Token expired: Bearer spotify_secret_access_token_123',
          rawCategory: 'security',
          details: {
            apiKey: 'spotify_api_key_456',
            token: 'refresh_tok_789',
            url: 'https://user:password123@api.spotify.com'
          }
        }
      ]

      persistToSessionStorage(entries, true)

      const storedRaw = sessionStorage.getItem(SESSION_STORAGE_KEY)
      expect(storedRaw).not.toBeNull()
      expect(storedRaw).not.toContain('spotify_secret_access_token_123')
      expect(storedRaw).not.toContain('spotify_api_key_456')
      expect(storedRaw).not.toContain('refresh_tok_789')
      expect(storedRaw).not.toContain('password123')

      const parsed = JSON.parse(storedRaw!)
      expect(parsed[0].message).toBe('Token expired: Bearer [REDACTED]')
      expect(parsed[0].details.apiKey).toBe('[REDACTED]')
      expect(parsed[0].details.token).toBe('[REDACTED]')
      expect(parsed[0].details.url).toContain('[REDACTED]')
    })

    it('redacts secrets when logs are added via useLogs().addLog and debounced to sessionStorage', () => {
      const { addLog } = useLogs()

      addLog({
        level: 'warn',
        provider: 'Tidal',
        category: 'Download',
        message: 'Download error with api_key=tidal_super_secret_key_888 and Bearer tidal_bearer_abc',
        rawCategory: 'downloads',
        details: {
          cookie: 'session_cookie=sensitive_session_val',
          userPassword: 'plaintext_password'
        }
      })

      // Before timer advances, debounce is active
      expect(sessionStorage.getItem(SESSION_STORAGE_KEY)).toBeNull()

      // Advance debounce timer (50ms)
      vi.advanceTimersByTime(60)

      const storedRaw = sessionStorage.getItem(SESSION_STORAGE_KEY)
      expect(storedRaw).not.toBeNull()
      expect(storedRaw).not.toContain('tidal_super_secret_key_888')
      expect(storedRaw).not.toContain('tidal_bearer_abc')
      expect(storedRaw).not.toContain('sensitive_session_val')
      expect(storedRaw).not.toContain('plaintext_password')

      const parsed = JSON.parse(storedRaw!)
      expect(parsed[0].message).toContain('api_key=[REDACTED]')
      expect(parsed[0].message).toContain('Bearer [REDACTED]')
      expect(parsed[0].details.cookie).toBe('[REDACTED]')
    })

    it('clears sessionStorage when resetLogs is invoked', () => {
      persistToSessionStorage([
        {
          id: 'log-clean',
          time: '12:00:00',
          level: 'info',
          provider: 'System',
          category: 'Core',
          message: 'Safe log message',
          rawCategory: 'system'
        }
      ], true)

      expect(sessionStorage.getItem(SESSION_STORAGE_KEY)).not.toBeNull()

      resetLogs()

      expect(sessionStorage.getItem(SESSION_STORAGE_KEY)).toBeNull()
    })
  })
})
