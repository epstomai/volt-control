<script lang="ts">
  // Monitor mix and the two cue mixes: one strip per active source bus.
  import Fader from '../controls/Fader.svelte'
  import Knob from '../controls/Knob.svelte'
  import Learn from '../controls/Learn.svelte'
  import Meter from '../controls/Meter.svelte'
  import Toggle from '../controls/Toggle.svelte'
  import { t } from '../i18n.svelte'
  import { app, isOn, trigger } from '../state.svelte'

  type Mix = 'main' | 'cue1' | 'cue2'
  let mix = $state<Mix>('main')
  const tabs: { key: Mix; label: () => string }[] = [
    { key: 'main', label: () => t('mix.main') },
    { key: 'cue1', label: () => 'CUE 1' },
    { key: 'cue2', label: () => 'CUE 2' },
  ]

  // Only buses the device currently uses (linking swaps 1+2 for 1-2, etc.).
  const sources = $derived(
    app.buses.filter((b) => (b.bus_type === 'input' || b.bus_type === 'loopback') && isOn(`bus.${b.slug}.enabled`)),
  )
</script>

<div class="tabs">
  {#each tabs as tb (tb.key)}
    <button class:on={mix === tb.key} onclick={() => (mix = tb.key)}>{tb.label()}</button>
  {/each}
  <span class="spacer"></span>
  <Learn id="act.clear_solos"><button class="action" onclick={() => trigger('act.clear_solos')}>{t('mix.clear_solos')}</button></Learn>
</div>

<div class="mixer">
  {#each sources as b (b.slug)}
    {@const pre = `mix.${b.slug}.${mix}`}
    <section class="panel ch">
      <h3>{b.name}</h3>
      <div class="pans">
        {#if b.stereo}
          <Knob id="{pre}.pan_l" size={38} bipolar />
          <Knob id="{pre}.pan_r" size={38} bipolar />
        {:else}
          <Knob id="{pre}.pan" size={38} bipolar />
        {/if}
      </div>
      <div class="fz">
        <Meter id="{b.slug}.in" height={200} channels={b.stereo ? 2 : 1} />
        <Fader id="{pre}.gain" height={200} />
      </div>
      <div class="btns">
        <Toggle id="{pre}.mute" label="M" color="var(--danger)" small />
        {#if mix === 'main'}
          <Toggle id="mix.{b.slug}.solo" label="S" color="#e8c23d" small />
        {:else}
          <Toggle id="{pre}.pre" label="PRE" small />
        {/if}
      </div>
    </section>
  {/each}

  <section class="panel ch master">
    <h3>{mix === 'main' ? 'MONITOR' : mix === 'cue1' ? 'CUE 1' : 'CUE 2'}</h3>
    {#if mix === 'main'}
      <div class="pans"><Knob id="mon.volume" size={38} /></div>
      <div class="fz">
        <Meter id="mon.out" height={200} />
      </div>
      <div class="btns">
        <Toggle id="mon.mute" label="M" color="var(--danger)" small />
        <Toggle id="mon.mono" label="MONO" small />
      </div>
    {:else}
      <div class="pans"></div>
      <div class="fz">
        <Meter id="{mix}.out" height={200} />
        <Fader id="{mix}.volume" height={200} accent="#56c7a8" />
      </div>
      <div class="btns">
        <Toggle id="{mix}.mute" label="M" color="var(--danger)" small />
        <Toggle id="{mix}.mono" label="MONO" small />
      </div>
    {/if}
  </section>
</div>

<style>
  .tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 12px;
  }
  .tabs button {
    padding: 7px 14px;
    border-radius: 6px;
    border: 1px solid #000;
    background: #26282c;
    color: var(--muted);
    font: 600 12px/1 var(--sans);
    cursor: pointer;
  }
  .tabs button.on {
    background: var(--accent);
    color: #111;
  }
  .spacer {
    flex: 1;
  }
  .mixer {
    display: flex;
    gap: 10px;
    overflow-x: auto;
    padding-bottom: 6px;
  }
  .ch {
    width: 112px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .ch h3 {
    margin: 0;
    font: 700 11px/1 var(--sans);
    letter-spacing: 0.05em;
    color: var(--muted);
    white-space: nowrap;
  }
  .master {
    margin-left: auto;
    border-color: #4a3a1c;
  }
  /* Same height for mono/stereo/master so faders line up. */
  .pans {
    display: flex;
    gap: 2px;
    height: 62px;
  }
  .fz {
    display: flex;
    gap: 6px;
    align-items: flex-start;
  }
  .btns {
    display: flex;
    gap: 4px;
  }
</style>
