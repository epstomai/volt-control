// Shared drag/wheel/keyboard behaviour for knobs and faders.
import { setParam } from '../state.svelte'

export const clamp01 = (v: number) => Math.min(1, Math.max(0, v))

/** Sends at most one write per animation frame for a parameter. */
export function frameSender(id: () => string) {
  let raf = 0
  let pending = 0
  return (v: number) => {
    pending = v
    if (!raf)
      raf = requestAnimationFrame(() => {
        raf = 0
        setParam(id(), pending)
      })
  }
}

/** Pixels of travel for a full sweep; Shift gives fine control. */
export const sweepPx = (e: { shiftKey: boolean }, base: number) => (e.shiftKey ? base * 4 : base)

export function wheelStep(e: WheelEvent) {
  const dir = e.deltaY < 0 ? 1 : -1
  return dir * (e.shiftKey ? 0.002 : 0.01)
}

export function keyStep(e: KeyboardEvent): number | null {
  const fine = e.shiftKey ? 0.002 : 0.01
  switch (e.key) {
    case 'ArrowUp':
    case 'ArrowRight':
      return fine
    case 'ArrowDown':
    case 'ArrowLeft':
      return -fine
    case 'PageUp':
      return 0.1
    case 'PageDown':
      return -0.1
    default:
      return null
  }
}
