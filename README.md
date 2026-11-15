# Radio On Air

Automação de rádio (rádio DJ) **leve e open source** para Linux, feita para **rádios comunitárias**.

> Em desenvolvimento — Fases 1 (tocar, fila, busca, cartucheira, pré-escuta) e 2 (agendamentos, pastas, senha) prontas; em teste. Veja o [ESCOPO.md](ESCOPO.md), inclusive a lista do que testar.

## Como funciona

- **Tela:** [Tauri](https://tauri.app) + [Svelte](https://svelte.dev) (programa de desktop, janela própria).
- **Áudio:** [Liquidsoap](https://www.liquidsoap.info) rodando escondido; o programa entrega uma faixa por vez.
- **Sem banco de dados:** tudo em pastas e arquivos de texto.
  - Músicas no padrão `Artista - Música.mp3`, em qualquer subpasta.
  - Índice do acervo em `indice.txt`, atualizado sozinho quando entra/sai música.
  - Configurações em `config.json`, agendamentos em `agendamentos.json`, rodízio em `rodizio.json`.

Os arquivos de dados ficam em `~/.local/share/org.radioonair.app/` (Linux)
ou `~/Library/Application Support/org.radioonair.app/` (macOS).

## Desenvolver

Requisitos: Node 22+, Rust (stable) e Liquidsoap 2.4+.

```bash
npm install
npm run tauri dev
```

Para o PATH do Rust instalado pelo rustup: `export PATH="$HOME/.cargo/bin:$PATH"`.

No Linux (Ubuntu/Debian), o Liquidsoap vem pelo `apt install liquidsoap`.
No macOS, dá para compilar isolado com o [opam](https://opam.ocaml.org) em `~/.opam`
(o programa procura `~/.opam/radio/bin/liquidsoap`). Também é possível apontar
outro executável com a variável `RADIO_LIQUIDSOAP`.

Testes do núcleo:

```bash
cd src-tauri && cargo test
```

## Licença

[GPL-3.0-or-later](LICENSE).
