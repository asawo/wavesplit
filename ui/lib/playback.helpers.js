export function formatTime(s) {
  if (!s && s !== 0) return '--:--'
  s = Math.floor(s)
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

export function hashStr(str) {
  let h = 0
  for (const c of str) h = (Math.imul(31, h) + c.charCodeAt(0)) | 0
  return h >>> 0
}

export function makeWaveformBars(seed, count) {
  let s = hashStr(seed)
  return Array.from({ length: count }, () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0
    return 0.12 + (s / 0xFFFFFFFF) * 0.88
  })
}

export function extractWaveform(audioBuffer, numPoints) {
  const channel = audioBuffer.getChannelData(0)
  const blockSize = Math.floor(channel.length / numPoints)
  const result = new Array(numPoints)
  for (let i = 0; i < numPoints; i++) {
    const start = i * blockSize
    let sum = 0
    for (let j = start; j < Math.min(start + blockSize, channel.length); j++) {
      sum += channel[j] * channel[j]
    }
    result[i] = Math.sqrt(sum / Math.max(blockSize, 1))
  }
  const max = Math.max(...result, 0.001)
  return result.map(v => v / max)
}

// Pure version of the component's toggleSolo — takes/returns a plain stemState object.
export function applyToggleSolo(stemState, key) {
  const wasSoloed = stemState[key].soloed
  const reset = Object.fromEntries(
    Object.entries(stemState).map(([k, v]) => [k, { ...v, soloed: false }])
  )
  if (!wasSoloed) reset[key] = { ...reset[key], soloed: true }
  return reset
}

// Pure version of the component's isMuted(key).
export function computeMuted(stemState, key) {
  const anySoloed = Object.values(stemState).some(s => s.soloed)
  return anySoloed ? !stemState[key].soloed : stemState[key].muted
}

export function waveformGradientId(trackId, stemKey) {
  return `wf-${trackId}-${stemKey}`
}
