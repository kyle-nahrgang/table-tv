import { Box, Typography } from '@mui/material'

export function CameraSettings() {
  return (
    <Box sx={{ p: 2 }}>
      <Typography variant="h4" component="h1" gutterBottom>
        Camera Settings
      </Typography>
      <Typography color="text.secondary">
        Configure camera settings here.
      </Typography>
    </Box>
  )
}
