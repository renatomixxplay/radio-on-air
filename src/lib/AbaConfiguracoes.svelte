<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount, untrack } from "svelte";
  import { api, type Audio, type Bloqueios, type Placas, type Retrato, type Transmissao } from "./api";
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

  // ---------- rádio ----------
  // Cópias para editar: começam com o valor salvo.
  let nome = $state(untrack(() => r.nome_radio));
  async function salvarNome() {
    if (nome.trim() !== r.nome_radio) await api.nomeRadioDefinir(nome);
  }

  // ---------- placas ----------
  let placas = $state<Placas>({ saidas: [], entradas: [] });
  let buscandoPlacas = $state(false);
  async function lerPlacas() {
    buscandoPlacas = true;
    placas = await api.placas();
    buscandoPlacas = false;
  }
  onMount(lerPlacas);
  /** Lista com a placa salva mesmo que ela não esteja ligada agora. */
  function opcoes(lista: { id: string; nome: string }[], atual: string | null) {
    return atual && !lista.some((p) => p.id === atual) ? [...lista, { id: atual, nome: `${atual} (desligada)` }] : lista;
  }
  async function trocarPlaca(chave: keyof Audio, valor: string) {
    await api.audioDefinir({ ...r.audio, [chave]: valor || null });
  }

  // ---------- gravação ----------
  let erroGravacao = $state("");
  async function gravacao(mudar: Partial<Retrato["gravacao"]>) {
    try {
      await api.gravacaoDefinir({ ...r.gravacao, ...mudar });
      erroGravacao = "";
    } catch (e) {
      erroGravacao = String(e);
    }
  }
  async function pastaGravacao() {
    const p = await open({ directory: true, multiple: false, title: "Pasta das gravações" });
    if (typeof p === "string") await gravacao({ pasta: p });
  }

  // ---------- transmissão ----------
  let tx = $state<Transmissao>(untrack(() => structuredClone($state.snapshot(r.transmissao))));
  let erroTx = $state("");
  let txSalvo = $state(false);
  async function salvarTx(mudar: Partial<Transmissao> = {}) {
    Object.assign(tx, mudar);
    try {
      await api.transmissaoDefinir($state.snapshot(tx) as Transmissao);
      erroTx = "";
      txSalvo = true;
      setTimeout(() => (txSalvo = false), 2000);
    } catch (e) {
      erroTx = String(e);
      if (mudar.ligada) tx.ligada = false;
    }
  }
  const estadoTx = $derived.by(() => {
    const t = r.transmissao_estado;
    if (!r.transmissao.ligada) return { texto: "Desligada", cor: "" };
    if (t === "conectado") return { texto: "No ar", cor: "ok" };
    if (t === "conectando") return { texto: "Conectando…", cor: "" };
    if (t === "caiu") return { texto: "Caiu, tentando de novo…", cor: "erro" };
    if (t.startsWith("erro")) return { texto: `Erro: ${t.replace(/^erro:?\s*/, "")} (tentando de novo…)`, cor: "erro" };
    return { texto: t, cor: "" };
  });

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
    <h3 class="grupo">Rádio</h3>
    <section class="painel cartao">
      <div class="linha item">
        <div class="textos">
          <span>Nome da rádio</span>
          <span class="suave">Aparece no topo do programa e na transmissão.</span>
        </div>
        <input
          class="largo"
          bind:value={nome}
          onblur={salvarNome}
          onkeydown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()}
        />
      </div>
    </section>

    <h3 class="grupo">Placa de som</h3>
    <section class="painel cartao">
      <div class="linha item">
        <div class="textos">
          <span>Saída principal</span>
          <span class="suave">O que vai ao ar. Trocar interrompe a música que está tocando.</span>
        </div>
        <select class="largo" value={r.audio.saida ?? ""} onchange={(e) => trocarPlaca("saida", e.currentTarget.value)}>
          <option value="">Padrão do sistema</option>
          {#each opcoes(placas.saidas, r.audio.saida) as p (p.id)}<option value={p.id}>{p.nome}</option>{/each}
        </select>
      </div>
      <div class="linha item">
        <div class="textos">
          <span>Pré-escuta</span>
          <span class="suave">Para ouvir no fone antes de tocar.</span>
        </div>
        <select class="largo" value={r.audio.pre_escuta ?? ""} onchange={(e) => trocarPlaca("pre_escuta", e.currentTarget.value)}>
          <option value="">Padrão do sistema</option>
          {#each opcoes(placas.saidas, r.audio.pre_escuta) as p (p.id)}<option value={p.id}>{p.nome}</option>{/each}
        </select>
      </div>
      <div class="linha item">
        <div class="textos">
          <span>Entrada (mesa)</span>
          <span class="suave">De onde vêm a gravação e a transmissão.</span>
        </div>
        <select class="largo" value={r.audio.entrada ?? ""} onchange={(e) => trocarPlaca("entrada", e.currentTarget.value)}>
          <option value="">Padrão do sistema</option>
          {#each opcoes(placas.entradas, r.audio.entrada) as p (p.id)}<option value={p.id}>{p.nome}</option>{/each}
        </select>
      </div>
      <div class="linha item">
        <div class="textos">
          <span class="suave">Ligou uma placa agora? Atualize a lista.</span>
        </div>
        <button class="botao" onclick={lerPlacas} disabled={buscandoPlacas}>
          <Icone nome="refresh" tamanho={16} />{buscandoPlacas ? "Procurando…" : "Atualizar"}
        </button>
      </div>
    </section>

    <h3 class="grupo">Gravação</h3>
    <section class="painel cartao">
      <div class="linha item">
        <div class="textos">
          <span>Gravar a entrada</span>
          <span class="suave">Grava o que chega da mesa, sempre que o programa está aberto. Gravações com mais de 30 dias são apagadas.</span>
        </div>
        <button class="chave" class:on={r.gravacao.ligada} onclick={() => gravacao({ ligada: !r.gravacao.ligada })} aria-label="Gravar a entrada">
          <span></span>
        </button>
      </div>
      <div class="linha item">
        <div class="textos">
          <span>Pasta das gravações</span>
          <span class="reticencias suave">{r.gravacao.pasta ?? "Nenhuma pasta escolhida"}</span>
        </div>
        <button class="botao" onclick={pastaGravacao}><Icone nome="folderOpen" tamanho={16} /> Trocar</button>
      </div>
      <div class="linha item">
        <div class="textos">
          <span>Qualidade (MP3 mono)</span>
        </div>
        <div class="chips">
          {#each [16, 24, 32] as b}
            <button class="chip" class:on={r.gravacao.bitrate === b} onclick={() => gravacao({ bitrate: b })}>{b}</button>
          {/each}
          <span class="suave">kbps</span>
        </div>
      </div>
      <div class="linha item">
        <div class="textos">
          <span>Um arquivo a cada</span>
          <span class="suave">Pasta do dia (dia-mês-ano) e arquivo com a hora do início, ex.: 15-11-2026/14-00.mp3</span>
        </div>
        <div class="chips">
          <button class="chip" class:on={r.gravacao.minutos === 30} onclick={() => gravacao({ minutos: 30 })}>30 min</button>
          <button class="chip" class:on={r.gravacao.minutos === 60} onclick={() => gravacao({ minutos: 60 })}>1 hora</button>
        </div>
      </div>
      {#if erroGravacao}<div class="msg erro">{erroGravacao}</div>{/if}
    </section>

    <h3 class="grupo">Transmissão</h3>
    <section class="painel cartao">
      <div class="linha item">
        <div class="textos">
          <span>Transmitir a entrada</span>
          <span class="suave estado {estadoTx.cor}">{estadoTx.texto}</span>
        </div>
        <button class="chave" class:on={r.transmissao.ligada} onclick={() => salvarTx({ ligada: !r.transmissao.ligada })} aria-label="Transmitir">
          <span></span>
        </button>
      </div>
      <div class="linha item">
        <div class="textos"><span>Servidor (Icecast)</span></div>
        <input class="largo" bind:value={tx.servidor} placeholder="servidor.exemplo.com" spellcheck="false" />
      </div>
      <div class="linha item">
        <div class="textos"><span>Porta</span></div>
        <input type="number" min="1" max="65535" bind:value={tx.porta} />
      </div>
      <div class="linha item">
        <div class="textos"><span>Senha</span></div>
        <input type="password" bind:value={tx.senha} autocomplete="off" />
      </div>
      <div class="linha item">
        <div class="textos"><span>Ponto de montagem</span></div>
        <input bind:value={tx.ponto} placeholder="/live" spellcheck="false" />
      </div>
      <div class="linha item">
        <div class="textos"><span>Qualidade (MP3)</span></div>
        <select bind:value={tx.bitrate}>
          {#each [32, 48, 64, 96, 128, 192, 256, 320] as b}<option value={b}>{b} kbps</option>{/each}
        </select>
        <select bind:value={tx.samplerate}>
          {#each [22050, 32000, 44100, 48000] as s}<option value={s}>{s} Hz</option>{/each}
        </select>
        <div class="chips">
          <button class="chip" class:on={!tx.estereo} onclick={() => (tx.estereo = false)}>Mono</button>
          <button class="chip" class:on={tx.estereo} onclick={() => (tx.estereo = true)}>Estéreo</button>
        </div>
      </div>
      <div class="linha item">
        <div class="textos">
          <span class="suave">{txSalvo ? "Salvo." : "Depois de mudar os dados, salve."}</span>
        </div>
        <button class="botao" onclick={() => salvarTx()}>Salvar</button>
      </div>
      {#if erroTx}<div class="msg erro">{erroTx}</div>{/if}
      {#if r.erro_captura}<div class="msg erro">{r.erro_captura}</div>{/if}
    </section>

    <h3 class="grupo">Ao abrir o programa</h3>
    <section class="painel cartao">
      <div class="linha item">
        <div class="textos">
          <span>Entrar no ar sozinho</span>
          <span class="suave">Começa a tocar assim que o programa abre (bom para quando a energia volta).</span>
        </div>
        <button class="chave" class:on={r.tocar_ao_abrir} onclick={() => api.definirTocarAoAbrir(!r.tocar_ao_abrir)} aria-label="Entrar no ar sozinho">
          <span></span>
        </button>
      </div>
      <div class="linha item">
        <div class="textos">
          <span>Abrir em tela cheia</span>
          <span class="suave">O programa já abre ocupando a tela toda. Para entrar ou sair na hora: botão no topo ou tecla F11.</span>
        </div>
        <button class="chave" class:on={r.abrir_tela_cheia} onclick={() => api.definirAbrirTelaCheia(!r.abrir_tela_cheia)} aria-label="Abrir em tela cheia">
          <span></span>
        </button>
      </div>
    </section>
    <h3 class="grupo">Músicas</h3>
    <section class="painel cartao">
      <div class="linha item">
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
        <button class="botao" onclick={api.atualizarMusicas}><Icone nome="refresh" tamanho={16} /> Atualizar</button>
      </div>
      <div class="linha item">
        <div class="textos">
          <span>Vinheta entre as músicas</span>
          <span class="suave">Entre uma música e outra, toca uma vinheta sorteada dos botões da cartucheira.</span>
        </div>
        <button
          class="chave"
          class:on={r.vinheta_entre_musicas}
          onclick={() => api.definirVinhetaEntreMusicas(!r.vinheta_entre_musicas)}
          aria-label="Vinheta entre as músicas"
        >
          <span></span>
        </button>
      </div>
    </section>
    <h3 class="grupo">Senha e bloqueios</h3>
    <section class="painel cartao">
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
    </section>
  </div>
</div>
<style>
  .aba {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 10px 20px;
  }
  .coluna {
    display: flex;
    flex-direction: column;
  }
  /* Cada grupo é um cartão próprio; o título fica acima, com espaço entre eles. */
  .grupo {
    margin: 14px 4px 8px;
    font-size: 15px;
    font-weight: 500;
    color: var(--texto);
  }
  .grupo:first-child {
    margin-top: 4px;
  }
  .cartao {
    overflow: hidden;
  }
  .item {
    padding: 12px 14px;
    gap: 12px;
  }
  .cartao > .item:last-child,
  .cartao > .msg:last-child {
    border-bottom: none;
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
  /* Todos os botões da tela com a mesma largura. */
  .botao {
    width: 130px;
    height: 36px;
    padding: 0 12px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
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
  .msg {
    padding: 0 14px 8px;
    font-size: 13px;
    color: var(--tag-musica);
  }
  .msg.erro {
    color: #ff9b9b;
  }
  .largo {
    width: 260px;
  }
  select {
    height: 36px;
    border-radius: 8px;
    flex: none;
  }
  .chips {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .chip {
    height: 34px;
    padding: 0 12px;
    border-radius: 8px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .chip.on {
    background: #2b3a55;
    border-color: #4a6aa5;
    color: #cfe0ff;
  }
  .estado.ok {
    color: var(--tag-musica);
  }
  .estado.erro {
    color: #ff9b9b;
  }
  .remover {
    color: #ff9b9b;
  }
</style>
