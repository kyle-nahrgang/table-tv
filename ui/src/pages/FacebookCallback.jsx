import { useEffect, useState } from 'react'
import { useSearchParams, useNavigate } from 'react-router-dom'
import { Box, Typography, CircularProgress, Button } from '@mui/material'

/**
 * Handles the OAuth callback from Facebook when redirect goes to the UI port.
 * Exchanges the code for an auth_key and redirects to return_to?auth_key=...
 */
export function FacebookCallback() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const [error, setError] = useState(null)

  useEffect(() => {
    const fbError = searchParams.get('error')
    const fbErrorDesc = searchParams.get('error_description')
    if (fbError) {
      console.error('[FacebookCallback] Facebook error:', fbError, fbErrorDesc)
      setError(fbErrorDesc || fbError)
      return
    }

    const code = searchParams.get('code')
    const state = searchParams.get('state')
    if (!code || !state) {
      console.error('[FacebookCallback] Missing code or state', { hasCode: !!code, hasState: !!state })
      setError('Missing code or state from Facebook.')
      return
    }
    console.log('[FacebookCallback] Exchanging code for auth_key...')

    let cancelled = false
    async function exchange() {
      try {
        const res = await fetch('/api/facebook/exchange-code', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ code, state }),
        })
        const text = await res.text()
        let data = {}
        try {
          if (text) data = JSON.parse(text)
        } catch { /* plain text error */ }
        if (cancelled) return
        if (!res.ok) {
          console.error('[FacebookCallback] Exchange failed', res.status, text)
          setError(data?.detail || data?.message || text || 'Failed to complete sign in.')
          return
        }
        const { auth_key, return_to } = data
        console.log('[FacebookCallback] Exchange succeeded, redirecting to', return_to)
        const sep = return_to.includes('?') ? '&' : '?'
        window.location.href = `${return_to}${sep}auth_key=${encodeURIComponent(auth_key)}`
      } catch (err) {
        console.error('[FacebookCallback] Exchange error', err)
        if (!cancelled) setError(err.message || 'Failed to complete sign in.')
      }
    }
    exchange()
    return () => { cancelled = true }
  }, [searchParams])

  if (error) {
    return (
      <Box display="flex" flexDirection="column" alignItems="center" justifyContent="center" minHeight="50vh" gap={2} p={2}>
        <Typography color="error">{error}</Typography>
        <Button variant="contained" onClick={() => navigate('/')}>
          Back to Home
        </Button>
      </Box>
    )
  }

  return (
    <Box display="flex" flexDirection="column" alignItems="center" justifyContent="center" minHeight="50vh" gap={2}>
      <CircularProgress />
      <Typography color="text.secondary">Completing Facebook sign in…</Typography>
    </Box>
  )
}
