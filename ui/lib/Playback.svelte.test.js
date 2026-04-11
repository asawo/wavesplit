import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, fireEvent, waitFor, cleanup } from '@testing-library/svelte'
import Playback from './Playback.svelte'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn(path => `asset://${path}`),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}))

// Minimal AudioBuffer that satisfies extractWaveform (needs getChannelData)
function mockBuffer() {
  return {
    duration: 10,
    getChannelData: vi.fn().mockReturnValue(new Float32Array(1000)),
  }
}

// Fresh AudioContext mock per test so spies don't bleed between tests
function makeAudioCtx() {
  return {
    createGain: vi.fn().mockReturnValue({
      gain: { value: 1, setTargetAtTime: vi.fn() },
      connect: vi.fn(),
    }),
    createBufferSource: vi.fn().mockReturnValue({
      buffer: null, connect: vi.fn(), start: vi.fn(), stop: vi.fn(), disconnect: vi.fn(),
    }),
    destination: {},
    currentTime: 0,
    state: 'running',
    close: vi.fn().mockResolvedValue(undefined),
    decodeAudioData: vi.fn().mockResolvedValue(mockBuffer()),
    resume: vi.fn().mockResolvedValue(undefined),
  }
}

function makeTrack(id) {
  return {
    id,
    title: `Track ${id}`,
    artist: null,
    status_download: 'done',
    status_stems: 'done',
    status_analysis: 'done',
    error_message: null,
    duration_ms: 10000,
  }
}

let audioCtx
let cancelAnimationFrameSpy

beforeEach(() => {
  audioCtx = makeAudioCtx()
  vi.stubGlobal('AudioContext', vi.fn().mockReturnValue(audioCtx))

  let rafId = 0
  vi.stubGlobal('requestAnimationFrame', vi.fn().mockImplementation(() => ++rafId))
  cancelAnimationFrameSpy = vi.fn()
  vi.stubGlobal('cancelAnimationFrame', cancelAnimationFrameSpy)

  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
    ok: true,
    arrayBuffer: vi.fn().mockResolvedValue(new ArrayBuffer(1024)),
  }))

  invoke.mockResolvedValue({
    bass: '/stems/bass.wav', drums: '/stems/drums.wav',
    vocals: '/stems/vocals.wav', other: '/stems/other.wav',
  })
})

afterEach(() => {
  vi.unstubAllGlobals()
  cleanup()
})

// Wait until the play button is enabled (audio has finished loading)
async function waitForAudio(container) {
  await waitFor(() => {
    const btn = container.querySelector('.play-btn')
    if (!btn || btn.disabled) throw new Error('audio not loaded yet')
  })
}

describe('Playback resource management', () => {
  it('cancels the existing RAF loop when switching to a different track', async () => {
    const { container, rerender } = render(Playback, {
      track: makeTrack('a'),
      active: true,
      onBack: vi.fn(),
    })

    await waitForAudio(container)

    // Start playback — schedTick() → requestAnimationFrame, setting rafId
    await fireEvent.click(container.querySelector('.play-btn'))
    expect(requestAnimationFrame).toHaveBeenCalled()

    // Reset spy so we only capture cancellations from the track switch
    cancelAnimationFrameSpy.mockClear()

    // Switch to a different track — loadAudio() should call cancelTick()
    await rerender({ track: makeTrack('b'), active: true, onBack: vi.fn() })

    await waitFor(() => expect(cancelAnimationFrameSpy).toHaveBeenCalled())
  })

  it('closes the AudioContext when the component unmounts', async () => {
    const { container, unmount } = render(Playback, {
      track: makeTrack('a'),
      active: true,
      onBack: vi.fn(),
    })

    // Wait for audio to load so AudioContext is created
    await waitForAudio(container)

    unmount()

    expect(audioCtx.close).toHaveBeenCalled()
  })
})
