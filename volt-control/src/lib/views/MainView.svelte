<script lang="ts">
  import { app, isOn } from '../state.svelte'
  import type { Bus } from '../api'
  import InputStrip from './InputStrip.svelte'
  import MonitorPanel from './MonitorPanel.svelte'

  // One strip per active input bus, as the device reports them: a linked pair
  // shows its stereo bus, an unlinked pair its two mono halves.
  const shown = (b: Bus) => {
    if (b.link) return isOn(b.link) === b.stereo
    const enabled = `bus.${b.slug}.enabled`
    return !app.params[enabled] || isOn(enabled)
  }
  const strips = $derived(app.buses.filter((b) => b.bus_type === 'input' && shown(b)))

  // The link switch sits on the stereo strip, or on the first half of the pair.
  const linkOf = (b: Bus) => (b.link && (b.stereo || strips.find((x) => x.link === b.link) === b) ? b.link : undefined)
</script>

<!-- Panels grow to fill each row and wrap when the window narrows: no dead space. -->
<div class="main">
  <MonitorPanel />
  {#each strips as bus (bus.slug)}
    <InputStrip {bus} link={linkOf(bus)} />
  {/each}
</div>

<style>
  .main {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: stretch;
  }
</style>
