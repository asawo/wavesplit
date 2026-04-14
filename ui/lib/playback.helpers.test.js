import { describe, it, expect } from 'vitest'
import {
  formatTime,
  hashStr,
  makeWaveformBars,
  extractWaveform,
  applyToggleSolo,
  computeMuted,
  waveformGradientId,
} from './playback.helpers.js'

// ── formatTime ──────────────────────────────────────────────

describe('formatTime', () => {
  it('formats zero as 0:00', () => expect(formatTime(0)).toBe('0:00'))
  it('formats seconds below a minute', () => expect(formatTime(59)).toBe('0:59'))
  it('formats exactly one minute', () => expect(formatTime(60)).toBe('1:00'))
  it('formats mixed minutes and seconds', () => expect(formatTime(90)).toBe('1:30'))
  it('handles >60 minutes', () => expect(formatTime(3661)).toBe('61:01'))
  it('returns --:-- for null', () => expect(formatTime(null)).toBe('--:--'))
  it('returns --:-- for undefined', () => expect(formatTime(undefined)).toBe('--:--'))
  it('truncates fractional seconds', () => expect(formatTime(1.9)).toBe('0:01'))
  it('pads seconds with leading zero', () => expect(formatTime(65)).toBe('1:05'))
})

// ── hashStr ─────────────────────────────────────────────────

describe('hashStr', () => {
  it('is deterministic', () => expect(hashStr('hello')).toBe(hashStr('hello')))
  it('produces different values for different inputs', () => {
    expect(hashStr('abc')).not.toBe(hashStr('xyz'))
  })
  it('returns 0 for empty string', () => expect(hashStr('')).toBe(0))
  it('returns an unsigned 32-bit integer', () => {
    const h = hashStr('test')
    expect(h).toBeGreaterThanOrEqual(0)
    expect(h).toBeLessThan(2 ** 32)
    expect(Number.isInteger(h)).toBe(true)
  })
})

// ── makeWaveformBars ─────────────────────────────────────────

describe('makeWaveformBars', () => {
  it('returns an array of the requested length', () => {
    expect(makeWaveformBars('seed', 10)).toHaveLength(10)
    expect(makeWaveformBars('seed', 120)).toHaveLength(120)
  })
  it('all values are in [0.12, 1.0]', () => {
    const bars = makeWaveformBars('trackid', 100)
    for (const v of bars) {
      expect(v).toBeGreaterThanOrEqual(0.12)
      expect(v).toBeLessThanOrEqual(1.0)
    }
  })
  it('is deterministic — same seed produces same array', () => {
    expect(makeWaveformBars('abc', 50)).toEqual(makeWaveformBars('abc', 50))
  })
  it('different seeds produce different arrays', () => {
    expect(makeWaveformBars('seed1', 20)).not.toEqual(makeWaveformBars('seed2', 20))
  })
  it('handles count=0', () => expect(makeWaveformBars('x', 0)).toEqual([]))
  it('handles count=1', () => expect(makeWaveformBars('x', 1)).toHaveLength(1))
})

// ── extractWaveform ──────────────────────────────────────────

function mockAudioBuffer(samples) {
  return { getChannelData: () => Float32Array.from(samples) }
}

describe('extractWaveform', () => {
  it('returns an array of the requested length', () => {
    const buf = mockAudioBuffer(new Array(1000).fill(0.5))
    expect(extractWaveform(buf, 10)).toHaveLength(10)
  })
  it('all values are in [0, 1]', () => {
    const buf = mockAudioBuffer(Array.from({ length: 1000 }, (_, i) => Math.sin(i)))
    const result = extractWaveform(buf, 20)
    for (const v of result) {
      expect(v).toBeGreaterThanOrEqual(0)
      expect(v).toBeLessThanOrEqual(1)
    }
  })
  it('max value is exactly 1 (normalized)', () => {
    const buf = mockAudioBuffer(Array.from({ length: 400 }, (_, i) => (i % 100 < 50 ? 1 : 0.1)))
    const result = extractWaveform(buf, 4)
    expect(Math.max(...result)).toBeCloseTo(1.0, 5)
  })
  it('silent buffer (all zeros) returns all zeros', () => {
    const buf = mockAudioBuffer(new Array(100).fill(0))
    const result = extractWaveform(buf, 5)
    for (const v of result) expect(v).toBe(0)
  })
  it('handles numPoints=1', () => {
    const buf = mockAudioBuffer([0.5, 0.8, 0.3])
    const result = extractWaveform(buf, 1)
    expect(result).toHaveLength(1)
    expect(result[0]).toBeCloseTo(1.0, 5)
  })
})

