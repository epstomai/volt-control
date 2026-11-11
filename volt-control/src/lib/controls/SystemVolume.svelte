<script lang="ts">
  // Windows system volume in the header: a slim slider that tracks the pointer
  // 1:1 (grab anywhere on the track), plus a speaker button for mute.
  import { t } from '../i18n.svelte'
  import { app, isOn, setParam } from '../state.svelte'
  import Learn from './Learn.svelte'
  import { clamp01, frameSender, keyStep, wheelStep } from './drag'

  const p = $derived(app.params['win.volume'])
  const muted = $derived(isOn('win.mute'))
  let dragging = $state(false)
  let local = $state(0)
  const n = $derived(dragging ? local : (p?.normalized ?? 0))
  const send = frameSender(() => 'win.volume')

  function at(e: PointerEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    return clamp01((e.clientX - r.left) / r.width)
  }
  function down(e: PointerEvent) {
    if (e.button !== 0) return
    ;(e.currentTarget as Element).setPointerCapture(e.pointerId)
    dragging = true
    local = at(e)
    send(local)
  }
  function move(e: PointerEvent) {
    if (!dragging) return
    local = at(e)
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

{#if p}
  <div class="sysvol" class:muted>
    <Learn id="win.mute">
      <button class="mute" aria-label={t('sys.mute')} aria-pressed={muted} onclick={() => setParam('win.mute', muted ? 0 : 1)}>
        <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
          <path d="M2 6h2.5L8 3v10L4.5 10H2z" />
          {#if muted}
            <path d="M10.5 6l4 4m0-4l-4 4" class="stroke" />
          {:else}
            <path d="M10.5 5.5a3.5 3.5 0 010 5M12.5 3.5a6.5 6.5 0 010 9" class="stroke" />
          {/if}
        </svg>
      </button>
    </Learn>
    <Learn id="win.volume">
      <div
        class="track"
        role="slider"
        tabindex="0"
        aria-label={t('sys.volume')}
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
        <div class="fill" style:transform="scaleX({n})"></div>
        <div class="thumb" style:left="{n * 100}%"></div>
      </div>
    </Learn>
    <span class="value">{Math.round(n * 100)}</span>
  </div>
{/if}

<style>
  .sysvol {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .track {
    position: relative;
    width: 96px;
    height: 18px;
    cursor: pointer;
    touch-action: none;
  }
  /* the visible rail is thin; the hit area is the full 18px */
  .track::before,
  .fill {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 7px;
    height: 4px;
    border-radius: 2px;
  }
  .track::before {
    background: var(--track);
  }
  .fill {
    background: var(--muted);
    transform-origin: left;
  }
  .thumb {
    position: absolute;
    top: 4px;
    width: 10px;
    height: 10px;
    margin-left: -5px;
    border-radius: 50%;
    background: var(--fg);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.5);
  }
  .track:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: 2px;
    border-radius: 4px;
  }
  .value {
    min-width: 2.2em;
    font: 600 12px/1 var(--mono);
    color: var(--fg);
  }
  .muted .fill {
    background: #4a4d55;
  }
  .muted .value {
    color: var(--muted);
  }
  .mute {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .mute svg {
    fill: currentColor;
  }
  .mute .stroke {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
  }
  .muted .mute {
    color: var(--danger);
  }
  @media (hover: hover) and (pointer: fine) {
    .mute:hover {
      color: var(--fg);
      background: #24262b;
    }
  }
</style>
