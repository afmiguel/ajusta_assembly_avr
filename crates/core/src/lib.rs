//! Converte um arquivo Intel HEX, ou a listagem do `avr-objdump -D`, em
//! assembly que pode ser remontado no Atmel Studio (endereços em words,
//! `.word` → `.dw`).
//!
//! ```
//! use ajusta_asm_core::{converter, Opcoes};
//!
//! let listagem = "\n       0:\t0c 94 3a 00 \tjmp\t0x74\t;  0x74\n";
//! assert_eq!(converter(listagem, &Opcoes::default()).texto, "\tjmp     0x3a\t");
//!
//! let hex = ":040000000C943A0022\n:00000001FF\n";
//! assert_eq!(converter(hex, &Opcoes::default()).texto, "\tjmp     0x3a\t");
//! ```

pub mod avr;
mod desmontador;
mod emissor;
pub mod ihex;
mod parser;

pub use ihex::{ErroHex, eh_intel_hex};
pub use parser::{Instrucao, Linha, ler_linha};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opcoes {
    /// Troca a sequência final de `.dw 0xffff` (flash apagada) por uma única linha.
    pub compactar_preenchimento: bool,
    /// Acrescenta o endereço (em words) de cada instrução como comentário.
    pub comentar_endereco: bool,
}

