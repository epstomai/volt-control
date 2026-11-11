// Single reactive application state, fed by backend events.
import { api, events, type Bus, type Config, type Mapping, type MeterValue, type MidiMsg, type Param, type Snapshot, type Status } from './api'
import { applyLang, t, type Lang } from './i18n.svelte'

type Notice = { id: number; text: string }

export const app = $state({
  status: { connected: false, message: t('status.starting') } as Status,
  device: null as Snapshot['device'] | null,
  buses: [] as Bus[],
  params: {} as Record<string, Param>,
  mappings: [] as Mapping[],
  config: null as Config | null,
  /** Parameter waiting for a MIDI message (MIDI learn). */
  learn: null as string | null,
  /** While on, clicking a control arms MIDI learn instead of operating it. */
  learnMode: false,
  midiActivity: null as (MidiMsg & { at: number }) | null,
  notices: [] as Notice[],
})

/** Meters change ~60x/s; keep them out of the deep-reactive tree. */
export const live = $state({ meters: {} as Record<string, MeterValue> })

let noticeId = 0
export function notify(text: string) {
  const id = ++noticeId
  app.notices.push({ id, text })
  setTimeout(() => {
    app.notices = app.notices.filter((n) => n.id !== id)
  }, 6000)
}

function applySnapshot(s: Snapshot | null) {
  app.device = s?.device ?? null
  app.buses = s?.buses ?? []
  // System volume (win.*) lives outside the device snapshot: keep it.
  const params: Record<string, Param> = {}
  for (const [id, p] of Object.entries(app.params)) if (id.startsWith('win.')) params[id] = p
  for (const p of s?.params ?? []) params[p.id] = p
  app.params = params
  if (!s) live.meters = {}
}

/** Sends a value; errors become notices instead of disappearing. */
/** Returns false (after showing the error) when the write was rejected. */
export async function setParam(id: string, normalized: number): Promise<boolean> {
  try {
    await api.set(id, normalized)
    return true
  } catch (e) {
    notify(String(e))
    return false
  }
}

export async function trigger(id: string) {
  try {
    await api.trigger(id)
  } catch (e) {
    notify(String(e))
  }
}

export async function startLearn(id: string | null) {
  app.learn = id
  await api.midiLearn(id)
}

export async function refreshConfig() {
  app.config = await api.config()
  app.mappings = app.config.midi.mappings
}

export async function init() {
  applyLang(await api.language())
  await Promise.all([
    events.status((s) => {
      app.status = s
    }),
    events.snapshot(applySnapshot),
    events.params((ps) => {
      for (const p of ps) app.params[p.id] = p
    }),
    events.meters((m) => {
      live.meters = m
    }),
    events.notice(notify),
    events.midiActivity((m) => {
      app.midiActivity = { ...m, at: performance.now() }
    }),
    events.midiMappings((m) => {
      app.mappings = m
      if (app.config) app.config.midi.mappings = m
      app.learn = null
    }),
  ])
  // Events may have fired before the listeners were attached: pull current state.
  app.status = await api.status()
  applySnapshot(await api.snapshot())
  for (const p of await api.system()) app.params[p.id] = p
  await refreshConfig()
}

/** `lang` = null follows Windows. The backend re-translates status, tray and parameter names. */
export async function setLanguage(lang: Lang | null) {
  applyLang(await api.setLanguage(lang))
  applySnapshot(await api.snapshot())
  for (const p of await api.system()) app.params[p.id] = p
  await refreshConfig()
}

// ---------------------------------------------------------------- helpers

export const isOn = (id: string) => (app.params[id]?.normalized ?? 0) >= 0.5

export function busName(slug: string) {
  return app.buses.find((b) => b.slug === slug)?.name ?? slug
}

/** Input strip title from the bus name: "ANALOG 1-2" -> "输入 1-2"; other names stay. */
export function inputTitle(name: string) {
  const m = name.match(/^ANALOG\s+(.+)$/i)
  return m ? t('input.title', { n: m[1] }) : name
}

/** Label of a stereo link switch: "链接 1-2" (the numbers of the pair's stereo bus). */
export function linkLabel(link: string) {
  const name = app.buses.find((b) => b.link === link && b.stereo)?.name ?? ''
  return t('input.link', { n: name.match(/[\d-]+$/)?.[0] ?? '' }).trim()
}

/** Human title for a parameter id, used in the MIDI mapping list. */
export function paramTitle(id: string) {
  const p = app.params[id]
  if (!p) return id
  const g = p.group
  if (g.startsWith('input.')) return `${busName(g.slice(6))} · ${p.label}`
  if (g.startsWith('mix.')) {
    const out = id.split('.')[2]
    const target = out === 'cue1' ? ' → CUE 1' : out === 'cue2' ? ' → CUE 2' : out === 'main' ? ` → ${t('target.monitor')}` : ''
    return `${busName(g.slice(4))}${target} · ${p.label}`
  }
  return p.label
}

/** Device volume (0..100) at knob position n; mirrors `model::volume` (curve: 0..10% -> 0..40, 10..100% -> 40..100). */
export function volumeValue(n: number): number {
  if (!(app.config?.volume_curve ?? true)) return n * 100
  return n < 0.1 ? (n / 0.1) * 40 : 40 + ((n - 0.1) / 0.9) * 60
}

/** Output volumes attenuate 1 dB per step below 100. */
export const volumeDb = (value: unknown) =>
  typeof value !== 'number' ? '' : value <= 0 ? t('vol.muted') : `${(value - 100).toFixed(1)} dB`

/** Linear amplitude -> dBFS. */
export const toDb = (v: number) => (v > 0 ? 20 * Math.log10(v) : -Infinity)
