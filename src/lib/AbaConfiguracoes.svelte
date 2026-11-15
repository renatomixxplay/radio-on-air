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
  <section class="painel caixa">
    <div class="cabeca">
      <div class="titulo">Configurações</div>
    </div>
    <div class="lista">
      <div class="grupo">Ao abrir o programa</div>
      <div class="linha item">
        <div class="textos">
          <span>Entrar no ar sozinho</span>
          <span class="suave">Começa a tocar assim que o programa abre (bom para quando a energia volta).</span>
        </div>
        <button class="chave" class:on={r.tocar_ao_abrir} onclick={() => api.definirTocarAoAbrir(!r.tocar_ao_abrir)} aria-label="Entrar no ar sozinho">
          <span></span>
        </button>
      </div>

      <div class="grupo">Músicas</div>
      <div class="linha item">
        <span class="cor-pasta"><Icone nome="folder" tamanho={20} /></span>
        <div class="textos">
          <span>Pasta de músicas</span>
          <span class="reticencias suave">{r.pasta_musicas ?? "Nenhuma pasta escolhida"}</span>
        </div>
        <button class="botao" onclick={trocarPastaMusicas}><Icone nome="folderOpen" tamanho={16} /> Trocar</button>
      </div>
      <div class="linha item">
        <div class="textos">
          <span>{r.carregando ? "Lendo as pastas…" : `${r.total_musicas.toLocaleString("pt-BR")} músicas no acervo`}</span>
          <span class="suave">Use se alguma música nova não apareceu na busca.</span>
        </div>
        <button class="botao" onclick={api.atualizarMusicas}><Icone nome="refresh" tamanho={16} /> Atualizar músicas</button>
      </div>

      <div class="grupo">Senha e bloqueios</div>
      {#if r.protegido}
        {#each AREAS as a}
          <div class="linha item">
            <div class="textos">
              <span>Pedir senha para abrir {a.nome}</span>
            </div>
            <button class="chave" class:on={r.bloqueios[a.chave]} onclick={() => alternarBloqueio(a.chave)} aria-label={`Bloquear ${a.nome}`}>
              <span></span>
            </button>
          </div>
        {/each}
      {/if}
      <div class="linha item">
        <div class="textos">
          <span>{r.protegido ? "Trocar a senha" : "Criar senha"}</span>
          <span class="suave">
            {r.protegido ? "Mínimo de 4 caracteres." : "Com senha, dá para bloquear agendamentos, pastas, cartucheira e configurações."}
          </span>
        </div>
        <input type="password" placeholder="Nova senha" bind:value={nova} />
        <input type="password" placeholder="Repetir" bind:value={repetir} />
        <button class="botao" onclick={salvarSenha}>
          <Icone nome="cadeado" tamanho={16} />{r.protegido ? "Trocar" : "Criar"}
        </button>
      </div>
      {#if msgSenha}<div class="msg" class:erro={erroSenha}>{msgSenha}</div>{/if}
      {#if r.protegido}
        <div class="linha item">
          <div class="textos">
            <span>Remover senha</span>
            <span class="suave">Tudo fica liberado de novo.</span>
          </div>
          <button class="botao remover" onclick={removerSenha}>Remover</button>
        </div>
      {/if}
    </div>
  </section>
</div>

<style>
  .aba {
    flex: 1;
    min-height: 0;
    display: flex;
    padding: 10px;
  }
  .caixa {
    flex: 1;
    min-width: 0;
  }
  .cabeca {
    padding: 14px;
    border-bottom: 0.5px solid var(--borda);
  }
  .titulo {
    font-size: 16px;
    font-weight: 500;
  }
  /* Faixa de título de cada grupo: bem visível para separar as seções. */
  .grupo {
    padding: 10px 14px;
    font-size: 13px;
    font-weight: 500;
    color: var(--texto-2);
    background: var(--painel-2);
    border-top: 1px solid var(--borda-forte);
    border-bottom: 1px solid var(--borda-forte);
  }
  .grupo:first-child {
    border-top: none;
  }
  .item {
    padding: 12px 14px;
    gap: 12px;
    border-bottom: 1px solid #2c313b;
  }
  .textos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .suave {
    font-size: 13px;
  }
  .cor-pasta {
    color: var(--pasta);
    display: flex;
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
  input {
    width: 180px;
    height: 36px;
    border-color: var(--borda-forte);
    border-radius: 8px;
    flex: none;
  }
  .chave {
    width: 44px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 999px;
    background: var(--borda-forte);
    position: relative;
    flex: none;
  }
  .chave:active {
    transform: none;
  }
  .chave span {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    transition: left 0.15s;
  }
  .chave.on {
    background: #238636;
  }
  .chave.on span {
    left: 23px;
  }
  .msg {
    padding: 0 14px 8px;
    font-size: 13px;
    color: var(--tag-musica);
  }
  .msg.erro {
    color: #ff9b9b;
  }
  .remover {
    color: #ff9b9b;
  }
</style>
