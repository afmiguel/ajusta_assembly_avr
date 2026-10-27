//! Converte a listagem do `avr-objdump -D` em assembly que pode ser
//! remontado no Atmel Studio (endereços em words, `.word` → `.dw`).
//!
//! ```
//! use ajusta_asm_core::{converter, Opcoes};
//!
//! let listagem = "\n       0:\t0c 94 3a 00 \tjmp\t0x74\t;  0x74\n";
//! let resultado = converter(listagem, &Opcoes::default());
//! assert_eq!(resultado.texto, "\tjmp     0x3a\t");
//! ```

mod emissor;
mod parser;

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

pub fn converter(listagem: &str, opcoes: &Opcoes) -> Resultado {
    let mut avisos = Vec::new();
    let mut estatisticas = Estatisticas::default();
    // Guarda se a linha é preenchimento para poder compactar o final.
    let mut saida: Vec<(String, bool)> = Vec::new();

    for (indice, texto) in listagem.lines().enumerate() {
        let linha = indice + 1;
        estatisticas.linhas_lidas = linha;
        match ler_linha(linha, texto) {
            Linha::Ignorada => {}
            Linha::Invalida(motivo) => avisos.push(Aviso {
                linha,
                mensagem: motivo.to_owned(),
            }),
            Linha::Instrucao(instr) => match emissor::emitir(&instr, opcoes.comentar_endereco) {
                Ok(texto) => {
                    estatisticas.instrucoes += 1;
                    saida.push((texto, emissor::eh_preenchimento(&instr)));
                }
                Err(mensagem) => avisos.push(Aviso { linha, mensagem }),
            },
        }
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

    const FFFF: &str = "   7ffe:\tff ff       \t.word\t0xffff\t; ????";

    #[test]
    fn entrada_vazia() {
        let r = converter("", &Opcoes::default());
        assert_eq!(r.texto, "");
        assert!(r.avisos.is_empty());
    }

    #[test]
    fn so_preenchimento_vira_uma_linha() {
        let entrada = [FFFF; 3].join("\n");
        let r = converter(&entrada, &Opcoes::default());
        assert_eq!(r.texto, "\t.dw     0xffff");
        assert_eq!(r.estatisticas.preenchimento_removido, 2);
    }

    #[test]
    fn sem_compactar_mantem_tudo() {
        let entrada = [FFFF; 3].join("\n");
        let opcoes = Opcoes {
            compactar_preenchimento: false,
            ..Opcoes::default()
        };
        assert_eq!(converter(&entrada, &opcoes).texto.lines().count(), 3);
    }

    #[test]
    fn avisos_indicam_a_linha() {
        let entrada = "cabeçalho\n  10:\t0c 94\n  14:\t0c 94 57 00 \tjmp\tmain";
        let r = converter(entrada, &Opcoes::default());
        let linhas: Vec<_> = r.avisos.iter().map(|a| a.linha).collect();
        assert_eq!(linhas, [2, 3]);
    }
}