impl Default for Opcoes {
    fn default() -> Self {
        Self {
            compactar_preenchimento: true,
            comentar_endereco: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    /// Número da linha na entrada (a partir de 1).
    pub linha: usize,
    pub mensagem: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Estatisticas {
    pub linhas_lidas: usize,
    pub instrucoes: usize,
    pub preenchimento_removido: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resultado {
    pub texto: String,
    pub avisos: Vec<Aviso>,
    pub estatisticas: Estatisticas,
}

/// Gera a listagem que o `avr-objdump -D -z -m avr5` produziria para o
/// arquivo Intel HEX.
pub fn desmontar(hex: &str, nome_arquivo: &str) -> Result<String, ErroHex> {
    let secoes = ihex::ler_intel_hex(hex)?;
    Ok(desmontador::listagem(&secoes, nome_arquivo))
}

/// Converte um Intel HEX ou uma listagem do avr-objdump (detectado pelo conteúdo).
pub fn converter(entrada: &str, opcoes: &Opcoes) -> Resultado {
    if !eh_intel_hex(entrada) {
        return converter_listagem(entrada, opcoes);
    }
    match desmontar(entrada, "entrada.hex") {
        Ok(listagem) => converter_listagem(&listagem, opcoes),
        Err(erro) => Resultado {
            avisos: vec![Aviso {
                linha: erro.linha,
                mensagem: format!("Intel HEX inválido: {}", erro.mensagem),
            }],
            ..Resultado::default()
        },
    }
}

pub fn converter_listagem(listagem: &str, opcoes: &Opcoes) -> Resultado {
    let mut avisos = Vec::new();
    let mut estatisticas = Estatisticas::default();
    // Guarda se a linha é preenchimento para poder compactar o final.
    let mut saida: Vec<(String, bool)> = Vec::new();
    // Endereço em que a próxima instrução deveria estar.
    let mut proximo: Option<u32> = None;
    // O objdump (sem -z) troca sequências de zeros por "...".
    let mut zeros_omitidos = false;

    for (indice, texto) in listagem.lines().enumerate() {
        let linha = indice + 1;
        estatisticas.linhas_lidas = linha;
        if texto.trim() == "..." {
            zeros_omitidos = true;
            continue;
        }
        let instr = match ler_linha(linha, texto) {
            Linha::Ignorada => continue,
            Linha::Invalida(motivo) => {
                avisos.push(Aviso {
                    linha,
                    mensagem: motivo.to_owned(),
                });
                continue;
            }
            Linha::Instrucao(instr) => instr,
        };

        match proximo {
            Some(esperado) if zeros_omitidos && instr.endereco > esperado => {
                for endereco in (esperado..instr.endereco).step_by(2) {
                    let nop = Instrucao {
                        linha,
                        endereco,
                        tamanho: 2,
                        mnemonico: "nop",
                        operandos: Vec::new(),
                        comentario: None,
                    };
                    if let Ok(texto) = emissor::emitir(&nop, opcoes.comentar_endereco) {
                        saida.push((texto, false));
                    }
                }
            }
            Some(esperado) if esperado == instr.endereco => {}
            None if instr.endereco == 0 => {}
            _ => saida.push((emissor::linha_org(instr.endereco), false)),
        }
        zeros_omitidos = false;
        proximo = Some(instr.endereco + instr.tamanho);

        match emissor::emitir(&instr, opcoes.comentar_endereco) {
            Ok(texto) => {
                estatisticas.instrucoes += 1;
                saida.push((texto, emissor::eh_preenchimento(&instr)));
            }
            Err(mensagem) => avisos.push(Aviso { linha, mensagem }),
        }
    }

    if zeros_omitidos {
        avisos.push(Aviso {
            linha: estatisticas.linhas_lidas,
            mensagem: "zeros omitidos (\"...\") no fim da listagem; gere-a com avr-objdump -z"
                .into(),
        });
    }

    if opcoes.compactar_preenchimento {
        let finais = saida.iter().rev().take_while(|(_, p)| *p).count();
        if finais > 0 {
            saida.truncate(saida.len() - finais);
            saida.push((emissor::linha_preenchimento(), true));
            estatisticas.preenchimento_removido = finais - 1;
        }
    }

    let texto = saida
        .into_iter()
        .map(|(linha, _)| linha)
        .collect::<Vec<_>>()
        .join("\n");

    Resultado {
        texto,
        avisos,
        estatisticas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preenchimento() -> String {
        [0x7ffa, 0x7ffc, 0x7ffe]
            .map(|a| format!("{a:8x}:\tff ff       \t.word\t0xffff\t; ????"))
            .join("\n")
    }

    #[test]
    fn entrada_vazia() {
        let r = converter("", &Opcoes::default());
        assert_eq!(r.texto, "");
        assert!(r.avisos.is_empty());
    }

    #[test]
    fn so_preenchimento_vira_uma_linha() {
        let entrada = preenchimento();
        let r = converter(&entrada, &Opcoes::default());
        assert_eq!(r.texto, "\t.org    0x3ffd\t\n\t.dw     0xffff");
        assert_eq!(r.estatisticas.preenchimento_removido, 2);
    }

    #[test]
    fn sem_compactar_mantem_tudo() {
        let entrada = preenchimento();
        let opcoes = Opcoes {
            compactar_preenchimento: false,
            ..Opcoes::default()
        };
        assert_eq!(converter(&entrada, &opcoes).texto.lines().count(), 4);
    }

    #[test]
    fn avisos_indicam_a_linha() {
        let entrada = "cabeçalho\n  10:\t0c 94\n  14:\t0c 94 57 00 \tjmp\tmain";
        let r = converter(entrada, &Opcoes::default());
        let linhas: Vec<_> = r.avisos.iter().map(|a| a.linha).collect();
        assert_eq!(linhas, [2, 3]);
    }

    #[test]
    fn zeros_omitidos_viram_nop() {
        let entrada = "   0:\t08 95       \tret\n\t...\n   a:\t08 95       \tret";
        let r = converter(entrada, &Opcoes::default());
        let esperado = [
            "\tret     \t",
            "\tnop     \t",
            "\tnop     \t",
            "\tnop     \t",
            "\tnop     \t",
            "\tret     \t",
        ];
        assert_eq!(r.texto, esperado.join("\n"));
    }

    #[test]
    fn lacuna_vira_org() {
        let entrada = "  10:\t08 95       \tret\n  20:\t08 95       \tret";
        let r = converter(entrada, &Opcoes::default());
        assert_eq!(
            r.texto,
            "\t.org    0x8\t\n\tret     \t\n\t.org    0x10\t\n\tret     \t"
        );
    }

    #[test]
    fn hex_invalido_gera_aviso() {
        let r = converter(":020000000895F9\n", &Opcoes::default());
        assert_eq!(r.texto, "");
        assert!(r.avisos[0].mensagem.contains("checksum"));
    }
}
