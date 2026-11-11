<script lang="ts">
  // MIDI ports, learn hints and the editable mapping table.
  import { onMount } from 'svelte'
  import { api, type Mapping, type MidiState, type Mode, type MsgKind } from '../api'
  import { t, type Key } from '../i18n.svelte'
  import { app, notify, paramTitle, startLearn } from '../state.svelte'

  let midi = $state<MidiState | null>(null)
  let inputs = $state<string[]>([])
  let output = $state<string | null>(null)

  async function refresh() {
    midi = await api.midiState()
    inputs = [...(app.config?.midi.inputs ?? [])]
    output = app.config?.midi.output ?? null
  }
  onMount(refresh)

  async function apply() {
    const errors = await api.midiSelect(inputs, output)
    errors.forEach(notify)
    if (app.config) {
      app.config.midi.inputs = [...inputs]
      app.config.midi.output = output
    }
    if (!errors.length) notify(t('midi.updated'))
    await refresh()
  }

  function toggleInput(name: string) {
    inputs = inputs.includes(name) ? inputs.filter((n) => n !== name) : [...inputs, name]
  }

  const modes: Mode[] = ['absolute', 'relative_twos', 'relative_offset', 'relative_signed', 'toggle', 'momentary', 'trigger']
  const columns: Key[] = ['col.param', 'col.message', 'col.mode', 'col.range', 'col.step', 'col.invert', 'col.pickup', 'col.feedback']
  const isRelative = (m: Mapping) => m.mode.startsWith('relative')

  function msgText(m: { kind: string; channel: number; number: number }) {
    const ch = `Ch${m.channel + 1}`
    if (m.kind === 'cc') return `CC ${m.number} · ${ch}`
    if (m.kind === 'cc14') return `CC ${m.number}+${m.number + 32} · ${ch}`
    if (m.kind === 'note') return `Note ${m.number} · ${ch}`
    return `PitchBend · ${ch}`
  }

  async function save(m: Mapping) {
    try {
      await api.midiUpdate($state.snapshot(m))
    } catch (e) {
      notify(String(e))
    }
  }

  const activity = $derived(app.midiActivity)
</script>

