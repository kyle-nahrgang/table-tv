/**
 * Authenticated API client. Call setTokenGetter from AuthProvider with a function
 * that returns the access token (or null when not authenticated).
 */

/** @type {(() => Promise<string | null>) | null} */
let getTokenFn = null

/**
 * Set the function used to obtain the access token for API requests.
 * Must be called from AuthProvider when Auth0 is ready.
 * @param {(() => Promise<string | null>) | null} fn
 */
export function setTokenGetter(fn) {
  getTokenFn = fn
}

/**
 * Fetch with Authorization header when token is available.
 * @param {string} url
 * @param {RequestInit} [options]
 * @returns {Promise<Response>}
 */
export async function fetchWithAuth(url, options = {}) {
  const headers = { ...options.headers }
  try {
    const token = getTokenFn ? await getTokenFn() : null
    if (token) {
      headers.Authorization = `Bearer ${token}`
    }
  } catch {
    // Token fetch failed (e.g. not authenticated)
  }
  return fetch(url, { ...options, headers })
}

/**
 * Get the current access token. Returns null if not authenticated.
 * Use for building URLs that can't use headers (e.g. img src).
 * @returns {Promise<string | null>}
 */
export async function getToken() {
  try {
    return getTokenFn ? await getTokenFn() : null
  } catch {
    return null
  }
}

/**
 * Build a URL with access_token query param for endpoints that can't use headers (e.g. img src).
 * @param {string} baseUrl
 * @param {string | null} token
 * @returns {string}
 */
export function urlWithToken(baseUrl, token) {
  if (!token) return baseUrl
  const sep = baseUrl.includes('?') ? '&' : '?'
  return `${baseUrl}${sep}access_token=${encodeURIComponent(token)}`
}