// ── applyToggleSolo / computeMuted ───────────────────────────

function makeStemState(solos = {}, mutes = {}) {
  return Object.fromEntries(
    ['vocals', 'drums', 'bass', 'other'].map(k => [
      k,
      { muted: mutes[k] ?? false, soloed: solos[k] ?? false, volume: 1 },
    ])
  )
}

describe('applyToggleSolo', () => {
  it('soloes an unsolo\'d stem', () => {
    const s = makeStemState()
    const next = applyToggleSolo(s, 'bass')
    expect(next.bass.soloed).toBe(true)
    expect(next.vocals.soloed).toBe(false)
    expect(next.drums.soloed).toBe(false)
  })
  it('un-soloes a stem that was already soloed', () => {
    const s = makeStemState({ bass: true })
    const next = applyToggleSolo(s, 'bass')
    expect(next.bass.soloed).toBe(false)
    expect(Object.values(next).every(v => !v.soloed)).toBe(true)
  })
  it('switches solo from A to B — only B ends up soloed', () => {
    const s = makeStemState({ vocals: true })
    const next = applyToggleSolo(s, 'drums')
    expect(next.drums.soloed).toBe(true)
    expect(next.vocals.soloed).toBe(false)
  })
  it('does not mutate the original stemState', () => {
    const s = makeStemState()
    applyToggleSolo(s, 'bass')
    expect(s.bass.soloed).toBe(false)
  })
})

describe('computeMuted', () => {
  it('muted stem with no solo active → true', () => {
    const s = makeStemState({}, { bass: true })
    expect(computeMuted(s, 'bass')).toBe(true)
  })
  it('unmuted stem with no solo active → false', () => {
    const s = makeStemState()
    expect(computeMuted(s, 'bass')).toBe(false)
  })
  it('non-soloed stem when a solo is active → true (muted by solo)', () => {
    const s = makeStemState({ vocals: true })
    expect(computeMuted(s, 'drums')).toBe(true)
  })
  it('soloed stem when a solo is active → false (audible)', () => {
    const s = makeStemState({ vocals: true })
    expect(computeMuted(s, 'vocals')).toBe(false)
  })
  it('mute flag is ignored when any stem is soloed', () => {
    // bass is both muted and non-soloed; vocals is soloed
    const s = makeStemState({ vocals: true }, { bass: true })
    // bass mute flag is true but it should just follow the solo logic
    expect(computeMuted(s, 'bass')).toBe(true) // non-soloed → muted regardless
    // if bass were the soloed stem, the mute flag should not override it
    const s2 = makeStemState({ bass: true }, { bass: true })
    expect(computeMuted(s2, 'bass')).toBe(false) // soloed → audible even if mute flag is set
  })
})

// ── waveformGradientId ──────────────────────────────────────

describe('waveformGradientId', () => {
  it("formats as 'wf-{trackId}-{stemKey}'", () => {
    expect(waveformGradientId('abc', 'bass')).toBe('wf-abc-bass')
  })
  it("works for 'master' key", () => {
    expect(waveformGradientId('abc', 'master')).toBe('wf-abc-master')
  })
  it('produces unique IDs for different trackIds', () => {
    expect(waveformGradientId('t1', 'bass')).not.toBe(waveformGradientId('t2', 'bass'))
  })
  it('produces unique IDs for different stemKeys', () => {
    expect(waveformGradientId('t1', 'bass')).not.toBe(waveformGradientId('t1', 'drums'))
  })
})
