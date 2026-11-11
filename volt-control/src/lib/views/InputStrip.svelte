<script lang="ts">
  // Preamp channel: gain, +48V, tone, compressor, polarity, auto gain, jack status.
  // Controls the model lacks render nothing (see the controls).
  import Knob from '../controls/Knob.svelte'
  import Learn from '../controls/Learn.svelte'
  import Meter from '../controls/Meter.svelte'
  import Segmented from '../controls/Segmented.svelte'
  import Toggle from '../controls/Toggle.svelte'
  import type { Bus } from '../api'
  import { t } from '../i18n.svelte'
  import { app, inputTitle, isOn, linkLabel, trigger } from '../state.svelte'

  let { bus, link }: { bus: Bus; link?: string } = $props()
  const slug = $derived(bus.slug)
  const title = $derived(inputTitle(bus.name))

  const hasJack = $derived(['mic', 'line', 'hiz'].some((k) => app.params[`in.${slug}.jack_${k}`]))
  /** Plugged jack type; null when nothing is plugged in. */
  const jack = $derived(
    isOn(`in.${slug}.jack_hiz`) ? 'HI-Z' : isOn(`in.${slug}.jack_line`) ? 'LINE' : isOn(`in.${slug}.jack_mic`) ? 'MIC' : null,
  )
</script>

<section class="panel strip">
  <header>
    <h2>{title} {#if title !== bus.name}<small>{bus.name}</small>{/if}</h2>
    {#if hasJack}<span class="jack" class:empty={!jack}>{jack ?? t('input.no_jack')}</span>{/if}
    {#if link}<Toggle id={link} label={linkLabel(link)} small />{/if}
  </header>
  <div class="top">
    <Meter id="{slug}.in" height={110} channels={bus.stereo ? 2 : 1} />
    <Knob id="in.{slug}.gain" size={84} label="GAIN" accent="#e86f3d" />
  </div>
  <div class="row">
    <Toggle id="in.{slug}.48v" label="+48V" color="var(--danger)" small announce={(on) => t(on ? 'input.48v_on' : 'input.48v_off', { title })} />
    <Toggle id="in.{slug}.invert" label="Ø" small />
    <Toggle id="in.{slug}.direct" label="DIRECT" color="#56c7a8" small />
    <Learn id="in.{slug}.autogain">
      <button class="action" onclick={() => trigger(`in.${slug}.autogain`)}>AUTO GAIN</button>
    </Learn>
  </div>
  <Segmented id="in.{slug}.tone" label="Tone" />
  <Segmented id="in.{slug}.comp" label={t('input.comp')} />
</section>

<style>
  .strip {
    flex: 1 1 230px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 24px;
  }
  header h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .top {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    justify-content: center;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    justify-content: center;
  }
  .jack {
    font: 700 10px/1 var(--sans);
    padding: 3px 6px;
    border-radius: 4px;
    background: #2fbf71;
    color: #111;
  }
  .jack.empty {
    background: var(--track);
    color: var(--muted);
  }
</style>
