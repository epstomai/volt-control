"""One-off patch: DIRECT button + 14-bit CC in the Svelte UI. Run from volt-control/src."""
import sys


def patch(path, pairs):
    s = open(path, encoding="utf-8").read()
    for old, new in pairs:
        if old not in s:
            sys.exit(f"{path}: anchor not found:\n{old[:120]}")
        s = s.replace(old, new, 1)
    open(path, "w", encoding="utf-8").write(s)


patch("lib/api.ts", [
("export type MsgKind = 'cc' | 'note' | 'pitch_bend'", "export type MsgKind = 'cc' | 'cc14' | 'note' | 'pitch_bend'"),
])

patch("lib/views/MidiView.svelte", [
("""    if (m.kind === 'cc') return `CC ${m.number} · ${ch}`""",
 """    if (m.kind === 'cc') return `CC ${m.number} · ${ch}`
    if (m.kind === 'cc14') return `CC ${m.number}+${m.number + 32} 14-bit · ${ch}`"""),
("""            <td class="mono">{msgText(m)}</td>""",
 """            <td class="mono">
              {msgText(m)}
              {#if (m.kind === 'cc' || m.kind === 'cc14') && m.number < 32}
                <select
                  class="res"
                  title="CC 0–31 可与 CC 32–63 组成 14-bit 高精度控制"
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
            </td>"""),
("import { api, type Mapping, type MidiState, type Mode } from '../api'",
 "import { api, type Mapping, type MidiState, type Mode, type MsgKind } from '../api'"),
("""  .mono {
    font-family: var(--mono);
  }""", """  .mono {
    font-family: var(--mono);
  }
  .res {
    margin-left: 4px;
    padding: 1px 3px;
    font-size: 11px;
  }"""),
])

patch("lib/views/InputStrip.svelte", [
("""    <Toggle id="in.{slug}.invert" label="Ø" small />""",
 """    <Toggle id="in.{slug}.invert" label="Ø" small />
    <Toggle id="in.{slug}.direct" label="DIRECT" color="#56c7a8" small />"""),
("""  .row {
    display: flex;
    gap: 5px;
    justify-content: center;
  }""", """  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    justify-content: center;
  }"""),
])

# Learn hint mentions 14-bit detection.
patch("lib/views/MidiView.svelte", [
("""      <li>音量、增益类绑定默认开启「软接管」：控制器位置追上当前值后才生效，避免音量突跳。</li>""",
 """      <li>音量、增益类绑定默认开启「软接管」：控制器位置追上当前值后才生效，避免音量突跳。</li>
      <li>控制器发 14-bit CC（CC n + CC n+32）时会自动识别为 16384 级高精度；也可在映射表里手动切换 7/14-bit。</li>"""),
])
print("patched")
