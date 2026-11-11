<script lang="ts">
  import { api } from '../api'
  import Knob from '../controls/Knob.svelte'
  import Learn from '../controls/Learn.svelte'
  import Segmented from '../controls/Segmented.svelte'
  import Toggle from '../controls/Toggle.svelte'
  import { LANGS, t, type Key, type Lang } from '../i18n.svelte'
  import { app, notify, refreshConfig, setLanguage, trigger, volumeValue } from '../state.svelte'

  const limited: { id: string; label: Key }[] = [
    { id: 'mon.volume', label: 'set.limit_mon' },
    { id: 'hp.volume', label: 'set.limit_hp' },
  ]

  async function changeLanguage(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value
    try {
      await setLanguage(v === '' ? null : (v as Lang))
    } catch (err) {
      notify(String(err))
    }
  }

  async function setLimit(id: string, max: number | null) {
    await api.setLimit(id, max)
    await refreshConfig()
  }

  async function setTray(e: Event) {
    await api.setCloseToTray((e.currentTarget as HTMLInputElement).checked)
    await refreshConfig()
  }

  async function setAutostart(e: Event) {
    const on = (e.currentTarget as HTMLInputElement).checked
    try {
      await api.setAutostart(on)
    } catch (err) {
      notify(t('set.autostart_failed', { e: String(err) }))
    }
    await refreshConfig()
  }

  async function setCurve(e: Event) {
    await api.setVolumeCurve((e.currentTarget as HTMLInputElement).checked)
    await refreshConfig()
  }

  let exporting = $state(false)
  async function exportInfo() {
    exporting = true
    try {
      const path = await api.exportDeviceInfo()
      notify(t('set.exported', { file: path.split(/[\\/]/).pop() ?? path }))
    } catch (e) {
      notify(t('set.export_failed', { e: String(e) }))
    } finally {
      exporting = false
    }
  }

  const sleepChoices = [0, 1, 5, 15, 30, 60]
  async function setSleep(e: Event) {
    await api.setScreenSleep(+(e.currentTarget as HTMLSelectElement).value)
    await refreshConfig()
  }

  async function identify() {
    await trigger('act.identify')
    notify(t('set.identified'))
  }
</script>

<div class="grid">
  <section class="panel">
    <h2>{t('set.device')}</h2>
    {#if app.device}
      <dl>
        <dt>{t('set.model')}</dt><dd>{app.device.name} ({app.device.device_type})</dd>
        <dt>{t('set.serial')}</dt><dd class="mono">{app.device.serial}</dd>
        <dt>{t('set.firmware')}</dt><dd class="mono">{app.device.firmware}</dd>
      </dl>
    {/if}
    <Segmented id="dev.sample_rate" label={t('set.sample_rate')} />
    <div class="row">
      <Knob id="dev.brightness" size={56} label={t('set.brightness')} />
      <Toggle id="dev.no_clip" label={t('set.no_clip')} />
      <Learn id="act.identify"><button class="action" onclick={identify}>{t('set.identify')}</button></Learn>
    </div>
    <label class="row sleep">
      {t('set.screen_sleep')}
      <select value={app.config?.screen_sleep_min ?? 15} onchange={setSleep}>
        {#each sleepChoices as m (m)}<option value={m}>{m ? t('set.minutes', { n: m }) : t('set.never')}</option>{/each}
      </select>
    </label>
    <div class="row">
      <button class="action" onclick={exportInfo} disabled={exporting}>{exporting ? t('set.exporting') : t('set.export')}</button>
      <span class="muted">{t('set.export_hint')}</span>
    </div>
  </section>

  <section class="panel">
    <h2>{t('set.meters')}</h2>
    <Segmented id="sys.peak_hold" label={t('set.peak_hold')} />
    <Segmented id="sys.clip_hold" label={t('set.clip_hold')} />
    <div class="row"><Toggle id="sys.post_fader_meters" label={t('set.post_fader')} /></div>
  </section>

  <section class="panel">
    <h2>{t('set.safety')}</h2>
    <p class="muted">{t('set.safety_hint')}</p>
    {#each limited as l (l.id)}
      {@const v = app.config?.limits[l.id]}
      <div class="limit">
        <label class="check">
          <input type="checkbox" checked={v !== undefined} onchange={(e) => setLimit(l.id, e.currentTarget.checked ? 0.8 : null)} />
          {t(l.label)}
        </label>
        <input type="range" min="0" max="100" disabled={v === undefined} value={Math.round((v ?? 0.8) * 100)} onchange={(e) => setLimit(l.id, +e.currentTarget.value / 100)} />
        <span class="mono">{v === undefined ? t('set.none') : `${Math.round(volumeValue(v))}`}</span>
      </div>
    {/each}
  </section>

  <section class="panel">
    <h2>{t('set.app')}</h2>
    <label class="row sleep">
      {t('set.language')}
      <select value={app.config?.language ?? ''} onchange={changeLanguage}>
        <option value="">{t('set.lang_system')}</option>
        {#each LANGS as l (l.key)}<option value={l.key}>{l.name}</option>{/each}
      </select>
    </label>
    <label class="check">
      <input type="checkbox" checked={app.config?.close_to_tray ?? true} onchange={setTray} />
      {t('set.close_to_tray')}
    </label>
    <label class="check">
      <input type="checkbox" checked={app.config?.autostart ?? true} onchange={setAutostart} />
      {t('set.autostart')}
    </label>
    <label class="check">
      <input type="checkbox" checked={app.config?.volume_curve ?? true} onchange={setCurve} />
      {t('set.curve')}
    </label>
    <p class="muted">{t('set.app_hint')}</p>
  </section>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(330px, 1fr));
    gap: 14px;
    align-items: stretch;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 10px;
    margin: 0;
    font-size: 12px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
  }
  .row {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .sleep {
    font-size: 13px;
    gap: 8px;
  }
  .limit {
    display: grid;
    grid-template-columns: 140px 1fr 44px;
    gap: 8px;
    align-items: center;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 13px;
  }
  .mono {
    font-family: var(--mono);
  }
</style>
