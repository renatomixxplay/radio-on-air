<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Bloqueios, type Retrato } from "./api";
  import Icone from "./Icone.svelte";

  let {
    r,
    senhaSessao,
    aoMudarSenha,
  }: { r: Retrato; senhaSessao: string; aoMudarSenha: (s: string) => void } = $props();

  const AREAS: { chave: keyof Bloqueios; nome: string }[] = [
    { chave: "agendamentos", nome: "Agendamentos" },
    { chave: "pastas", nome: "Pastas" },
    { chave: "cartucheira", nome: "Cartucheira" },
    { chave: "configuracoes", nome: "Configurações" },
  ];

  let nova = $state("");
  let repetir = $state("");
  let msgSenha = $state("");
  let erroSenha = $state(false);

  async function trocarPastaMusicas() {
    const p = await open({ directory: true, multiple: false, title: "Pasta de músicas" });
    if (typeof p === "string") await api.definirPasta("musicas", p);
  }

  async function alternarBloqueio(chave: keyof Bloqueios) {
    const b = { ...r.bloqueios, [chave]: !r.bloqueios[chave] };
    await api.bloqueiosDefinir(b, senhaSessao);
  }

  function aviso(texto: string, erro = false) {
    msgSenha = texto;
    erroSenha = erro;
  }

  async function salvarSenha() {
    if (nova.length < 4) return aviso("A senha precisa ter pelo menos 4 caracteres.", true);
    if (nova !== repetir) return aviso("As duas senhas não são iguais.", true);
    try {
      await api.senhaDefinir(senhaSessao, nova);
      aoMudarSenha(nova);
      nova = repetir = "";
      aviso("Senha salva.");
    } catch (e) {
      aviso(String(e), true);
    }
  }

  async function removerSenha() {
    await api.senhaDefinir(senhaSessao, "");
    await api.bloqueiosDefinir({ agendamentos: false, pastas: false, cartucheira: false, configuracoes: false }, "");
    aoMudarSenha("");
    aviso("Senha removida. Nada fica bloqueado.");
  }
</script>

<div class="aba">
  <div class="coluna">
    <section class="painel bloco">
      <div class="titulo">Ao abrir o programa</div>
      <button class="opcao" onclick={() => api.definirTocarAoAbrir(!r.tocar_ao_abrir)}>
        <span class="chave" class:on={r.tocar_ao_abrir}><span></span></span>
        <span>
          Entrar no ar sozinho
          <span class="suave">Começa a tocar assim que o programa abre (bom para quando a energia volta).</span>
        </span>
      </button>
    </section>

    <section class="painel bloco">
      <div class="titulo">Músicas</div>
      <div class="linha-campo">
        <span class="cor-pasta"><Icone nome="folder" tamanho={18} /></span>
        <span class="reticencias caminho">{r.pasta_musicas ?? "Nenhuma pasta escolhida"}</span>
        <button class="botao" onclick={trocarPastaMusicas}><Icone nome="folderOpen" tamanho={16} /> Trocar</button>
      </div>
      <div class="linha-campo">
        <span class="suave">{r.carregando ? "Lendo as pastas…" : `${r.total_musicas.toLocaleString("pt-BR")} músicas`}</span>
        <button class="botao" onclick={api.atualizarMusicas} title="Use se alguma música nova não apareceu">
          <Icone nome="refresh" tamanho={16} /> Atualizar músicas
        </button>
      </div>
    </section>

    <section class="painel bloco">
      <div class="titulo">Senha e bloqueios</div>
      {#if r.protegido}
        <div class="suave paragrafo">Pedir senha para abrir:</div>
        {#each AREAS as a}
          <button class="opcao" onclick={() => alternarBloqueio(a.chave)}>
            <span class="chave" class:on={r.bloqueios[a.chave]}><span></span></span>
            <span>{a.nome}</span>
          </button>
        {/each}
        <div class="separador"></div>
        <div class="suave paragrafo">Trocar a senha</div>
      {:else}
        <div class="suave paragrafo">Crie uma senha para bloquear agendamentos, pastas, cartucheira e configurações.</div>
      {/if}
      <div class="senhas">
        <input type="password" placeholder="Nova senha" bind:value={nova} />
        <input type="password" placeholder="Repetir a senha" bind:value={repetir} />
        <button class="botao" onclick={salvarSenha}>
          <Icone nome="cadeado" tamanho={16} />{r.protegido ? "Trocar" : "Criar senha"}
        </button>
      </div>
      {#if msgSenha}<div class="msg" class:erro={erroSenha}>{msgSenha}</div>{/if}
      {#if r.protegido}
        <button class="botao remover" onclick={removerSenha}>Remover senha</button>
      {/if}
    </section>
  </div>
</div>

<style>
  .aba {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px;
  }
  /* Ocupa a largura toda: blocos lado a lado quando cabem. */
  .coluna {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(420px, 1fr));
    align-items: start;
    gap: 10px;
  }
  .bloco {
    padding: 14px;
    gap: 8px;
  }
  .titulo {
    font-size: 16px;
    font-weight: 500;
    margin-bottom: 4px;
  }
  .suave {
    display: block;
    font-size: 13px;
  }
  .paragrafo {
    margin-bottom: 2px;
  }
  .opcao {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 0;
    border: none;
    background: transparent;
    text-align: left;
  }
  .opcao:active {
    transform: none;
  }
  .chave {
    width: 40px;
    height: 22px;
    border-radius: 999px;
    background: var(--borda-forte);
    position: relative;
    flex: none;
  }
  .chave span {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    transition: left 0.15s;
  }
  .chave.on {
    background: #238636;
  }
  .chave.on span {
    left: 21px;
  }
  .linha-campo {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .cor-pasta {
    color: var(--pasta);
    display: flex;
  }
  .caminho {
    flex: 1;
  }
  .linha-campo .suave {
    flex: 1;
  }
  .botao {
    height: 36px;
    padding: 0 12px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .senhas {
    display: flex;
    gap: 8px;
  }
  .senhas input {
    flex: 1;
    height: 36px;
    border-color: var(--borda-forte);
    border-radius: 8px;
  }
  .separador {
    border-top: 0.5px solid var(--borda);
    margin: 6px 0;
  }
  .msg {
    font-size: 13px;
    color: var(--tag-musica);
  }
  .msg.erro {
    color: #ff9b9b;
  }
  .remover {
    align-self: flex-start;
    color: #ff9b9b;
  }
</style>
