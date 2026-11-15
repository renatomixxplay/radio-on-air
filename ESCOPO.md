# Radio On Air — Escopo

Software de automação de rádio (rádio DJ) para Linux, open source, feito para **rádios comunitárias**.

> Documento vivo. Cada recurso só entra depois de aprovado.
> Legenda: ✅ feito · 💡 sugestão (aguardando aprovação) · ❓ pendente / a decidir · 🧪 feito, falta testar

**Situação em 15/11/2026:** Fases 1 e 2 prontas e rodando no Mac. Próximo passo: **testar tudo com calma** (seção 12) antes da Fase 3.

---

## 1. Princípios

- ✅ **Leve** — precisa rodar bem em PCs modestos e antigos.
- ✅ **Programa de desktop** — janela própria, instalado no computador (não é site/navegador).
- ✅ **Arquivos locais** — músicas e áudios ficam em pastas do próprio computador.
- ✅ **Sem banco de dados** — tudo em pastas + arquivos de texto. Backup = copiar a pasta.
- ✅ **Open source, licença GPL-3.0.**
- ✅ **Tema escuro**, visual simples: ícones no lugar de texto, letras grandes, uma fonte só (a do sistema).
- ✅ Interface só em **português**.
- ✅ Microfone fica na **mesa de som** (o programa não captura microfone).

## 2. Tecnologia

| Parte | Escolha | Por quê |
|---|---|---|
| Janela do programa | **Tauri 2** (Rust) | Instalador pequeno, usa pouca memória |
| Tela | **Svelte 5** (SvelteKit estático) | Telas pequenas e rápidas |
| Motor de áudio | **Liquidsoap 2.4** (roda escondido) | Feito para rádio: ducking, Icecast, gravação, 24h estável |
| Dados | Arquivos de texto/JSON | Fácil de ler, editar e fazer backup |

- ✅ Alvo: Linux (Ubuntu/Debian) — `.deb` e AppImage.
- 💡 Possivelmente **Windows** no futuro (`.exe`/`.msi`; o Liquidsoap tem versão Windows).
- ✅ Desenvolvimento no **macOS** (Intel): Liquidsoap compilado isolado em `~/.opam` (sem Homebrew mexer no sistema).
- ✅ O motor de áudio **liga sozinho ao abrir o programa**, em segundo plano (no Mac leva ~4 s).
- ✅ No Mac a saída tem uma folga de **1 s de áudio** contra travadinhas (no Linux não precisa).

### Arquivos de dados

Ficam em `~/.local/share/org.radioonair.app/` (Linux) ou `~/Library/Application Support/org.radioonair.app/` (Mac):

| Arquivo | Conteúdo |
|---|---|
| `config.json` | Pasta de músicas, pastas, cartucheira, tocar ao abrir, senha (hash) e bloqueios |
| `indice.txt` | Lista de nomes das músicas (o programa abre na hora mesmo com 100 mil) |
| `agendamentos.json` | Agendamentos |
| `rodizio.json` | Último arquivo tocado de cada pasta no modo "só um" |
| `motor.liq` / `motor.log` | Script e avisos do motor de áudio (diagnóstico) |

## 3. Telas

Navegação no topo: **No ar · Agendamentos · Pastas · Cartucheira · Configurações**.
Todas as abas de configuração seguem o mesmo estilo: **título acima, cartões separados embaixo**.

### 3.1 No ar ✅

```
┌──────────────────────────────────────────────────────┐
│ Radio On Air   abas…        [ligando o áudio…] NO AR 14:32:07 │
├─────────────────────────────────┬────────────────────┤
│ 📅 agendamento (atual/próximo)  │ Cartucheira (8)    │
│ Música / artista / barra / tempo│                    │
│   🎙  ▶/⏸  🔁  ⏭               │                    │
├────────────┬────────────────────┴──┬─────────────────┤
│ Músicas    │ Fila (até 10)         │ Pastas          │
└────────────┴───────────────────────┴─────────────────┘
```

- **Topo:** NO AR / FORA DO AR, relógio grande, "ligando o áudio…" enquanto o motor sobe, cadeado quando a senha está desbloqueada.
- **Tocando agora:**
  - No lugar do título, uma linha 📅 de agendamento (amarelo transparente): **Por cima:** … / **Depois desta:** … / **Próximo:** HH:MM … / "Nenhum agendamento".
  - Música e artista, **barra de progresso** (vermelha nos 10 s finais), tempo tocado e o que falta.
  - Botões só com ícone: **Ducking · Tocar/Pausar · Repetir · Passar** (sem Parar).
  - **Ducking:** abaixa a música para 20% (rampa suave); a cartucheira não abaixa.
  - **Repetir:** a música atual toca de novo no fim; a fila espera.
  - **Passar** não funciona durante comercial/hora certa (agendamento não se pula).
