//! Leitura de arquivos Intel HEX.
//!
//! Como no `avr-objdump`, registros de dados contíguos formam uma seção;
//! um salto de endereço inicia uma nova seção.

use std::fmt;

/// Bloco contíguo de bytes da memória de programa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secao {
    /// Endereço do primeiro byte.
    pub inicio: u32,
    pub dados: Vec<u8>,
}

impl Secao {
    pub fn fim(&self) -> u32 {
        self.inicio + self.dados.len() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroHex {
    /// Número da linha no arquivo (a partir de 1).
    pub linha: usize,
    pub mensagem: String,
}

impl fmt::Display for ErroHex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "linha {}: {}", self.linha, self.mensagem)
    }
}

impl std::error::Error for ErroHex {}

/// Indica se o texto parece um arquivo Intel HEX (e não uma listagem).
pub fn eh_intel_hex(texto: &str) -> bool {
    texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .is_some_and(|l| {
            l.len() >= 11 && l.starts_with(':') && l[1..].bytes().all(|b| b.is_ascii_hexdigit())
        })
}

pub fn ler_intel_hex(texto: &str) -> Result<Vec<Secao>, ErroHex> {
    let mut secoes: Vec<Secao> = Vec::new();
    let mut base = 0u32;

    for (indice, linha) in texto.lines().enumerate() {
        let numero = indice + 1;
        let erro = |mensagem: &str| ErroHex {
            linha: numero,
            mensagem: mensagem.to_owned(),
        };

        let linha = linha.trim();
        if linha.is_empty() {
            continue;
        }
        let registro = linha
            .strip_prefix(':')
            .ok_or_else(|| erro("registro não começa com ':'"))?;
        let bytes = decodificar_hex(registro).ok_or_else(|| erro("caracteres não hexadecimais"))?;
        if bytes.len() < 5 || bytes.len() != usize::from(bytes[0]) + 5 {
            return Err(erro("tamanho do registro não confere"));
        }
        if bytes.iter().fold(0u8, |soma, b| soma.wrapping_add(*b)) != 0 {
            return Err(erro("checksum inválido"));
        }

        let endereco = u32::from(u16::from_be_bytes([bytes[1], bytes[2]]));
        let dados = &bytes[4..bytes.len() - 1];
        match bytes[3] {
            0x00 => {
                let inicio = base + endereco;
                match secoes.last_mut() {
                    Some(secao) if secao.fim() == inicio => secao.dados.extend_from_slice(dados),
                    _ => secoes.push(Secao {
                        inicio,
                        dados: dados.to_vec(),
                    }),
                }
            }
            0x01 => break,
            0x02 if dados.len() == 2 => {
                base = u32::from(u16::from_be_bytes([dados[0], dados[1]])) << 4;
            }
            0x04 if dados.len() == 2 => {
                base = u32::from(u16::from_be_bytes([dados[0], dados[1]])) << 16;
            }
            // Endereços de início de execução não afetam a memória.
            0x03 | 0x05 => {}
            tipo => return Err(erro(&format!("tipo de registro desconhecido: {tipo:02X}"))),
        }
    }

    secoes.retain(|s| !s.dados.is_empty());
    Ok(secoes)
}

fn decodificar_hex(texto: &str) -> Option<Vec<u8>> {
    if !texto.len().is_multiple_of(2) {
        return None;
    }
    (0..texto.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(texto.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registros_contiguos_formam_uma_secao() {
        let hex = ":02000000089561\n:02000200089D57\n:00000001FF\n";
        let secoes = ler_intel_hex(hex).unwrap();
        assert_eq!(
            secoes,
            [Secao {
                inicio: 0,
                dados: vec![0x08, 0x95, 0x08, 0x9d]
            }]
        );
    }

    #[test]
    fn lacuna_cria_nova_secao_e_endereco_estendido() {
        let hex = ":02000000089561\n:020000040001F9\n:02001000089551\n:00000001FF\n";
        let secoes = ler_intel_hex(hex).unwrap();
        assert_eq!(secoes.len(), 2);
        assert_eq!(secoes[1].inicio, 0x10010);
    }

    #[test]
    fn checksum_errado() {
        let erro = ler_intel_hex(":020000000895F9\n").unwrap_err();
        assert_eq!(erro.linha, 1);
        assert!(erro.mensagem.contains("checksum"));
    }

    #[test]
    fn deteccao() {
        assert!(eh_intel_hex("\n:02000000089561\n"));
        assert!(!eh_intel_hex("       0:\t0c 94 3a 00 \tjmp\t0x74"));
        assert!(!eh_intel_hex(""));
    }
}
