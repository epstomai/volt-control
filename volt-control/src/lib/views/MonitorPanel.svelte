<script lang="ts">
  // Monitor (speakers) and headphone volume side by side. These are the same
  // parameters the front-panel encoder drives, so both stay in sync.
  import Knob from '../controls/Knob.svelte'
  import Meter from '../controls/Meter.svelte'
  import Toggle from '../controls/Toggle.svelte'
  import { i18n, t } from '../i18n.svelte'
  import { app, volumeValue } from '../state.svelte'

  const limitText = (id: string) => {
    const l = app.config?.limits[id]
    return l === undefined ? '' : t('mon.limit', { n: Math.round(volumeValue(l)) })
  }
</script>

<section class="panel outputs">
  <div class="out">
    <h2>{t('mon.title')} {#if i18n.lang !== 'en'}<small>MONITOR</small>{/if}</h2>
    <div class="body">
      <Meter id="mon.out" height={140} />
      <Knob id="mon.volume" size={104} accent="var(--accent)" db />
    </div>
    <span class="limit">{limitText('mon.volume')}</span>
    <div class="row">
      <Toggle id="mon.mute" label="MUTE" color="var(--danger)" small />
      <Toggle id="mon.mono" label="MONO" small />
      <Toggle id="dev.dim" label="DIM" color="#7fb2ff" small />
    </div>
  </div>
  {#if app.params['hp.volume']}
  <div class="divider"></div>
  <div class="out">
    <h2>{t('hp.title')} {#if i18n.lang !== 'en'}<small>HEADPHONE</small>{/if}</h2>
    <div class="body">
      <Meter id="hp.out" height={140} />
      <Knob id="hp.volume" size={104} accent="#56c7a8" db />
    </div>
    <span class="limit">{limitText('hp.volume')}</span>
    <div class="row">
      <Toggle id="hp.mute" label="MUTE" color="var(--danger)" small />
    </div>
  </div>
  {/if}
</section>

<style>
  .outputs {
    flex: 1.4 1 330px;
    min-width: 0;
    display: flex;
    justify-content: space-around;
    gap: 12px;
  }
  .out {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .out h2 {
    align-self: flex-start;
    white-space: nowrap;
  }
  .divider {
    width: 1px;
    background: var(--line);
  }
  .body {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .row {
    display: flex;
    gap: 5px;
  }
  .limit {
    font-size: 11px;
    color: var(--danger);
    min-height: 1em;
  }
</style>
