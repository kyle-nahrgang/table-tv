import { useState } from 'react'
import { Button, Box, CircularProgress } from '@mui/material'
import DownloadIcon from '@mui/icons-material/Download'
import { downloadGameRecording } from '../api/poolMatches.js'

/**
 * Button to download a game recording.
 *
 * @param {Object} props
 * @param {string} props.cameraId
 * @param {number | (() => number)} props.startMs - Start time in ms, or function to compute at click time
 * @param {number} props.durationSec
 * @param {string} props.filename
 * @param {boolean} [props.disabled]
 * @param {() => void} [props.onLoadingStart]
 * @param {() => void} [props.onLoadingEnd]
 * @param {(err: Error) => void} [props.onError] - Called when download fails
 * @param {string} [props.label] - Button label, default "Download"
 * @param {'text'|'outlined'|'contained'} [props.variant] - Button variant
 * @param {Object} [props.sx] - MUI sx prop for the button
 */
export function DownloadRecordingButton({
  cameraId,
  startMs,
  durationSec,
  filename,
  disabled = false,
  onLoadingStart,
  onLoadingEnd,
  onError,
  label = 'Download',
  variant,
  sx,
}) {
  const [loading, setLoading] = useState(false)
  const [progress, setProgress] = useState(0)
  const [fileForShare, setFileForShare] = useState(null)
  const shareSupported = !!navigator?.canShare
  const mode = fileForShare && shareSupported ? 'share' : 'download'

  const getStartMs = () => (typeof startMs === 'function' ? startMs() : startMs)

  const handleDownload = async () => {
    setLoading(true)
    setProgress(0)
    onLoadingStart?.()
    try {
      const file = await downloadGameRecording(
        cameraId,
        getStartMs(),
        durationSec,
        filename,
        setProgress,
        !shareSupported // autoDownload=false when sharing supported
      )
      // keep file for sharing if supported
      if (file && shareSupported && navigator.canShare({ files: [file] })) {
        setFileForShare(file)
      }
    } catch (err) {
      console.error('Download failed', err)
      onError?.(err)
    } finally {
      setLoading(false)
      setProgress(0)
      onLoadingEnd?.()
    }
  }

  const isDisabled = disabled || loading

  const handleClick = () => {
    if (mode === 'share' && fileForShare) {
      navigator.share({ files: [fileForShare] }).catch((err) => {
        // user cancelled or share failed; log and keep file for another try
        console.warn('Share failed', err)
      })
    } else {
      handleDownload()
    }
  }

  const displayLabel = mode === 'share' ? 'Share' : label

  if (loading) {
    // while loading, only show progress indicator
    return (
      <Box sx={{ position: 'relative', display: 'inline-flex' }}>
        <Box
          sx={{
            position: 'relative',
            width: 48,
            height: 48,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <CircularProgress
            variant="determinate"
            value={progress}
            size={32}
          />
          <Box
            sx={{
              position: 'absolute',
              fontSize: '0.65rem',
              fontWeight: 'bold',
            }}
          >
            {progress}%
          </Box>
        </Box>
      </Box>
    )
  }

  return (
    <Box sx={{ position: 'relative', display: 'inline-flex' }}>
      <Button
        size="small"
        variant={variant}
        startIcon={<DownloadIcon />}
        onClick={handleClick}
        disabled={isDisabled}
        sx={sx}
      >
        {displayLabel}
      </Button>
    </Box>
  )
}
