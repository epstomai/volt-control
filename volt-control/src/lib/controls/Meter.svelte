<script lang="ts">
  // Vertical peak meter, -60..0 dBFS. Values arrive as linear amplitude.
  import { live, toDb } from '../state.svelte'

  let { id, height = 180, channels = 2 }: { id: string; height?: number; channels?: number } = $props()

  const FLOOR = -60
  const m = $derived(live.meters[id])
  const pos = (v: number | undefined) => {
    const db = toDb(v ?? 0)
    return Math.max(0, Math.min(1, (db - FLOOR) / -FLOOR))
  }
  const bars = $derived(
    Array.from({ length: channels }, (_, i) => ({
      level: pos(m?.level?.[i] ?? m?.level?.[0]),
      peak: pos(m?.peak?.[i] ?? m?.peak?.[0]),
      clip: !!(m?.clip?.[i] ?? m?.clip?.[0]),
    })),
  )
</script>

<div class="meter" style:height="{height}px" aria-hidden="true">
  {#each bars as b, i (i)}
    <div class="ch">
      <div class="clip" class:on={b.clip}></div>
      <div class="bar">
        <div class="cover" style:transform="scaleY({1 - b.level})"></div>
        <div class="pk" style:transform="translateY({(1 - b.peak) * 100}%) translateY(-2px)"></div>
      </div>
    </div>
  {/each}
</div>

<style>
  .meter {
    display: flex;
    gap: 2px;
  }
  .ch {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 6px;
  }
  .clip {
    height: 5px;
    border-radius: 1px;
    background: #3a1818;
  }
  .clip.on {
    background: var(--danger);
    box-shadow: 0 0 5px var(--danger);
  }
  .bar {
    position: relative;
    flex: 1;
    border-radius: 1px;
    overflow: hidden;
    /* scale -60..0 dBFS: -18 dB at 70%, -6 dB at 90% */
    background: linear-gradient(to top, #2fbf71 0%, #2fbf71 70%, #e8c23d 70%, #e8c23d 90%, #e8513d 90%);
  }
  .cover {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 100%;
    transform-origin: top;
    background: #0b0c0e;
  }
  /* full-height layer whose top edge is the peak line */
  .pk {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 100%;
    border-top: 2px solid #fff;
    background: none;
    opacity: 0.8;
  }
</style>
