import { useState } from 'react'
import { Button } from '@mui/material'
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

  const getStartMs = () => (typeof startMs === 'function' ? startMs() : startMs)

  const handleDownload = async () => {
    setLoading(true)
    onLoadingStart?.()
    try {
      await downloadGameRecording(cameraId, getStartMs(), durationSec, filename)
    } catch (err) {
      console.error('Download failed', err)
      onError?.(err)
    } finally {
      setLoading(false)
      onLoadingEnd?.()
    }
  }

  const isDisabled = disabled || loading

  return (
    <Button
      size="small"
      variant={variant}
      startIcon={<DownloadIcon />}
      onClick={handleDownload}
      disabled={isDisabled}
      sx={sx}
    >
      {loading ? 'Downloading…' : label}
    </Button>
  )
}
