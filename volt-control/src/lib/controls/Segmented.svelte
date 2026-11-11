<script lang="ts">
  // One button per option of a choice parameter.
  import { app, setParam } from '../state.svelte'
  import Learn from './Learn.svelte'

  let { id, label = '' }: { id: string; label?: string } = $props()

  const p = $derived(app.params[id])
  const options = $derived(p?.type === 'choice' ? p.options : [])
  const index = $derived(options.length > 1 ? Math.round((p?.normalized ?? 0) * (options.length - 1)) : 0)
</script>

<!-- Absent on this model: nothing to show. -->
{#if p}
  <Learn {id}>
    <div class="seg">
      {#if label}<span class="label">{label}</span>{/if}
      <div class="opts">
        {#each options as o, i (o)}
          <button class:on={i === index} onclick={() => setParam(id, options.length > 1 ? i / (options.length - 1) : 0)}>{o}</button>
        {/each}
      </div>
    </div>
  </Learn>
{/if}

<style>
  .seg {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .label {
    font-size: 11px;
    color: var(--muted);
  }
  .opts {
    display: flex;
    border: 1px solid #000;
    border-radius: 6px;
    overflow: hidden;
  }
  button {
    flex: 1;
    padding: 6px 4px;
    border: 0;
    border-right: 1px solid #000;
    background: #26282c;
    color: var(--muted);
    font: 700 10px/1 var(--sans);
    letter-spacing: 0.03em;
    cursor: pointer;
  }
  button:last-child {
    border-right: 0;
  }
  /* joined buttons: a scaled segment would open gaps in the group */
  button:active {
    transform: none;
    background: #303237;
  }
  @media (hover: hover) and (pointer: fine) {
    button:hover {
      color: var(--fg);
    }
  }
  button.on {
    background: var(--accent);
    color: #111;
  }
</style>
