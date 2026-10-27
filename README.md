# Ajusta Assembly AVR

Converte a listagem gerada pelo `avr-objdump -D` em assembly que pode ser
remontado no Atmel Studio (Microchip Studio).

Versão em Rust/WebAssembly do antigo `ajusta_assembly.html` (PyScript).

## O que a conversão faz

| Listagem do objdump                         | Saída                 |
|---------------------------------------------|-----------------------|
| `0: 0c 94 3a 00  jmp 0x74 ; 0x74`           | `jmp     0x3a`        |
| `8a: 02 c0  rjmp .+4 ; 0x90`                | `rjmp    0x48`        |
| `78: cf ef  ldi r28, 0xFF ; 255`            | `ldi     r28,0xFF`    |
| `72: 5b 00  .word 0x005b ; ????`            | `.dw     0x005b`      |

- Endereços de `jmp`/`call` e de **todos** os desvios relativos (`rjmp`,
  `rcall`, `br*`) são convertidos de bytes para words (÷ 2).
- A sequência final de `.dw 0xffff` (flash apagada) vira uma única linha.
- Cabeçalhos são ignorados; linhas de instrução que não puderam ser lidas
  aparecem como avisos, com o número da linha.

### Diferenças em relação ao script Python

- Desvios relativos são reconhecidos pelo operando (`.+n`/`.-n`), e não por
  uma lista fixa de mnemônicos — `brlt`, `brlo`, `brmi` etc. passam a funcionar.
- Entrada vazia ou só com preenchimento não gera erro.
- Se a listagem não termina com `.dw 0xffff`, nenhuma linha é acrescentada.

Para a listagem de referência, a saída é idêntica à do script original
(`crates/core/tests/regressao.rs`).

## Estrutura

```
crates/
  core/   biblioteca de conversão, sem dependências
  cli/    linha de comando (ajusta-asm)
  web/    página em Leptos, compilada com Trunk
```

## Uso

### Página

```bash
cd crates/web
trunk serve --open
```

### Linha de comando

```bash
cargo run -p ajusta-asm -- listagem.lst -o codigo.asm
```

Opções: `--manter-preenchimento`, `--comentar-endereco`. Sem arquivo, lê da
entrada padrão e escreve na saída padrão.

### Testes

```bash
cargo test --workspace --exclude ajusta-asm-web
```

## Publicação no GitHub Pages

O workflow `.github/workflows/pages.yml` roda testes e clippy e publica
`crates/web/dist` a cada push na `main`. No repositório, em
**Settings → Pages → Source**, escolha **GitHub Actions**.

## Embutir no Canvas LMS (ou outro site)

```html
<iframe src="https://afmiguel.github.io/ajusta_assembly_avr/"
        width="100%" height="720" allow="clipboard-write"></iframe>
```

Dentro de um iframe, o navegador só libera a Clipboard API se o site que
embute conceder `allow="clipboard-write"` — e o editor do Canvas pode remover
esse atributo. Por isso o botão **Copiar** usa primeiro a cópia por seleção
(`execCommand("copy")`), que funciona sem essa permissão; se nada funcionar,
o texto fica selecionado para copiar com Ctrl+C (⌘C).
