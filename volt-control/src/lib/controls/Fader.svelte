<script lang="ts">
  import { app } from '../state.svelte'
  import Learn from './Learn.svelte'
  import { clamp01, frameSender, keyStep, sweepPx, wheelStep } from './drag'

  let { id, height = 180, accent = 'var(--accent)' }: { id: string; height?: number; accent?: string } = $props()

  const p = $derived(app.params[id])
  let dragging = $state(false)
  let local = $state(0)
  const n = $derived(dragging ? local : (p?.normalized ?? 0))
  const send = frameSender(() => id)
  const CAP = 26

  let startY = 0
  let startN = 0
  function down(e: PointerEvent) {
    if (e.button !== 0 || !p) return
    const track = e.currentTarget as HTMLElement
    track.setPointerCapture(e.pointerId)
    startY = e.clientY
    // Clicking the track (not the cap) jumps there; dragging is always relative.
    if (e.target === track) {
      const rect = track.getBoundingClientRect()
      startN = clamp01(1 - (e.clientY - rect.top - CAP / 2) / (height - CAP))
      send(startN)
    } else startN = n
    local = startN
    dragging = true
  }
  function move(e: PointerEvent) {
    if (!dragging) return
    local = clamp01(startN + (startY - e.clientY) / sweepPx(e, height - CAP))
    send(local)
  }
  function up() {
    dragging = false
  }
  function wheel(e: WheelEvent) {
    e.preventDefault()
    send(clamp01(n + wheelStep(e)))
  }
  function key(e: KeyboardEvent) {
    const d = keyStep(e)
    if (d === null) return
    e.preventDefault()
    send(clamp01(n + d))
  }
</script>

<!-- Absent on this model: nothing to show. -->
{#if p}
  <Learn {id}>
    <div class="fader">
      <div
        class="track"
        style:height="{height}px"
        role="slider"
        tabindex="0"
        aria-label={p.label}
        aria-valuenow={Math.round(n * 100)}
        aria-valuemin={0}
        aria-valuemax={100}
        onpointerdown={down}
        onpointermove={move}
        onpointerup={up}
        onpointercancel={up}
        onwheel={wheel}
        onkeydown={key}
      >
        <div class="groove"></div>
        <div class="fill" style:height="{n * (height - CAP) + CAP / 2}px" style:background={accent}></div>
        <div class="cap" style:bottom="{n * (height - CAP)}px"></div>
      </div>
      <div class="readout">{p.display}</div>
    </div>
  </Learn>
{/if}

<style>
  .fader {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    user-select: none;
  }
  .track {
    position: relative;
    width: 34px;
    cursor: ns-resize;
    touch-action: none;
  }
  .track:focus-visible {
    outline: 2px solid var(--accent);
    border-radius: 4px;
  }
  .groove {
    position: absolute;
    left: 15px;
    top: 0;
    bottom: 0;
    width: 4px;
    border-radius: 2px;
    background: var(--track);
    pointer-events: none;
  }
  .fill {
    position: absolute;
    left: 15px;
    bottom: 0;
    width: 4px;
    border-radius: 2px;
    opacity: 0.85;
    pointer-events: none;
  }
  .cap {
    position: absolute;
    left: 3px;
    width: 28px;
    height: 26px;
    border-radius: 4px;
    background: linear-gradient(#5b5e66, #34363c 45%, #2a2c31 55%, #45474d);
    border: 1px solid #000;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
    pointer-events: none;
  }
  .cap::after {
    content: '';
    position: absolute;
    left: 4px;
    right: 4px;
    top: 12px;
    height: 2px;
    background: var(--fg);
    opacity: 0.8;
  }
  .readout {
    font: 600 11px/1 var(--mono);
    min-width: 4em;
    text-align: center;
  }
</style>
