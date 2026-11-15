<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "./api";
  import Icone from "./Icone.svelte";

  let { slots }: { slots: ({ nome: string; caminho: string } | null)[] } = $props();

  let aceso = $state<number | null>(null);
  let editando = $state(false);

  async function escolher(i: number) {
    const arquivo = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Áudio", extensions: ["mp3", "ogg", "opus", "flac", "wav", "m4a", "aac"] }],
    });
    if (typeof arquivo === "string") await api.cartucheiraDefinir(i, arquivo);
  }

  async function clicar(i: number) {
    const slot = slots[i];
    if (editando || !slot) return escolher(i);
    aceso = i;
    setTimeout(() => (aceso === i ? (aceso = null) : null), 1200);
    await api.cartucheiraTocar(i);
  }
</script>

<section class="painel">
  <div class="rotulo">
    <Icone nome="grid" tamanho={14} />Cartucheira
    <button class="editar" class:on={editando} onclick={() => (editando = !editando)}>
      {editando ? "Pronto" : "Editar"}
    </button>
  </div>
  <div class="grade">
    {#each slots as slot, i}
      <div class="slot-caixa">
        <button
          class="slot"
          class:vazio={!slot}
          class:aceso={aceso === i}
          class:editando
          onclick={() => clicar(i)}
          title={slot?.caminho ?? "Escolher arquivo"}
        >
          {#if slot}
            <span class="reticencias">{slot.nome}</span>
          {:else}
            <Icone nome="plus" tamanho={14} />
          {/if}
        </button>
        {#if editando && slot}
          <button class="limpar" aria-label="Esvaziar botão" onclick={() => api.cartucheiraDefinir(i, null)}>
            <Icone nome="x" tamanho={12} />
          </button>
        {/if}
      </div>
    {/each}
  </div>
</section>

<style>
  .editar {
    margin-left: auto;
    font-size: 13px;
    padding: 2px 8px;
    color: var(--texto-3);
  }
  .editar.on {
    color: var(--tag-programa);
    border-color: #4a6aa5;
  }
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
    border-style: dashed;
    color: var(--apagado);
    background: transparent;
  }
  .slot.editando {
    border-color: #4a6aa5;
  }
  .slot.aceso {
    background: #5a3a0e;
    border-color: var(--pasta);
    color: var(--tag-vinheta);
  }
  .limpar {
    position: absolute;
    top: -6px;
    right: -6px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--no-ar);
    border: none;
    color: #fff;
  }
</style>
