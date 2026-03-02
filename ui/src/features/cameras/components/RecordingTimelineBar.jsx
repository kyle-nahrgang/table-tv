import { useState, useEffect, useCallback } from 'react'
import { Box, Typography, Tooltip } from '@mui/material'
import { getRecordingTimeline } from '../api/cameras.js'
import { parseRetentionMs } from '../../../utils/format.js'

/**
 * Timeline bar showing when video recording is available for a camera.
 * Uses the retention window (recordDeleteAfter) as the time range.
 * @param {Object} props
 * @param {string} props.cameraId
 * @param {string} props.recordDeleteAfter - e.g. "24h", "7d"
 * @param {boolean} [props.disabled] - Hide when camera offline
 */
export function RecordingTimelineBar({ cameraId, recordDeleteAfter, disabled }) {
  const [segments, setSegments] = useState([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)
  const [rangeStart, setRangeStart] = useState(0)
  const [rangeEnd, setRangeEnd] = useState(0)

  const fetchTimeline = useCallback(async () => {
    if (!cameraId || disabled) return
    let retentionMs = parseRetentionMs(recordDeleteAfter)
    if (retentionMs === Infinity) {
      retentionMs = 7 * 24 * 60 * 60 * 1000 // Default to 7 days when keeping forever
    }
    const endMs = Date.now()
    const startMs = endMs - retentionMs
    setLoading(true)
    setError(null)
    try {
      const data = await getRecordingTimeline(cameraId, startMs, endMs)
      setSegments(data)
      setRangeStart(startMs)
      setRangeEnd(endMs)
    } catch (err) {
      setError(err.message)
      setSegments([])
    } finally {
      setLoading(false)
    }
  }, [cameraId, recordDeleteAfter, disabled])

  useEffect(() => {
    fetchTimeline()
    const interval = setInterval(fetchTimeline, 60000) // Refresh every minute
    return () => clearInterval(interval)
  }, [fetchTimeline])

  if (disabled) return null

  const rangeMs = rangeEnd - rangeStart
  const totalDurationSec = segments.reduce((sum, s) => sum + s.duration_sec, 0)

  return (
    <Box sx={{ mt: 2, width: '100%' }}>
      <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.5 }}>
        Recording availability
      </Typography>
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 1,
          minHeight: 24,
          width: '100%',
        }}
      >
        <Typography variant="caption" color="text.secondary" sx={{ minWidth: 48, fontVariantNumeric: 'tabular-nums' }}>
          {loading ? '…' : rangeMs > 0 ? new Date(rangeStart).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }) : ''}
        </Typography>
        <Box
          sx={{
            flex: 1,
            minWidth: 0,
            height: 12,
            borderRadius: 1,
            backgroundColor: 'action.hover',
            overflow: 'hidden',
            position: 'relative',
          }}
        >
          {loading ? (
            <Box sx={{ width: '100%', height: '100%', bgcolor: 'action.selected', opacity: 0.5 }} />
          ) : error ? (
            <Tooltip title={error}>
              <Box sx={{ width: '100%', height: '100%', bgcolor: 'error.main', opacity: 0.3 }} />
            </Tooltip>
          ) : segments.length === 0 ? (
            <Tooltip title="No recording segments in retention window">
              <Box sx={{ width: '100%', height: '100%' }} />
            </Tooltip>
          ) : (
            segments.map((seg, i) => {
              const left = rangeMs > 0 ? ((seg.start_ms - rangeStart) / rangeMs) * 100 : 0
              const width = rangeMs > 0 ? (seg.duration_sec * 1000 / rangeMs) * 100 : 0
              return (
                <Tooltip
                  key={i}
                  title={`${new Date(seg.start_ms).toLocaleString()} – ${Math.round(seg.duration_sec)}s`}
                >
                  <Box
                    sx={{
                      position: 'absolute',
                      left: `${left}%`,
                      width: `${Math.max(width, 2)}%`,
                      height: '100%',
                      bgcolor: 'primary.main',
                      borderRadius: 0.5,
                    }}
                  />
                </Tooltip>
              )
            })
          )}
        </Box>
        <Typography variant="caption" color="text.secondary" sx={{ minWidth: 48, fontVariantNumeric: 'tabular-nums' }}>
          {loading ? '…' : rangeMs > 0 ? new Date(rangeEnd).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }) : ''}
        </Typography>
      </Box>
      {!loading && !error && segments.length > 0 && (
        <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 0.25 }}>
          {Math.round(totalDurationSec / 60)} min of video available
        </Typography>
      )}
    </Box>
  )
}
