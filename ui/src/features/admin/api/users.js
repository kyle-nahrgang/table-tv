/**
 * Users API client (admin only).
 */
import { fetchWithAuth } from '../../../apiClient.js'

/**
 * @returns {Promise<Array<{ auth0_sub: string, email: string, is_admin: boolean }>>}
 */
export async function listUsers() {
  const res = await fetchWithAuth('/api/users')
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to fetch users')
  }
  return res.json()
}

/**
 * Update user admin status.
 * @param {string} sub - Auth0 subject (user ID)
 * @param {{ is_admin: boolean }} data
 * @returns {Promise<{ auth0_sub: string, email: string, is_admin: boolean }>}
 */
export async function updateUser(sub, data) {
  const encodedSub = encodeURIComponent(sub)
  const res = await fetchWithAuth(`/api/users/${encodedSub}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to update user')
  }
  return res.json()
}
