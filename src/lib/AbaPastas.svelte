<script lang="ts">
  // Pastas que aparecem no painel "Pastas" da tela No ar (em ordem alfabética).
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Pasta } from "./api";
  import Icone from "./Icone.svelte";

  let { pastas }: { pastas: Pasta[] } = $props();

  async function adicionar() {
    const p = await open({ directory: true, multiple: true, title: "Adicionar pastas" });
    const lista = typeof p === "string" ? [p] : (p ?? []);
    for (const c of lista) await api.pastaAdicionar(c);
  }
</script>

<div class="aba">
  <div class="topo">
    <div>
      <h3 class="grupo">Pastas</h3>
      <div class="suave">Aparecem no painel Pastas da tela No ar, em ordem alfabética.</div>
    </div>
    <button class="botao" onclick={adicionar}><Icone nome="plus" tamanho={16} /> Adicionar pasta</button>
  </div>
  <section class="painel cartao">
    {#each pastas as p (p.caminho)}
      <div class="linha item">
        <span class="cor-pasta"><Icone nome="folder" tamanho={20} /></span>
        <div class="textos">
          <span class="reticencias">{p.nome}</span>
          <span class="reticencias suave caminho">{p.caminho}</span>
        </div>
        <button class="quadrado lixo" onclick={() => api.pastaRemover(p.caminho)} title="Tirar da lista" aria-label="Tirar da lista">
          <Icone nome="lixeira" tamanho={18} />
        </button>
      </div>
    {:else}
      <div class="vazio">Nenhuma pasta ainda. Use "Adicionar pasta".</div>
    {/each}
  </section>
</div>

<style>
  .aba {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 10px 20px;
  }
  .topo {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 12px;
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
  .cartao {
    overflow: hidden;
  }
  .item {
    padding: 12px 14px;
    gap: 12px;
  }
  .cartao > .item:last-child {
    border-bottom: none;
  }
  .botao {
    height: 38px;
    padding: 0 14px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .cor-pasta {
    color: var(--pasta);
    display: flex;
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
  .quadrado {
    width: 34px;
    height: 34px;
    padding: 0;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--texto-2);
    flex: none;
  }
  .lixo:hover {
    color: #fff;
    background: var(--no-ar);
    border-color: var(--no-ar);
  }
</style>
