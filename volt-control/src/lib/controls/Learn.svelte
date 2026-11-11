<script lang="ts">
  // Wraps any control: right-click (or click while in learn mode) arms MIDI learn for `id`.
  import type { Snippet } from 'svelte'
  import { t } from '../i18n.svelte'
  import { app, startLearn } from '../state.svelte'

  let { id, children, class: cls = '' }: { id: string; children: Snippet; class?: string } = $props()

  const mapped = $derived(app.mappings.some((m) => m.param === id))
  const learning = $derived(app.learn === id)

  function arm(e: Event) {
    e.preventDefault()
    e.stopPropagation()
    startLearn(learning ? null : id)
  }

  function capture(e: PointerEvent) {
    if (app.learnMode && e.button === 0) arm(e)
  }
</script>

<div
  class="learn {cls}"
  class:learning
  class:mapped
  class:armable={app.learnMode}
  role="group"
  oncontextmenu={arm}
  onpointerdowncapture={capture}
>
  {@render children()}
  {#if mapped}<span class="dot" aria-label={t('learn.mapped')}></span>{/if}
</div>

<style>
  .learn {
    position: relative;
    border-radius: 8px;
    outline: 2px solid transparent;
    outline-offset: 2px;
    transition: outline-color 0.15s;
  }
  .armable {
    outline-color: rgba(120, 170, 255, 0.25);
    cursor: crosshair;
  }
  .learning {
    outline-color: var(--learn);
    animation: pulse 0.9s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      outline-color: rgba(120, 170, 255, 0.2);
    }
  }
  .dot {
    position: absolute;
    top: -3px;
    right: -3px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--learn);
    box-shadow: 0 0 6px var(--learn);
    pointer-events: none;
  }
</style>
