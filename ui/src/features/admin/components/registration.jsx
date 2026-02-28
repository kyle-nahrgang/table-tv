import { useState } from 'react'
import {
  Box,
  Button,
  TextField,
  Typography,
  Paper,
  Alert,
} from '@mui/material'
import { createAdmin } from '../api/create'

/**
 * Registration form for creating the first admin account.
 * @param {Object} props
 * @param {() => void} props.onSuccess - Called after successful registration
 */
export function Registration({ onSuccess }) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e) => {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      await createAdmin(email, password)
      onSuccess?.()
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
          Create Admin Account
        </Typography>
        <Typography color="text.secondary" align="center" sx={{ mb: 2 }}>
          No admin exists yet. Create the first admin to get started.
        </Typography>
        <Box component="form" onSubmit={handleSubmit} display="flex" flexDirection="column" gap={2}>
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
            autoComplete="new-password"
            fullWidth
          />
          {error && <Alert severity="error">{error}</Alert>}
          <Button type="submit" variant="contained" disabled={loading} fullWidth>
            {loading ? 'Creating...' : 'Sign up'}
          </Button>
        </Box>
      </Paper>
    </Box>
  )
}
