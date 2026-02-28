import { useState } from 'react'
import {
  Box,
  Button,
  TextField,
  Typography,
  Paper,
  Alert,
} from '@mui/material'
import { loginAdmin } from '../api/login'

/**
 * Login form for authenticating an admin.
 * @param {Object} props
 * @param {(result: { ok: boolean }) => void} props.onSuccess - Called after successful login
 */
export function Login({ onSuccess }) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e) => {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      const result = await loginAdmin(email, password)
      onSuccess?.(result)
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
    }
  }

  return (
    <Box
      display="flex"
      flexDirection="column"
      alignItems="center"
      justifyContent="center"
      minHeight="100vh"
      p={2}
    >
      <Paper elevation={3} sx={{ maxWidth: 400, p: 3, width: '100%' }}>
        <Typography variant="h5" component="h1" gutterBottom align="center">
          Admin Login
        </Typography>
        <Box component="form" onSubmit={handleSubmit} display="flex" flexDirection="column" gap={2} sx={{ mt: 2 }}>
          <TextField
            type="email"
            label="Email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoComplete="email"
            fullWidth
          />
          <TextField
            type="password"
            label="Password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            autoComplete="current-password"
            fullWidth
          />
          {error && <Alert severity="error">{error}</Alert>}
          <Button type="submit" variant="contained" disabled={loading} fullWidth>
            {loading ? 'Logging in...' : 'Log in'}
          </Button>
        </Box>
      </Paper>
    </Box>
  )
}
