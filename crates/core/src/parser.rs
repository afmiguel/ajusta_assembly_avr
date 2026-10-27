//! Leitura das linhas geradas pelo `avr-objdump -D`.
//!
//! Formato de uma linha de instrução (campos separados por TAB, marcado como →):
//!
//! ```text
//!       8a:→02 c0       →rjmp→.+4      →;  0x90
//!  endereço   bytes       mnemônico     comentário
//!                              operandos
//! ```

/// Uma instrução extraída da listagem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instrucao<'a> {
    /// Número da linha na entrada (a partir de 1).
    pub linha: usize,
    /// Endereço em bytes, como impresso pelo objdump.
    pub endereco: u32,
    /// Quantidade de bytes da instrução (2 ou 4).
    pub tamanho: u32,
    pub mnemonico: &'a str,
    pub operandos: Vec<&'a str>,
    /// Texto após o `;`, sem espaços nas pontas.
    pub comentario: Option<&'a str>,
}

/// Resultado da leitura de uma linha.
#[derive(Debug, PartialEq, Eq)]
pub enum Linha<'a> {
    Instrucao(Instrucao<'a>),
    /// Cabeçalhos, linhas em branco, rótulos de seção etc.
    Ignorada,
    /// Parece uma linha de instrução, mas não pôde ser interpretada.
    Invalida(&'static str),
}

pub fn ler_linha(linha: usize, texto: &str) -> Linha<'_> {
    let Some((endereco, resto)) = texto.split_once(':') else {
        return Linha::Ignorada;
    };
    let Ok(endereco) = u32::from_str_radix(endereco.trim(), 16) else {
        return Linha::Ignorada;
    };
    // Cabeçalhos como "prog.hex:     file format ihex" não têm TAB após o ':'.
    let Some(resto) = resto.strip_prefix('\t') else {
        return Linha::Ignorada;
    };

    let mut campos = resto.split('\t');
    let tamanho = campos.next().map_or(0, |b| b.split_whitespace().count()) as u32;
    let mnemonico = campos.next().map(str::trim).unwrap_or_default();
    if mnemonico.is_empty() {
        return Linha::Invalida("instrução sem mnemônico");
    }
    if !mnemonico
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return Linha::Invalida("mnemônico com caracteres inesperados");
    }

    let mut operandos = Vec::new();
    let mut comentario = None;
    for campo in campos {
        if let Some(c) = campo.trim_start().strip_prefix(';') {
            comentario = Some(c.trim());
            break;
        }
        operandos.extend(campo.split(',').map(str::trim).filter(|o| !o.is_empty()));
    }

    Linha::Instrucao(Instrucao {
        linha,
        endereco,
        tamanho,
        mnemonico,
        operandos,
        comentario,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instr(texto: &str) -> Instrucao<'_> {
        match ler_linha(1, texto) {
            Linha::Instrucao(i) => i,
            outra => panic!("esperava instrução, veio {outra:?}"),
        }
    }

    #[test]
    fn instrucao_completa() {
        let i = instr("      8a:\t02 c0       \trjmp\t.+4      \t;  0x90");
        assert_eq!(i.endereco, 0x8a);
        assert_eq!(i.tamanho, 2);
        assert_eq!(i.mnemonico, "rjmp");
        assert_eq!(i.operandos, [".+4"]);
        assert_eq!(i.comentario, Some("0x90"));
    }

    #[test]
    fn varios_operandos() {
        let i = instr("      78:\tcf ef       \tldi\tr28, 0xFF\t; 255");
        assert_eq!(i.operandos, ["r28", "0xFF"]);
    }

    #[test]
    fn sem_operandos() {
        let i = instr("     1a4:\t08 95       \tret");
        assert_eq!(i.mnemonico, "ret");
        assert!(i.operandos.is_empty());
        assert_eq!(i.comentario, None);
    }

    #[test]
    fn crlf_e_espacos_extras() {
        let i = instr("  10:\t0c 94 57 00 \tjmp\t0xae\t;  0xae\r");
        assert_eq!(i.operandos, ["0xae"]);
        assert_eq!(i.comentario, Some("0xae"));
    }

    #[test]
    fn cabecalhos_sao_ignorados() {
        for texto in [
            "",
            "prog_c2.hex:     file format ihex",
            "Disassembly of section .sec1:",
            "00000000 <.sec1>:",
            r"C:\Users\fulano>avr-objdump -D prog.hex",
        ] {
            assert_eq!(ler_linha(1, texto), Linha::Ignorada, "{texto:?}");
        }
    }

    #[test]
    fn linha_truncada_e_invalida() {
        assert!(matches!(ler_linha(1, "  10:\t0c 94"), Linha::Invalida(_)));
    }
}
