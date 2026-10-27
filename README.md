# Ajusta Assembly AVR

Desmonta o arquivo `.hex` de um AVR e gera assembly que pode ser remontado no
Atmel Studio (Microchip Studio) — sem precisar do `avr-objdump`. Também aceita
a listagem já gerada pelo `avr-objdump -D`.

**Página:** https://afmiguel.github.io/ajusta_assembly_avr/

Versão em Rust/WebAssembly do antigo `ajusta_assembly.html` (PyScript).

## Desmontagem do .hex

O desmontador lê o Intel HEX e gera a mesma listagem do
`avr-objdump -D -z -m avr5`, que segue para a conversão abaixo. A
compatibilidade é verificada nos testes (`crates/core/tests/desmontador.rs`):

- a listagem de um programa real (16 mil linhas) é idêntica à gerada pelo
  `avr-objdump` do Atmel Studio 7, e o `.asm` final é idêntico ao do script Python;
- todas as 65.536 words possíveis são comparadas com o `avr-objdump`
  instalado na máquina (o teste é pulado se ele não existir).

Duas diferenças intencionais em relação ao `avr-objdump -D` sem opções, que
deslocariam os endereços do programa remontado:

- sequências de zeros são desmontadas como `nop` (o objdump as troca por `...`;
  equivale ao `-z`);
- lacunas entre trechos do `.hex` geram `.org` no assembly.

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
- Saltos de endereço viram `.org`; um `...` do objdump (zeros omitidos) vira
  a quantidade certa de `nop`.
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
cargo run -p ajusta-asm -- prog.hex -o prog.asm
```

A entrada pode ser o `.hex` ou uma listagem do avr-objdump (o tipo é
detectado pelo conteúdo). Opções: `--listagem` (só desmonta, como o
`avr-objdump -D -z -m avr5`), `--manter-preenchimento`, `--comentar-endereco`.
Sem arquivo, lê da entrada padrão e escreve na saída padrão.

No Windows, substitui o `avr-objdump.exe -D -m avr5 %1` mais o ajuste:

```bat
ajusta-asm.exe %1 -o "%~dpn1.asm"
```

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