- **Cartucheira:** 8 botões fixos; só toca (configuração na aba Cartucheira).
- **Músicas:** busca por música ou artista (sem acento/maiúscula), 🎧 pré-escuta e ➕ adicionar. Ícone de pasta troca a pasta de músicas.
- **Fila:** ícone colorido por tipo (🎵 verde música, 🎙 amarelo pasta/programa, ✨ roxo vinheta), duração, lixeira, arrastar pelos 6 pontinhos para reordenar. Topo mostra a lista de músicas ativa e "N/10 · total".
- **Pastas:** seletor com as pastas da aba Pastas (ordem alfabética) e os arquivos delas, com 🎧 e ➕.
- **Pré-escuta:** o 🎧 vira ■ e uma barrinha mostra o andamento; **clicar na linha pula** para aquele ponto. Toca numa saída separada e não mexe no que está no ar (por enquanto na mesma placa de som).
- Nas listas não se arrasta: adiciona-se só pelo ➕.

### 3.2 Agendamentos ✅

- Lista (cada agendamento em um cartão): horário, ícone do tipo, pastas e dias (S T Q Q S S D).
- Filtro por dia da semana; botão **Novo**.
- Editor ao lado: horário, dias, tipo (Lista · Comercial · Com ducking · Hora certa), pastas anexadas (arrastar para reordenar, **Todos / Só um**), salvar e excluir.

### 3.3 Pastas ✅
- Pastas que aparecem no painel Pastas do No ar (adicionar várias de uma vez, tirar). Ordem alfabética.

### 3.4 Cartucheira ✅
- 8 cartões (2 colunas): número, nome do áudio, **Escolher** e lixeira.

### 3.5 Configurações ✅ (parte)
- ✅ **Entrar no ar sozinho** ao abrir o programa.
- ✅ Pasta de músicas (**Trocar**) e **Atualizar** (relê o acervo).
- ✅ **Senha** (criar, trocar, remover) e **bloqueios** por área: Agendamentos, Pastas, Cartucheira, Configurações. Desbloqueia uma vez e vale até apertar o cadeado. Proteção contra mexida sem querer — não é segurança forte.
- ❓ Ainda por alinhar: saídas de áudio / placa da pré-escuta, transmissão, gravação, regras da fila.

## 4. Músicas e índice ✅

- Acervo de ~**100 mil músicas**, em **subpastas**.
- Padrão de nome `Artista - Música.mp3`; fora do padrão, o nome inteiro vira a música e o artista fica em branco.
- Busca em memória só pelos **nomes** dos arquivos.
- `indice.txt` para abrir na hora; atualização **automática** quando entra/sai/renomeia música; botão **Atualizar** se der erro. Sem varredura ao abrir.
- Duração lida do **cabeçalho** do arquivo só quando ele entra na fila (sem decodificar o áudio).
- ❓ Linux com muitas subpastas pode precisar aumentar `fs.inotify.max_user_watches` na instalação.

## 5. Fila ✅

- Até **10 itens**; o automático garante o **mínimo de 3** (quando cai para 3, puxa mais 1).
- O automático puxa aleatório da **lista de músicas ativa** (agendamento tipo Lista) ou do acervo inteiro.
- 💡 Não repetir a mesma música por X horas.
- 💡 Não repetir o mesmo artista por X minutos.
- 💡 Crossfade entre músicas.

## 6. Agendamentos ✅

Por **horário + dias da semana**. O programa só preenche/executa — não existe "sequência/receita".

| Tipo | Comportamento |
|---|---|
| 🎵 **Lista de músicas** | A partir do horário, a fila puxa **aleatório** da pasta. Vale até a próxima lista. Ao abrir no meio do dia, vale a última lista que já passou. |
| 📢 **Comercial** | Espera a música acabar e toca as pastas anexadas. |
| 🎙 **Comercial com ducking** | Entra **por cima**: a música cai para 20% quando há voz e sobe nos silêncios. Se o bloco **não cabe** no que falta da música, ela **repete por baixo** e, no fim da fala, **fade de 3 s** e próxima. |
| 🕐 **Hora certa** | Toca o arquivo com a hora atual no nome (`HRS08_MIN10`, `08-10`, `08h10`, `0810`, `8` = hora cheia). Espera a música acabar. |

