<script lang="ts">
  import { onMount } from 'svelte'
  import { cubicOut } from 'svelte/easing'
  import { fade, fly } from 'svelte/transition'
  import { api } from './lib/api'
  import SystemVolume from './lib/controls/SystemVolume.svelte'
  import { t, type Key } from './lib/i18n.svelte'
  import { app, init, notify, startLearn } from './lib/state.svelte'
  import MainView from './lib/views/MainView.svelte'
  import MidiView from './lib/views/MidiView.svelte'
  import MixerView from './lib/views/MixerView.svelte'
  import SettingsView from './lib/views/SettingsView.svelte'

  type Tab = 'main' | 'mixer' | 'midi' | 'settings'
  let tab = $state<Tab>('main')
  const tabs: { key: Tab; label: Key }[] = [
    { key: 'main', label: 'tab.main' },
    { key: 'mixer', label: 'tab.mixer' },
    { key: 'midi', label: 'tab.midi' },
    { key: 'settings', label: 'tab.settings' },
  ]

  function selectTab(key: Tab) {
    if (key === tab) return
    tab = key
    api.showTab(key).catch((e) => notify(t('err.show_tab', { e })))
  }

  onMount(() => {
    init().catch((e) => notify(t('err.init', { e })))
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        app.learnMode = false
        if (app.learn) startLearn(null)
      }
    }
    window.addEventListener('keydown', esc)
    return () => window.removeEventListener('keydown', esc)
  })

  // MIDI activity LED: lit for 120 ms after each message.
  let now = $state(performance.now())
  $effect(() => {
    if (!app.midiActivity) return
    now = performance.now()
    const t = setTimeout(() => (now = performance.now()), 130)
    return () => clearTimeout(t)
  })
  const midiLit = $derived(!!app.midiActivity && now - app.midiActivity.at < 120)
</script>

<header>
  <div class="brand">VOLT<span>CONTROL</span></div>
  <nav>
    {#each tabs as tb (tb.key)}
      <button class:on={tab === tb.key} onclick={() => selectTab(tb.key)}>{t(tb.label)}</button>
    {/each}
  </nav>
  <div class="status" class:ok={app.status.connected}>
    <span class="led"></span><span class="status-text">{app.status.message}</span>
  </div>
  <div class="sys-slot"><SystemVolume /></div>
  <span class="midi-led" class:lit={midiLit}>MIDI</span>
  <button
    class="learn-btn"
    class:on={app.learnMode}
    onclick={() => {
      app.learnMode = !app.learnMode
      if (!app.learnMode && app.learn) startLearn(null)
    }}
  >{t('learn.button')}</button>
</header>

<main class:offline={!app.device}>
  {#if !app.device}
    <div class="offline-note">
      <h2>{app.status.connected ? t('offline.reading') : t('offline.title')}</h2>
      <p>{app.status.message}</p>
      <p class="muted">{t('offline.hint')}</p>
    </div>
  {:else if tab === 'main'}
    <MainView />
  {:else if tab === 'mixer'}
    <MixerView />
  {:else if tab === 'midi'}
    <MidiView />
  {:else}
    <SettingsView />
  {/if}
</main>

<div class="toasts">
  {#each app.notices as n (n.id)}
    <!-- enters from where it rests, leaves faster than it came -->
    <div class="toast" in:fly={{ y: 8, duration: 200, easing: cubicOut }} out:fade={{ duration: 120 }}>{n.text}</div>
  {/each}
</div>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    background: #0d0e10;
    border-bottom: 1px solid var(--line);
  }
  .brand {
    white-space: nowrap;
    font: 800 15px/1 var(--sans);
    letter-spacing: 0.18em;
    color: var(--accent);
  }
  .brand span {
    color: var(--muted);
    margin-left: 6px;
    font-weight: 600;
  }
  nav {
    display: flex;
    gap: 4px;
  }
  nav button {
    white-space: nowrap;
    padding: 7px 14px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    font: 600 13px/1 var(--sans);
    cursor: pointer;
  }
  nav button.on {
    background: #24262b;
    color: var(--fg);
  }
  .status {
    min-width: 0;
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .led {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
    flex-shrink: 0;
  }
  .status.ok .led {
    background: #2fbf71;
    box-shadow: 0 0 6px #2fbf71;
  }
  .midi-led {
    font: 700 10px/1 var(--sans);
    padding: 4px 6px;
    border-radius: 4px;
    color: var(--muted);
    border: 1px solid var(--line);
  }
  .midi-led.lit {
    color: #111;
    background: var(--learn);
  }
  .learn-btn {
    white-space: nowrap;
    padding: 7px 12px;
    border-radius: 6px;
    border: 1px solid var(--learn);
    background: transparent;
    color: var(--learn);
    font: 600 12px/1 var(--sans);
    cursor: pointer;
  }
  .learn-btn.on {
    background: var(--learn);
    color: #111;
  }
  main {
    min-width: 0;
    padding: 12px;
    overflow: auto;
    flex: 1;
  }
  .offline-note {
    max-width: 520px;
    margin: 80px auto;
    text-align: center;
  }
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 10;
  }
  .toast {
    max-width: 420px;
    padding: 10px 14px;
    border-radius: 8px;
    background: #2a2c31;
    border: 1px solid var(--line);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.4);
    font-size: 13px;
  }
  /* Narrow windows: only what sits right of the tabs collapses, so the tabs
     never move under the cursor when a tab switch resizes the window. */
  @media (max-width: 640px) {
    .sys-slot,
    .status-text,
    .midi-led {
      display: none;
    }
  }
</style>
