<script lang="ts">
  import { api } from "./api";
  import Icone from "./Icone.svelte";

  let { slots }: { slots: ({ nome: string; caminho: string } | null)[] } = $props();

  let aceso = $state<number | null>(null);

  async function clicar(i: number) {
    if (!slots[i]) return;
    aceso = i;
    setTimeout(() => (aceso === i ? (aceso = null) : null), 1200);
    await api.cartucheiraTocar(i);
  }
</script>

<section class="painel">
  <div class="rotulo">
    <Icone nome="grid" tamanho={14} />Cartucheira
  </div>
  <div class="grade">
    {#each slots as slot, i}
      <div class="slot-caixa">
        <button
          class="slot"
          class:vazio={!slot}
          class:aceso={aceso === i}
          onclick={() => clicar(i)}
          title={slot?.caminho ?? "Vazio (configure na aba Cartucheira)"}
        >
          {#if slot}<span class="reticencias">{slot.nome}</span>{/if}
        </button>
      </div>
    {/each}
  </div>
</section>

<style>
  .grade {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    grid-auto-rows: 1fr;
    gap: 6px;
    padding: 0 10px 10px;
    min-height: 0;
  }
  .slot-caixa {
    position: relative;
    min-height: 0;
  }
  .slot {
    width: 100%;
    height: 100%;
    padding: 0 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #d6d9df;
    font-size: 15px;
    font-weight: 500;
    min-height: 48px;
  }
  .slot.vazio {
    cursor: default;
    border-style: dashed;
    color: var(--apagado);
    background: transparent;
  }
  .slot.aceso {
    background: #5a3a0e;
    border-color: var(--pasta);
    color: var(--laranja);
  }
</style>
