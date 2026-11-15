<script lang="ts">
  // Configura os 8 botões da cartucheira.
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "./api";
  import Icone from "./Icone.svelte";

  let { slots }: { slots: ({ nome: string; caminho: string } | null)[] } = $props();

  async function escolher(i: number) {
    const arquivo = await open({
      multiple: false,
      directory: false,
      title: `Botão ${i + 1} da cartucheira`,
      filters: [{ name: "Áudio", extensions: ["mp3", "ogg", "opus", "flac", "wav", "m4a", "aac"] }],
    });
    if (typeof arquivo === "string") await api.cartucheiraDefinir(i, arquivo);
  }
</script>

<div class="aba">
  <div class="topo">
    <h3 class="grupo">Cartucheira</h3>
    <div class="suave">Escolha o áudio de cada um dos 8 botões.</div>
  </div>
  <div class="grade">
      {#each slots as slot, i}
        <div class="painel linha slot">
          <span class="numero mono">{i + 1}</span>
          <div class="textos">
            {#if slot}
              <span class="reticencias">{slot.nome}</span>
              <span class="reticencias suave caminho">{slot.caminho}</span>
            {:else}
              <span class="suave">Vazio</span>
            {/if}
          </div>
          <button class="botao" onclick={() => escolher(i)}>
            <Icone nome="folderOpen" tamanho={16} />Escolher
          </button>
          {#if slot}
            <button class="quadrado lixo" onclick={() => api.cartucheiraDefinir(i, null)} title="Esvaziar" aria-label="Esvaziar">
              <Icone nome="lixeira" tamanho={18} />
            </button>
          {:else}
            <span class="quadrado-vazio"></span>
          {/if}
        </div>
      {/each}
  </div>
</div>

<style>
  .aba {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 10px 20px;
  }
  .topo {
    margin: 4px 4px 10px;
  }
  .grupo {
    margin: 0 0 2px;
    font-size: 15px;
    font-weight: 500;
  }
  .suave {
    font-size: 13px;
  }
  .grade {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .slot {
    flex-direction: row;
    padding: 12px 14px;
    border-bottom: 0.5px solid var(--borda);
  }
  .numero {
    width: 34px;
    height: 34px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--painel-2);
    color: var(--texto-2);
    flex: none;
  }
  .textos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .caminho {
    font-size: 12px;
  }
  .botao {
    height: 34px;
    padding: 0 12px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .quadrado,
  .quadrado-vazio {
    width: 34px;
    height: 34px;
    flex: none;
  }
  .quadrado {
    padding: 0;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--texto-2);
  }
  .lixo:hover {
    color: #fff;
    background: var(--no-ar);
    border-color: var(--no-ar);
  }
</style>
