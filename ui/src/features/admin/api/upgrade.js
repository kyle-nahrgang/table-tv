/**
 * Upgrade API client (admin only).
 * Streams apt command output to the onOutput callback.
 */

import { fetchWithAuth } from '../../../apiClient.js'

/**
 * Run apt update and stream output to onOutput.
 * @param {(chunk: string) => void} onOutput - Called with each chunk of output
 */
export async function checkForUpgrades(onOutput) {
  const res = await fetchWithAuth('/api/upgrade/check', { method: 'POST' })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to run apt update')
  }
  await streamResponseBody(res, onOutput)
}

/**
 * Run apt install -y table-tv and stream output to onOutput.
 * @param {(chunk: string) => void} onOutput - Called with each chunk of output
 */
export async function upgradeNow(onOutput) {
  const res = await fetchWithAuth('/api/upgrade/install', { method: 'POST' })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to run apt install')
  }
  await streamResponseBody(res, onOutput)
}

async function streamResponseBody(res, onOutput) {
  const reader = res.body.getReader()
  const decoder = new TextDecoder()
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    onOutput(decoder.decode(value, { stream: true }))
  }
}
