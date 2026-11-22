<script lang="ts">
  // Configura os 8 botões da cartucheira padrão e das cartucheiras agendadas
  // (com data de início e fim: no período, entram no lugar da padrão).
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type CartucheiraAgendada } from "./api";
  import Icone from "./Icone.svelte";
  import Data from "./Data.svelte";

  type Slot = { nome: string; caminho: string } | null;
  let { padrao, agendadas }: { padrao: Slot[]; agendadas: CartucheiraAgendada[] } = $props();

  /** Qual está aberta: null = a padrão; 0 = nova agendada (ainda não salva). */
  let sel = $state<number | null>(null);
  let nome = $state("");
  let inicio = $state("");
  let fim = $state("");
  let erro = $state("");
  /** Primeiro clique na lixeira arma; o segundo apaga. */
  let armado = $state(false);

  const atual = $derived(sel ? agendadas.find((a) => a.id === sel) ?? null : null);
  const slots = $derived(sel === null ? padrao : (atual?.botoes ?? []));
  const mudou = $derived(sel === 0 || (!!atual && (nome !== atual.nome || inicio !== atual.inicio || fim !== atual.fim)));

  function abrir(id: number | null) {
    sel = id;
    erro = "";
    armado = false;
    const a = id ? agendadas.find((x) => x.id === id) : null;
    nome = a?.nome ?? "";
    inicio = a?.inicio ?? "";
    fim = a?.fim ?? "";
  }

  function nova() {
    abrir(null);
    sel = 0;
  }

  async function salvar() {
    try {
      const id = await api.cartucheiraAgendadaSalvar(sel ?? 0, nome, inicio, fim);
      erro = "";
      nome = nome.trim();
      sel = id;
    } catch (e) {
      erro = String(e);
    }
  }

  async function apagar() {
    if (!armado) {
      armado = true;
      setTimeout(() => (armado = false), 3000);
      return;
    }
    if (sel) await api.cartucheiraAgendadaApagar(sel);
    abrir(null);
  }

  /** "2026-12-20" → "20/12/2026". */
  const br = (d: string) => d.split("-").reverse().join("/");

  async function escolher(i: number) {
    const arquivo = await open({
      multiple: false,
      directory: false,
      title: `Botão ${i + 1} da cartucheira`,
      filters: [{ name: "Áudio", extensions: ["mp3", "ogg", "opus", "flac", "wav", "m4a", "aac"] }],
    });
    if (typeof arquivo === "string") await api.cartucheiraDefinir(i, arquivo, sel || null);
  }
</script>

<div class="aba">
  <div class="escolha">
    <button class="cart" class:sel={sel === null} onclick={() => abrir(null)}>
      Padrão
    </button>
    {#each agendadas as a (a.id)}
      <button class="cart" class:sel={sel === a.id} class:passou={a.passou} onclick={() => abrir(a.id)} title={a.hoje ? "No ar hoje" : a.passou ? "Período já acabou" : ""}>
        {#if a.hoje}<span class="ponto"></span>{/if}
        <span class="reticencias">{a.nome}</span>
        <span class="datas mono">{br(a.inicio).slice(0, 5)} a {br(a.fim).slice(0, 5)}</span>
      </button>
    {/each}
    <button class="quadrado" class:sel={sel === 0} onclick={nova} title="Nova cartucheira agendada" aria-label="Nova cartucheira agendada">
      <Icone nome="plus" tamanho={20} />
    </button>
  </div>

  {#if sel !== null}
    <div class="painel periodo">
      <label class="campo nome">
        <span class="suave">Nome</span>
        <input type="text" bind:value={nome} placeholder="Ex.: Natal" />
      </label>
      <div class="campo">
        <span class="suave">Início</span>
        <Data bind:value={inicio} />
      </div>
      <div class="campo">
        <span class="suave">Fim</span>
        <Data bind:value={fim} />
      </div>
      <div class="acoes">
        {#if mudou}
          <button class="botao" onclick={() => (sel === 0 ? abrir(null) : abrir(sel))}>Cancelar</button>
          <button class="botao salvar" onclick={salvar}>Salvar</button>
        {/if}
        {#if sel !== 0}
          <button class="quadrado lixo" class:armado onclick={apagar} title={armado ? "Clique de novo para apagar" : "Apagar esta cartucheira"} aria-label="Apagar esta cartucheira">
            <Icone nome="lixeira" tamanho={18} />
          </button>
        {/if}
      </div>
    </div>
    {#if erro}<div class="erro">{erro}</div>{/if}
  {/if}

  {#if sel === 0}
    <div class="suave dica">Escolha o nome e as datas e clique em Salvar. Depois, escolha os 8 botões.</div>
  {:else}
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
            <button class="quadrado lixo" onclick={() => api.cartucheiraDefinir(i, null, sel || null)} title="Esvaziar" aria-label="Esvaziar">
              <Icone nome="lixeira" tamanho={18} />
            </button>
          {:else}
            <span class="quadrado-vazio"></span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .aba {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 10px 20px;
  }
  .suave {
    font-size: 13px;
  }
  .escolha {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 4px 0 10px;
  }
  .cart {
    height: 40px;
    max-width: 280px;
    padding: 0 14px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
  }
  .cart.passou {
    color: var(--texto-3);
  }
  .cart.sel,
  .quadrado.sel {
    border-color: var(--azul);
    color: var(--texto);
  }
  .datas {
    font-size: 13px;
    color: var(--texto-2);
    flex: none;
  }
  .ponto {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--verde);
    flex: none;
  }
  .periodo {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 10px;
    padding: 12px 14px;
    margin-bottom: 10px;
  }
  .campo {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .campo input {
    height: 38px;
    font-size: 15px;
  }
  .nome {
    flex: 1;
    min-width: 160px;
  }
  .acoes {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }
  .salvar {
    background: #238636;
    border-color: #2ea043;
    color: #fff;
  }
  .dica {
    margin: 0 4px;
    font-size: 14px;
  }
  .erro {
    margin: -4px 4px 10px;
    color: #ff9b9b;
    font-size: 14px;
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
    height: 38px;
    padding: 0 12px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .slot .botao {
    height: 34px;
  }
  .quadrado,
  .quadrado-vazio {
    width: 38px;
    height: 38px;
    flex: none;
  }
  .slot .quadrado,
  .slot .quadrado-vazio {
    width: 34px;
    height: 34px;
  }
  .quadrado {
    padding: 0;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--texto-2);
  }
  .escolha .quadrado {
    height: 40px;
    width: 40px;
  }
  .lixo:hover,
  .lixo.armado {
    color: #fff;
    background: var(--no-ar);
    border-color: var(--no-ar);
  }
</style>
