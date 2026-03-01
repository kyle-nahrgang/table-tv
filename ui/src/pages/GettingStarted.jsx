import { useEffect } from 'react'
import { Box, Typography, Button, CircularProgress } from '@mui/material'
import { useAuth0 } from '@auth0/auth0-react'
import { useLocation } from 'react-router-dom'
import { useAuth } from '../authStore.jsx'
import { useApiInfo } from '../apiInfoStore.jsx'

export function GettingStarted() {
  const { loginWithRedirect } = useAuth0()
  const location = useLocation()
  const { isLoggedIn } = useAuth()
  const { refetch } = useApiInfo()

  // When user returns from login, refetch so has_users updates and we show the app
  useEffect(() => {
    if (isLoggedIn) {
      refetch({ silent: true })
    }
  }, [isLoggedIn, refetch])

  const handleLogin = () => {
    loginWithRedirect({ appState: { returnTo: location.pathname || '/' } })
  }

  // Brief loading state while refetching after login
  if (isLoggedIn) {
    return (
      <Box display="flex" justifyContent="center" alignItems="center" minHeight="100vh">
        <CircularProgress />
      </Box>
    )
  }

  return (
    <Box
      display="flex"
      flexDirection="column"
      alignItems="center"
      justifyContent="center"
      minHeight="100vh"
      p={3}
      sx={{ textAlign: 'center' }}
    >
      <Typography variant="h4" component="h1" gutterBottom>
        Getting Started
      </Typography>
      <Typography variant="body1" color="text.secondary" sx={{ maxWidth: 400, mb: 3 }}>
        No users have registered yet. Log in to create the first admin account and configure Table
        TV.
      </Typography>
      <Button variant="contained" size="large" onClick={handleLogin}>
        Log in
      </Button>
    </Box>
  )
}
