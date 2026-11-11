<script lang="ts">
  import { app, volumeDb } from '../state.svelte'
  import Learn from './Learn.svelte'
  import { clamp01, frameSender, keyStep, sweepPx, wheelStep } from './drag'

  let {
    id,
    size = 64,
    label = '',
    accent = 'var(--accent)',
    bipolar = false,
    db = false,
  }: { id: string; size?: number; label?: string; accent?: string; bipolar?: boolean; db?: boolean } = $props()

  const p = $derived(app.params[id])
  const limit = $derived(app.config?.limits[id])
  let dragging = $state(false)
  let local = $state(0)
  const n = $derived(dragging ? local : (p?.normalized ?? 0))
  const send = frameSender(() => id)

  // 270° sweep starting at 225° (7:30 o'clock).
  const START = 135
  const SWEEP = 270
  const r = $derived(size / 2 - 5)
  const c = $derived(size / 2)
  function pt(deg: number, radius: number) {
    const a = (deg * Math.PI) / 180
    return [c + radius * Math.cos(a), c + radius * Math.sin(a)]
  }
  function arc(from: number, to: number) {
    const [x1, y1] = pt(START + from * SWEEP, r)
    const [x2, y2] = pt(START + to * SWEEP, r)
    const large = (to - from) * SWEEP > 180 ? 1 : 0
    return `M ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2}`
  }
  const valueArc = $derived(bipolar ? arc(Math.min(0.5, n), Math.max(0.5, n)) : arc(0, Math.max(n, 0.0001)))
  const pointer = $derived([pt(START + n * SWEEP, r * 0.35), pt(START + n * SWEEP, r * 0.85)])
  const limitTick = $derived(limit === undefined ? null : [pt(START + limit * SWEEP, r - 4), pt(START + limit * SWEEP, r + 4)])

  let startY = 0
  let startN = 0
  function down(e: PointerEvent) {
    if (e.button !== 0 || !p) return
    ;(e.currentTarget as Element).setPointerCapture(e.pointerId)
    startY = e.clientY
    startN = n
    local = n
    dragging = true
  }
  function move(e: PointerEvent) {
    if (!dragging) return
    local = clamp01(startN + (startY - e.clientY) / sweepPx(e, 220))
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
  <Learn {id} class="knob-wrap">
    <div class="knob">
      <svg
        width={size}
        height={size}
        role="slider"
        tabindex="0"
        aria-label={label || p?.label}
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
        <circle cx={c} cy={c} r={r - 7} class="cap" />
        <path d={arc(0, 1)} class="track" />
        <path d={valueArc} class="value" style:stroke={accent} />
        {#if limitTick}
          <line x1={limitTick[0][0]} y1={limitTick[0][1]} x2={limitTick[1][0]} y2={limitTick[1][1]} class="limit" />
        {/if}
        <line x1={pointer[0][0]} y1={pointer[0][1]} x2={pointer[1][0]} y2={pointer[1][1]} class="pointer" />
      </svg>
      <div class="readout">{p.display}</div>
      {#if db}<div class="sub">{volumeDb(p.value)}</div>{/if}
      {#if label}<div class="label">{label}</div>{/if}
    </div>
  </Learn>
{/if}

<style>
  .knob {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    user-select: none;
  }
  svg {
    cursor: ns-resize;
    touch-action: none;
    border-radius: 50%;
  }
  svg:focus-visible {
    outline: 2px solid var(--accent);
  }
  .cap {
    fill: var(--knob);
    stroke: #000;
    stroke-opacity: 0.5;
  }
  .track {
    fill: none;
    stroke: var(--track);
    stroke-width: 4;
    stroke-linecap: round;
  }
  .value {
    fill: none;
    stroke-width: 4;
    stroke-linecap: round;
  }
  .pointer {
    stroke: var(--fg);
    stroke-width: 2.5;
    stroke-linecap: round;
  }
  .limit {
    stroke: var(--danger);
    stroke-width: 2;
  }
  .readout {
    font: 600 12px/1.2 var(--mono);
    color: var(--fg);
    min-width: 4em;
    text-align: center;
  }
  .sub {
    font: 11px/1.2 var(--mono);
    color: var(--muted);
  }
  .label {
    font-size: 11px;
    color: var(--muted);
    letter-spacing: 0.04em;
  }
</style>
