<script lang="ts">
  import { api } from "./api";
  import Icone from "./Icone.svelte";

  let { area, aoLiberar }: { area: string; aoLiberar: (senha: string) => void } = $props();

  let senha = $state("");
  let campo: HTMLInputElement | undefined = $state();
  // Já abre com o cursor no campo de senha.
  $effect(() => campo?.focus());
  let erro = $state("");

  async function entrar(ev: SubmitEvent) {
    ev.preventDefault();
    if (!senha) return (erro = "Digite a senha.");
    if (await api.senhaConferir(senha)) aoLiberar(senha);
    else erro = "Senha incorreta.";
  }
</script>

<div class="fundo">
  <form class="painel caixa" onsubmit={entrar}>
    <span class="icone"><Icone nome="cadeado" tamanho={28} /></span>
    <div class="titulo">{area} está bloqueado</div>
    <input
      type="password"
      placeholder="Senha"
      bind:value={senha}
      oninput={() => (erro = "")}
      bind:this={campo}
    />
    {#if erro}<div class="erro">{erro}</div>{/if}
    <button class="botao" type="submit"><Icone nome="aberto" tamanho={16} /> Desbloquear</button>
  </form>
</div>

<style>
  .fundo {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .caixa {
    width: 320px;
    padding: 22px;
    align-items: center;
    gap: 12px;
  }
  .icone {
    color: var(--tag-comercial);
    display: flex;
  }
  .titulo {
    font-size: 16px;
    font-weight: 500;
  }
  input {
    width: 100%;
    height: 40px;
    border-color: var(--borda-forte);
    border-radius: 8px;
  }
  .erro {
    color: #ff9b9b;
    font-size: 13px;
  }
  .botao {
    width: 100%;
    height: 40px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    background: #238636;
    border-color: #2ea043;
    color: #fff;
  }
</style>