- **Agendamento não entra na fila e não se pula:** comercial e hora certa tocam numa sequência própria antes da fila. Passar não pula agendamento; se pular uma música, o comercial que estava esperando continua garantido.
- Fora do ar, comercial e hora certa **não são guardados** para depois (lista de músicas vale mesmo fora do ar).
- Várias **pastas** por agendamento, tocando **na ordem anexada**; cada pasta **Todos** ou **Só um** (rodízio salvo).
- Dentro da pasta: **ordem alfabética natural** (`2` antes de `10`). Abertura/fechamento = nomear em ordem.
- **Validade** no nome: `Ofertas [12-10-2025].mp3` toca até 12/10/2025 inclusive; vencido é pulado (fica na pasta). Só comerciais. Na tela o nome aparece sem a data.
- Cores: agendamentos em **amarelo**; hora certa em azul.
- 💡 Hora certa "repetir toda hora" (hoje é um agendamento por horário).
- ❓ Hora certa: confirmar se espera a música (atual) ou entra por cima com ducking.

## 7. Áudio (Fase 3)

- ✅ Saída principal (caixa de som / mesa).
- ✅ Pré-escuta numa saída separada (falta escolher a placa).
- ❓ **Transmissão pela internet** (Icecast): servidor, porta, ponto de montagem, senha, qualidade.
- ❓ **Gravação** do que vai ao ar. 💡 1 arquivo por hora; 💡 apagar após X dias.
- 💡 Nível do ducking configurável.

## 8. Histórico

- 💡 Um arquivo de texto por dia com o que tocou (`historico/AAAA-MM-DD.txt`) — relatório e base para não repetir.

## 9. Leveza

Metas para PC antigo (2 núcleos, 4 GB, HD mecânico): **< ~200 MB** de memória, **< ~5%** de CPU, abrir em poucos segundos.

Medido no Mac (versão de desenvolvimento), tocando: programa ~30 MB, tela ~55 MB, Liquidsoap ~60 MB → **~145 MB**; CPU somada ~10–12% de um núcleo. ❓ Medir de novo no Linux e na versão final.

## 10. Fases

1. ✅ **Tocando** — acervo, busca, fila, Tocando agora, cartucheira, pré-escuta.
2. ✅ **Agendamentos** — listas, comerciais, ducking que detecta fala, hora certa, validade, rodízio. Mais as abas Pastas, Cartucheira e Configurações (senha, tocar ao abrir).
3. **Saídas** — placa da pré-escuta, transmissão Icecast, gravação.
4. **Acabamento** — instalador `.deb`/AppImage (gerado pelo GitHub), teste num Linux de verdade, medir leveza.

## 11. Pendências e decisões em aberto

- ❓ Hora certa: espera a música ou ducking? Opção "toda hora"?
- ❓ Comercial que dispara fora do ar: descartar (atual) ou guardar?
- ❓ Sugestões 💡 da fila (repetição, crossfade) e do histórico.
- ❓ Configurações de áudio, transmissão e gravação (Fase 3).
- ❓ Limite do inotify no Linux com muitas subpastas.
- 🧹 Na sua máquina há **5 agendamentos de teste** (11:04–11:07) apontando para pastas temporárias — apagar.

## 12. O que testar (🧪)

Nada disso foi testado de ouvido ou clicando por completo:

**No ar**
- [ ] Ducking manual abaixa e sobe a música; cartucheira não abaixa junto.
- [ ] Repetir: a música toca de novo; desligar volta para a fila.
- [ ] Passar pula música, mas fica bloqueado durante comercial/hora certa.
- [ ] Pré-escuta: 🎧 vira ■, barrinha anda, clicar na linha pula, ar continua normal.
- [ ] Barra vermelha nos 10 s finais.
- [ ] Som sem acelerar/desacelerar e sem travadinha ao abrir outros apps (folga de 1 s no Mac).
- [ ] Fila: reordenar arrastando, lixeira, mínimo de 3, lista ativa aparecendo.

**Agendamentos**
- [ ] Comercial normal entra no fim da música ("depois desta") e não aparece na fila.
- [ ] Várias pastas, ordem anexada, Todos / Só um (rodízio andando e salvo depois de fechar o programa).
- [ ] Validade: arquivo vencido não toca.
- [ ] Ducking curto: música abaixa com voz, sobe no silêncio, volta ao normal.
- [ ] Ducking longo: música repete por baixo, fade de 3 s e próxima.
- [ ] Hora certa com os arquivos reais da rádio.
- [ ] Lista de músicas trocando de pasta no horário.
- [ ] Próximo agendamento correto no Tocando agora.

**Abas**
- [ ] Pastas: adicionar várias, tirar, ordem alfabética no No ar.
- [ ] Cartucheira: escolher/esvaziar os 8 botões.
- [ ] Configurações: entrar no ar sozinho (fechar e abrir o programa), Atualizar, criar/trocar/remover senha, bloqueios por aba, cadeado para bloquear de novo.
