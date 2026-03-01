import { useNavigate, useLocation } from 'react-router-dom'
import { Box, Tab, Tabs, Button, Typography } from '@mui/material'
import { useAuth0 } from '@auth0/auth0-react'
import { useAuth } from '../../../authStore.jsx'
import { ServerSettings } from './ServerSettings'
import { CameraSettings } from './CameraSettings'
import { MatchesSettings } from './MatchesSettings'
import { UserManagement } from './UserManagement'

const TAB_PATHS = ['/admin/server-settings', '/admin/camera-settings', '/admin/matches', '/admin/users']

export function Admin() {
  const navigate = useNavigate()
  const location = useLocation()
  const { loginWithRedirect } = useAuth0()
  const { isLoggedIn, isAdmin, loading, error } = useAuth()

  const handleLogin = () => {
    loginWithRedirect({ appState: { returnTo: location.pathname } })
  }

  if (!isLoggedIn) {
    return (
      <Box display="flex" flexDirection="column" alignItems="center" justifyContent="center" minHeight={400} gap={2}>
        {loading && <Typography color="text.secondary">Loading...</Typography>}
        {error && <Typography color="error">{error}</Typography>}
        {!loading && (
          <>
            <Typography color="text.secondary">Log in to access the admin panel.</Typography>
            <Button variant="contained" onClick={handleLogin}>
              Log in
            </Button>
          </>
        )}
      </Box>
    )
  }

  if (!isAdmin) {
    return (
      <Box display="flex" flexDirection="column" alignItems="center" justifyContent="center" minHeight={400}>
        <Typography color="text.secondary">You don&apos;t have admin access.</Typography>
      </Box>
    )
  }

  const tabIndex = TAB_PATHS.includes(location.pathname)
    ? TAB_PATHS.indexOf(location.pathname)
    : 0

  const handleTabChange = (_, newIndex) => {
    navigate(TAB_PATHS[newIndex])
  }

  return (
    <Box>
      <Tabs value={tabIndex} onChange={handleTabChange} sx={{ mb: 2 }}>
        <Tab label="Server Settings" />
        <Tab label="Camera Settings" />
        <Tab label="Matches" />
        <Tab label="Users" />
      </Tabs>
      {tabIndex === 0 && <ServerSettings />}
      {tabIndex === 1 && <CameraSettings />}
      {tabIndex === 2 && <MatchesSettings />}
      {tabIndex === 3 && <UserManagement />}
    </Box>
  )
}
