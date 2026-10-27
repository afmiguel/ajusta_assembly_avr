//! Gera, a partir das seções de um Intel HEX, a mesma listagem que o
//! `avr-objdump -D -z -m avr5` produziria.
//!
//! O `-z` (desmontar sequências de zeros) é intencional: sem ele o objdump
//! troca os zeros por `...` e a contagem de endereços se perde.

use std::fmt::Write;

use crate::avr::{WordAusente, decodificar};
use crate::ihex::Secao;

pub fn listagem(secoes: &[Secao], nome_arquivo: &str) -> String {
    let mut saida = format!("\n{nome_arquivo}:     file format ihex\n\n");
    for (indice, secao) in secoes.iter().enumerate() {
        let nome = format!(".sec{}", indice + 1);
        let _ = write!(
            saida,
            "\nDisassembly of section {nome}:\n\n{:08x} <{nome}>:\n",
            secao.inicio
        );
        desmontar_secao(secao, &mut saida);
    }
    saida
}

fn desmontar_secao(secao: &Secao, saida: &mut String) {
    let largura = largura_endereco(secao);
    let dados = &secao.dados;
    let word = |i: usize| Some(u16::from_le_bytes([*dados.get(i)?, *dados.get(i + 1)?]));

    let mut i = 0;
    while i < dados.len() {
        let endereco = secao.inicio + i as u32;
        let _ = write!(saida, "{endereco:>largura$x}:\t");

        let Some(w) = word(i) else {
            let _ = write!(saida, "Address 0x{endereco:x} is out of bounds.\n\n");
            return;
        };
        let instr = match decodificar(endereco, w, word(i + 2)) {
            Ok(instr) => instr,
            Err(WordAusente) => {
                let _ = write!(saida, "Address 0x{:x} is out of bounds.\n\n", endereco + 2);
                return;
            }
        };

        let tamanho = instr.tamanho as usize;
        let bytes: String = dados[i..i + tamanho]
            .iter()
            .map(|b| format!("{b:02x} "))
            .collect();
        let _ = write!(saida, "{bytes:<12}\t{}", instr.mnemonico);
        if !instr.operandos.is_empty() {
            let _ = write!(saida, "\t{}", instr.operandos);
        }
        if let Some(comentario) = &instr.comentario {
            let _ = write!(saida, "\t;{comentario}");
        }
        saida.push('\n');
        i += tamanho;
    }
}

/// O objdump descarta zeros à esquerda do endereço em blocos de 4 dígitos,
/// conforme o maior endereço da seção, sempre mantendo ao menos um zero.
fn largura_endereco(secao: &Secao) -> usize {
    let fim = format!("{:08x}", secao.fim());
    let zeros = fim.bytes().take_while(|&b| b == b'0').count();
    let descartados = if zeros == fim.len() && secao.inicio != 0 {
        0
    } else {
        zeros.saturating_sub(1) & !3
    };
    fim.len() - descartados
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secao(inicio: u32, dados: &[u8]) -> Secao {
        Secao {
            inicio,
            dados: dados.to_vec(),
        }
    }

    #[test]
    fn formato_igual_ao_objdump() {
        let secoes = [
            secao(0, &[0x08, 0x95]),
            secao(0x10, &[0x08, 0x95, 0x0c, 0x94]),
            secao(0x100, &[0x08, 0x95, 0xff]),
        ];
        let esperado = "\nsecs.hex:     file format ihex\n\n\
            \nDisassembly of section .sec1:\n\n00000000 <.sec1>:\n   0:\t08 95       \tret\n\
            \nDisassembly of section .sec2:\n\n00000010 <.sec2>:\n  10:\t08 95       \tret\n  12:\tAddress 0x14 is out of bounds.\n\n\
            \nDisassembly of section .sec3:\n\n00000100 <.sec3>:\n 100:\t08 95       \tret\n 102:\tAddress 0x102 is out of bounds.\n\n";
        assert_eq!(listagem(&secoes, "secs.hex"), esperado);
    }

    #[test]
    fn largura_dos_enderecos() {
        assert_eq!(largura_endereco(&secao(0, &[0; 0x34])), 4);
        assert_eq!(largura_endereco(&secao(0, &[0; 0x8000])), 8);
        assert_eq!(largura_endereco(&secao(0x10000, &[0; 2])), 8);
    }
}
