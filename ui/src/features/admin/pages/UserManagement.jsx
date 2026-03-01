import { useState, useEffect } from 'react'
import {
  Box,
  Typography,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Switch,
  Alert,
  CircularProgress,
} from '@mui/material'
import { listUsers, updateUser } from '../api/users.js'
import { useAuth } from '../../../authStore.jsx'

export function UserManagement() {
  const { user: currentUser } = useAuth()
  const [users, setUsers] = useState([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [updatingSub, setUpdatingSub] = useState(null)

  const fetchUsers = async () => {
    setLoading(true)
    setError('')
    try {
      const data = await listUsers()
      setUsers(data)
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchUsers()
  }, [])

  const handleAdminToggle = async (u) => {
    setUpdatingSub(u.auth0_sub)
    setError('')
    try {
      await updateUser(u.auth0_sub, { is_admin: !u.is_admin })
      await fetchUsers()
    } catch (err) {
      setError(err.message)
    } finally {
      setUpdatingSub(null)
    }
  }

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant="h4" component="h1" gutterBottom>
        User Management
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        Users are created when they first sign in. Toggle admin access below. At least one admin
        must remain.
      </Typography>

      {error && (
        <Alert severity="error" onClose={() => setError('')} sx={{ mb: 2 }}>
          {error}
        </Alert>
      )}

      {loading ? (
        <Box display="flex" justifyContent="center" py={4}>
          <CircularProgress />
        </Box>
      ) : (
        <TableContainer component={Paper}>
          <Table>
            <TableHead>
              <TableRow>
                <TableCell>Email</TableCell>
                <TableCell>Auth0 Sub</TableCell>
                <TableCell align="center">Admin</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {users.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={3} align="center" sx={{ py: 4 }}>
                    <Typography color="text.secondary">
                      No users yet. Users appear after they sign in.
                    </Typography>
                  </TableCell>
                </TableRow>
              ) : (
                users.map((u) => (
                  <TableRow key={u.auth0_sub}>
                    <TableCell>{u.email}</TableCell>
                    <TableCell>
                      <Typography
                        variant="body2"
                        color="text.secondary"
                        sx={{ fontFamily: 'monospace', fontSize: '0.8rem' }}
                      >
                        {u.auth0_sub}
                      </Typography>
                    </TableCell>
                    <TableCell align="center">
                      <Switch
                        checked={u.is_admin}
                        onChange={() => handleAdminToggle(u)}
                        disabled={
                          updatingSub === u.auth0_sub ||
                          (u.auth0_sub === currentUser?.sub && u.is_admin) // Can't remove own admin
                        }
                        color="primary"
                      />
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </TableContainer>
      )}
    </Box>
  )
}
