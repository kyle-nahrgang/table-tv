import { Box, Typography } from '@mui/material'

export function Home() {
  return (
    <Box sx={{ p: 2 }}>
      <Typography variant="h4" component="h1" gutterBottom>
        Home
      </Typography>
      <Typography color="text.secondary">
        Welcome to Table TV.
      </Typography>
    </Box>
  )
}
