//! Geração das linhas no formato aceito pelo assembler do Atmel Studio.
//!
//! O objdump trabalha com endereços em bytes; o Atmel Studio, em words
//! (16 bits). Por isso os destinos de saltos são divididos por 2.

use crate::parser::Instrucao;

/// Coluna em que os operandos começam (mnemônico + espaços).
const LARGURA_MNEMONICO: usize = 8;

/// Saltos cujo operando já é um endereço absoluto em bytes.
const SALTOS_ABSOLUTOS: [&str; 2] = ["jmp", "call"];

pub fn emitir(instr: &Instrucao<'_>, comentar_endereco: bool) -> Result<String, String> {
    let mnemonico = match instr.mnemonico {
        ".word" => ".dw",
        outro => outro,
    };

    let operandos = if SALTOS_ABSOLUTOS.contains(&mnemonico) {
        let destino = instr
            .operandos
            .first()
            .ok_or_else(|| format!("'{mnemonico}' sem endereço de destino"))?;
        vec![em_words(parse_hex(destino)?)]
    } else {
        instr
            .operandos
            .iter()
            .map(|op| match deslocamento_relativo(op) {
                Some(desl) => destino_relativo(instr, desl).map(em_words),
                None => Ok((*op).to_owned()),
            })
            .collect::<Result<_, _>>()?
    };

    let mut linha = format!("\t{mnemonico:<LARGURA_MNEMONICO$}{}\t", operandos.join(","));
    if comentar_endereco {
        linha.push_str(&format!("; {:#06x}", instr.endereco / 2));
    }
    Ok(linha)
}

/// Linha que representa uma word de flash apagada.
pub fn linha_preenchimento() -> String {
    format!("\t{:<LARGURA_MNEMONICO$}0xffff", ".dw")
}

pub fn eh_preenchimento(instr: &Instrucao<'_>) -> bool {
    instr.mnemonico == ".word"
        && matches!(instr.operandos.as_slice(), [op] if op.eq_ignore_ascii_case("0xffff"))
}

fn em_words(endereco_bytes: u32) -> String {
    format!("{:#x}", endereco_bytes / 2)
}

fn parse_hex(texto: &str) -> Result<u32, String> {
    let digitos = texto
        .strip_prefix("0x")
        .or_else(|| texto.strip_prefix("0X"))
        .unwrap_or(texto);
    u32::from_str_radix(digitos, 16)
        .map_err(|_| format!("endereço hexadecimal inválido: '{texto}'"))
}

/// Reconhece operandos relativos como `.+4` ou `.-10`.
fn deslocamento_relativo(op: &str) -> Option<i64> {
    let resto = op.strip_prefix('.')?;
    if !resto.starts_with(['+', '-']) {
        return None;
    }
    resto.parse().ok()
}

/// O objdump já anota o destino no comentário (`;  0x90`); se não houver
/// comentário, calcula a partir do PC seguinte.
fn destino_relativo(instr: &Instrucao<'_>, deslocamento: i64) -> Result<u32, String> {
    if let Some(destino) = instr
        .comentario
        .and_then(|c| c.split_whitespace().next())
        .and_then(|c| parse_hex(c).ok())
    {
        return Ok(destino);
    }
    u32::try_from(i64::from(instr.endereco) + 2 + deslocamento)
        .map_err(|_| format!("destino relativo fora da memória ({deslocamento:+})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instr<'a>(
        endereco: u32,
        mnemonico: &'a str,
        operandos: &[&'a str],
        comentario: Option<&'a str>,
    ) -> Instrucao<'a> {
        Instrucao {
            linha: 1,
            endereco,
            mnemonico,
            operandos: operandos.to_vec(),
            comentario,
        }
    }

    #[test]
    fn instrucao_comum_mantem_operandos() {
        let i = instr(0x78, "ldi", &["r28", "0xFF"], Some("255"));
        assert_eq!(emitir(&i, false).unwrap(), "\tldi     r28,0xFF\t");
    }

    #[test]
    fn word_vira_dw() {
        let i = instr(0x72, ".word", &["0x005b"], Some("????"));
        assert_eq!(emitir(&i, false).unwrap(), "\t.dw     0x005b\t");
    }

    #[test]
    fn salto_absoluto_em_words() {
        let i = instr(0, "jmp", &["0x74"], Some("0x74"));
        assert_eq!(emitir(&i, false).unwrap(), "\tjmp     0x3a\t");
    }

    #[test]
    fn salto_relativo_usa_comentario() {
        let i = instr(0x8a, "rjmp", &[".+4"], Some("0x90"));
        assert_eq!(emitir(&i, false).unwrap(), "\trjmp    0x48\t");
    }

    #[test]
    fn qualquer_desvio_relativo_e_reconhecido() {
        // brlt não constava da lista fixa do script original.
        let i = instr(0x94, "brlt", &[".-10"], Some("0x8c"));
        assert_eq!(emitir(&i, false).unwrap(), "\tbrlt    0x46\t");
    }

    #[test]
    fn salto_relativo_sem_comentario_e_calculado() {
        let i = instr(0x94, "brne", &[".-10"], None);
        assert_eq!(emitir(&i, false).unwrap(), "\tbrne    0x46\t");
    }

    #[test]
    fn comentario_com_simbolo() {
        let i = instr(0x8a, "rcall", &[".+4"], Some("0x90 <main+0x4>"));
        assert_eq!(emitir(&i, false).unwrap(), "\trcall   0x48\t");
    }

    #[test]
    fn mnemonico_longo_nao_quebra_alinhamento() {
        let i = instr(0, "verylongop", &["r1"], None);
        assert_eq!(emitir(&i, false).unwrap(), "\tverylongopr1\t");
    }

    #[test]
    fn comentario_de_endereco() {
        let i = instr(0x8a, "nop", &[], None);
        assert_eq!(emitir(&i, true).unwrap(), "\tnop     \t; 0x0045");
    }

    #[test]
    fn jmp_invalido_gera_erro() {
        let i = instr(0, "jmp", &["main"], None);
        assert!(emitir(&i, false).is_err());
    }
}
