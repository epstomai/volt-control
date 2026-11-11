// Typed bridge to the Rust backend (src-tauri/src/lib.rs).
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Lang } from './i18n.svelte'

export type Kind =
  | { type: 'continuous' }
  | { type: 'toggle' }
  | { type: 'choice'; options: string[] }
  | { type: 'read_only' }
  | { type: 'action' }

export type Param = {
  id: string
  label: string
  group: string
  normalized: number
  value: unknown
  display: string
} & Kind

export type Bus = { uid: string; slug: string; name: string; bus_type: string; stereo: boolean; link: string | null }

export type Snapshot = {
  device: { uid: string; name: string; device_type: string; serial: string; firmware: string }
  buses: Bus[]
  params: Param[]
  meters: string[]
}

export type Status = { connected: boolean; message: string }

/** Per-channel linear amplitudes (0..1). */
export type MeterValue = { level?: number[]; peak?: number[]; clip?: number[] }

export type MsgKind = 'cc' | 'cc14' | 'note' | 'pitch_bend'
export type Mode =
  | 'absolute'
  | 'relative_twos'
  | 'relative_offset'
  | 'relative_signed'
  | 'toggle'
  | 'momentary'
  | 'trigger'

export type Mapping = {
  id: number
  param: string
  kind: MsgKind
  channel: number
  number: number
  mode: Mode
  min: number
  max: number
  invert: boolean
  pickup: boolean
  feedback: boolean
  step: number
}

export type MidiMsg = { kind: MsgKind; channel: number; number: number; value: number }

export type Config = {
  midi: { inputs: string[]; output: string | null; mappings: Mapping[] }
  limits: Record<string, number>
  close_to_tray: boolean
  autostart: boolean
  volume_curve: boolean
  screen_sleep_min: number
  /** null = follow the Windows display language */
  language: Lang | null
}

export type MidiState = {
  ports: { inputs: string[]; outputs: string[] }
  open_inputs: string[]
  open_output: string | null
  learn: string | null
  mappings: Mapping[]
}

export const api = {
  status: () => invoke<Status>('get_status'),
  snapshot: () => invoke<Snapshot | null>('get_snapshot'),
  set: (id: string, normalized: number) => invoke<void>('set_param', { id, normalized }),
  trigger: (id: string) => invoke<void>('trigger', { id }),
  config: () => invoke<Config>('get_config'),
  setLimit: (id: string, max: number | null) => invoke<void>('set_limit', { id, max }),
  setCloseToTray: (enabled: boolean) => invoke<void>('set_close_to_tray', { enabled }),
  setAutostart: (enabled: boolean) => invoke<void>('set_autostart', { enabled }),
  setVolumeCurve: (enabled: boolean) => invoke<void>('set_volume_curve', { enabled }),
  setScreenSleep: (minutes: number) => invoke<void>('set_screen_sleep', { minutes }),
  language: () => invoke<Lang>('get_language'),
  setLanguage: (lang: Lang | null) => invoke<Lang>('set_language', { lang }),
  system: () => invoke<Param[]>('get_system'),
  showTab: (tab: string) => invoke<void>('show_tab', { tab }),
  exportDeviceInfo: () => invoke<string>('export_device_info'),
  midiState: () => invoke<MidiState>('midi_state'),
  midiSelect: (inputs: string[], output: string | null) => invoke<string[]>('midi_select', { inputs, output }),
  midiLearn: (param: string | null) => invoke<void>('midi_learn', { param }),
  midiUpdate: (mapping: Mapping) => invoke<void>('midi_update_mapping', { mapping }),
  midiDelete: (id: number) => invoke<void>('midi_delete_mapping', { id }),
}

export const events = {
  status: (f: (s: Status) => void) => listen<Status>('volt://status', (e) => f(e.payload)),
  snapshot: (f: (s: Snapshot | null) => void) => listen<Snapshot | null>('volt://snapshot', (e) => f(e.payload)),
  params: (f: (p: Param[]) => void) => listen<Param[]>('volt://params', (e) => f(e.payload)),
  meters: (f: (m: Record<string, MeterValue>) => void) =>
    listen<Record<string, MeterValue>>('volt://meters', (e) => f(e.payload)),
  notice: (f: (msg: string) => void) => listen<string>('volt://notice', (e) => f(e.payload)),
  midiActivity: (f: (m: MidiMsg) => void) => listen<MidiMsg>('midi://activity', (e) => f(e.payload)),
  midiMappings: (f: (m: Mapping[]) => void) => listen<Mapping[]>('midi://mappings', (e) => f(e.payload)),
}

export type { UnlistenFn }
