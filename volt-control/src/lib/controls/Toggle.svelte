<script lang="ts">
  // On/off button. `announce` returns a toast for a click, for switches whose
  // state the front panel may not show (it only shows the selected input).
  import { app, isOn, notify, setParam } from '../state.svelte'
  import Learn from './Learn.svelte'

  let {
    id,
    label = '',
    color = 'var(--accent)',
    small = false,
    announce,
  }: { id: string; label?: string; color?: string; small?: boolean; announce?: (on: boolean) => string } = $props()

  const p = $derived(app.params[id])
  const on = $derived(isOn(id))
  async function click() {
    const next = !on
    if ((await setParam(id, next ? 1 : 0)) && announce) notify(announce(next))
  }
</script>

<!-- Absent on this model: nothing to show. -->
{#if p}
  <Learn {id}>
    <button class="toggle" class:on class:small style:--c={color} onclick={click} aria-pressed={on}>
      {label || p.label}
    </button>
  </Learn>
{/if}

<style>
  .toggle {
    min-width: 54px;
    padding: 7px 10px;
    border-radius: 6px;
    border: 1px solid #000;
    background: linear-gradient(#34363c, #26282c);
    color: var(--muted);
    font: 700 11px/1 var(--sans);
    letter-spacing: 0.05em;
    white-space: nowrap;
    cursor: pointer;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
  }
  .toggle.small {
    min-width: 34px;
    padding: 5px 7px;
    font-size: 10px;
  }
  @media (hover: hover) and (pointer: fine) {
    .toggle:hover {
      color: var(--fg);
    }
  }
  .toggle.on {
    color: #111;
    background: var(--c);
    box-shadow: 0 0 10px color-mix(in srgb, var(--c) 55%, transparent);
  }
</style>