<div class="grid">
  <section class="panel">
    <h2>{t('midi.ports')}</h2>
    {#if midi}
      <h4>{t('midi.inputs')}</h4>
      {#each midi.ports.inputs as name (name)}
        <label class="check">
          <input type="checkbox" checked={inputs.includes(name)} onchange={() => toggleInput(name)} />
          {name}
          {#if midi.open_inputs.includes(name)}<span class="ok">{t('midi.opened')}</span>{/if}
        </label>
      {:else}
        <p class="muted">{t('midi.no_inputs')}</p>
      {/each}
      <h4>{t('midi.feedback_out')}</h4>
      <select bind:value={output}>
        <option value={null}>{t('midi.no_feedback')}</option>
        {#each midi.ports.outputs as name (name)}<option value={name}>{name}</option>{/each}
      </select>
      <div class="row">
        <button class="action" onclick={apply}>{t('midi.apply')}</button>
        <button class="action ghost" onclick={refresh}>{t('midi.refresh')}</button>
      </div>
    {/if}
    <h4>{t('midi.recent')}</h4>
    <div class="activity">{activity ? `${msgText(activity)} = ${activity.value}` : '—'}</div>
    <h4>{t('midi.howto')}</h4>
    <ol class="muted">
      <!-- static dictionary text; only markup is <b> -->
      <li>{@html t('midi.howto1')}</li>
      <li>{t('midi.howto2')}</li>
      <li>{t('midi.howto3')}</li>
      <li>{t('midi.howto4')}</li>
    </ol>
  </section>

  <section class="panel wide">
    <h2>{t('midi.mappings')} <small>{t('midi.count', { n: app.mappings.length })}</small></h2>
    {#if app.learn}
      <div class="learning">{t('midi.waiting', { name: paramTitle(app.learn) })} <button class="action ghost" onclick={() => startLearn(null)}>{t('midi.cancel')}</button></div>
    {/if}
    <div class="table-wrap">
    <table>
      <thead>
        <tr>{#each columns as c (c)}<th>{t(c)}</th>{/each}<th></th></tr>
      </thead>
      <tbody>
        {#each app.mappings as m (m.id)}
          <tr>
            <td class="name">{paramTitle(m.param)}</td>
            <td class="mono">
              {msgText(m)}
              {#if (m.kind === 'cc' || m.kind === 'cc14') && m.number < 32}
                <select
                  class="res"
                  value={m.kind}
                  onchange={(e) => {
                    m.kind = e.currentTarget.value as MsgKind
                    save(m)
                  }}
                >
                  <option value="cc">7-bit</option>
                  <option value="cc14">14-bit</option>
                </select>
              {/if}
            </td>
            <td>
              <select value={m.mode} onchange={(e) => { m.mode = e.currentTarget.value as Mode; save(m) }}>
                {#each modes as md (md)}<option value={md}>{t(`mode.${md}`)}</option>{/each}
              </select>
            </td>
            <td class="range">
              <input type="number" min="0" max="100" value={Math.round(m.min * 100)} onchange={(e) => { m.min = +e.currentTarget.value / 100; save(m) }} />
              –
              <input type="number" min="0" max="100" value={Math.round(m.max * 100)} onchange={(e) => { m.max = +e.currentTarget.value / 100; save(m) }} />
            </td>
            <td>
              <input type="number" step="0.1" min="0.1" max="25" disabled={!isRelative(m)} value={+(m.step * 100).toFixed(1)} onchange={(e) => { m.step = +e.currentTarget.value / 100; save(m) }} />
            </td>
            <td><input type="checkbox" checked={m.invert} onchange={(e) => { m.invert = e.currentTarget.checked; save(m) }} /></td>
            <td><input type="checkbox" checked={m.pickup} disabled={m.mode !== 'absolute'} onchange={(e) => { m.pickup = e.currentTarget.checked; save(m) }} /></td>
            <td><input type="checkbox" checked={m.feedback} onchange={(e) => { m.feedback = e.currentTarget.checked; save(m) }} /></td>
            <td><button class="del" aria-label={t('midi.delete')} onclick={() => api.midiDelete(m.id)}>✕</button></td>
          </tr>
        {:else}
          <tr><td colspan="9" class="muted">{t('midi.empty')}</td></tr>
        {/each}
      </tbody>
    </table>
    </div>
  </section>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: 320px 1fr;
    gap: 14px;
    align-items: start;
  }
  h4 {
    margin: 14px 0 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 3px 0;
  }
  .ok {
    font-size: 10px;
    color: #2fbf71;
  }
  .row {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .activity {
    font: 600 13px/1.4 var(--mono);
    color: var(--learn);
  }
  ol {
    padding-left: 18px;
    margin: 0;
    font-size: 12px;
    line-height: 1.6;
  }
  .learning {
    padding: 8px 10px;
    margin-bottom: 10px;
    border: 1px solid var(--learn);
    border-radius: 6px;
    color: var(--learn);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th {
    text-align: left;
    color: var(--muted);
    font-weight: 600;
    padding: 6px 4px;
    border-bottom: 1px solid var(--line);
  }
  td {
    padding: 5px 4px;
    border-bottom: 1px solid #1d1f23;
  }
  .range input,
  td input[type='number'] {
    width: 52px;
  }
  .mono {
    font-family: var(--mono);
  }
  .res {
    margin-left: 4px;
    padding: 1px 3px;
    font-size: 11px;
  }
  .del {
    background: none;
    border: 0;
    color: var(--muted);
    cursor: pointer;
  }
  @media (hover: hover) and (pointer: fine) {
    .del:hover {
      color: var(--danger);
    }
  }
  .table-wrap {
    overflow-x: auto;
  }
  th,
  td.name,
  td.mono,
  td.range {
    white-space: nowrap;
  }
  /* grid items default to min-width: auto, which lets the table widen the page */
  .grid > * {
    min-width: 0;
  }
  .check {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  select {
    max-width: 100%;
  }
  @media (max-width: 860px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
