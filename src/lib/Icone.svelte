<script module lang="ts">
  // Ícones Tabler (outline). Só os importados aqui entram no programa.
  import antenna from "@tabler/icons/outline/antenna.svg?raw";
  import play from "@tabler/icons/outline/player-play.svg?raw";
  import pause from "@tabler/icons/outline/player-pause.svg?raw";
  import stop from "@tabler/icons/outline/player-stop.svg?raw";
  import skip from "@tabler/icons/outline/player-skip-forward.svg?raw";
  import music from "@tabler/icons/outline/music.svg?raw";
  import list from "@tabler/icons/outline/list.svg?raw";
  import microphone from "@tabler/icons/outline/microphone.svg?raw";
  import grid from "@tabler/icons/outline/layout-grid.svg?raw";
  import folder from "@tabler/icons/outline/folder.svg?raw";
  import folderOpen from "@tabler/icons/outline/folder-open.svg?raw";
  import search from "@tabler/icons/outline/search.svg?raw";
  import plus from "@tabler/icons/outline/plus.svg?raw";
  import menos from "@tabler/icons/outline/minus.svg?raw";
  import x from "@tabler/icons/outline/x.svg?raw";
  import refresh from "@tabler/icons/outline/refresh.svg?raw";
  import calendar from "@tabler/icons/outline/calendar.svg?raw";
  import settings from "@tabler/icons/outline/settings.svg?raw";
  import grip from "@tabler/icons/outline/grip-vertical.svg?raw";
  import alert from "@tabler/icons/outline/alert-triangle.svg?raw";
  import voltar from "@tabler/icons/outline/chevron-left.svg?raw";
  import repetir from "@tabler/icons/outline/repeat.svg?raw";
  import lixeira from "@tabler/icons/outline/trash.svg?raw";
  import fone from "@tabler/icons/outline/headphones.svg?raw";
  import vinheta from "@tabler/icons/outline/sparkles.svg?raw";
  import abrir from "@tabler/icons/outline/chevron-down.svg?raw";
  import megafone from "@tabler/icons/outline/speakerphone.svg?raw";
  import relogio from "@tabler/icons/outline/clock.svg?raw";
  import cadeado from "@tabler/icons/outline/lock.svg?raw";
  import aberto from "@tabler/icons/outline/lock-open.svg?raw";
  import ducking from "@tabler/icons/outline/microphone-2.svg?raw";
  import texto from "@tabler/icons/outline/abc.svg?raw";
  import aovivo from "@tabler/icons/outline/broadcast.svg?raw";
  import gravar from "@tabler/icons/outline/player-record.svg?raw";
  import transmitir from "@tabler/icons/outline/world.svg?raw";
  import telaCheia from "@tabler/icons/outline/maximize.svg?raw";
  import sairTelaCheia from "@tabler/icons/outline/minimize.svg?raw";

  const icones = {
    antenna, play, pause, stop, skip, music, list, microphone, grid, folder, folderOpen,
    search, plus, menos, x, refresh, calendar, settings, grip, alert, voltar, repetir, lixeira, fone, ducking, vinheta, abrir, megafone, relogio, cadeado, aberto, aovivo, gravar, transmitir,
    telaCheia, sairTelaCheia, texto,
    cantor: ducking,
  };
  export type NomeIcone = keyof typeof icones;
</script>

<script lang="ts">
  let { nome, tamanho = 16 }: { nome: NomeIcone; tamanho?: number } = $props();
</script>

<span class="icone" style:width="{tamanho}px" style:height="{tamanho}px" aria-hidden="true">
  {@html icones[nome]}
</span>

<style>
  .icone {
    display: inline-flex;
    flex: none;
  }
  .icone :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
