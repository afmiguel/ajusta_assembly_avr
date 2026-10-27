//! Decodificador do conjunto de instruções AVR.
//!
//! Os textos (mnemônicos, operandos e comentários) seguem exatamente o
//! formato do `avr-objdump -D`, inclusive nos casos em que o objdump prefere
//! a forma canônica (`eor r1, r1` em vez de `clr r1`) ou marca combinações
//! indefinidas (`ld r26, X+ ; undefined`).

/// Uma instrução decodificada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decodificada {
    /// Tamanho em bytes: 2 ou 4.
    pub tamanho: u32,
    pub mnemonico: &'static str,
    /// Operandos já formatados; vazio se não houver.
    pub operandos: String,
    /// Texto que segue o `;`, incluindo os espaços iniciais.
    pub comentario: Option<String>,
}

/// A instrução precisa de uma segunda word que não existe nos dados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordAusente;

pub fn precisa_segunda_word(w: u16) -> bool {
    let lds_sts = w & 0xfc0f == 0x9000;
    let jmp_call = w & 0xfe0c == 0x940c;
    lds_sts || jmp_call
}

/// Decodifica a instrução que começa em `endereco` (em bytes).
pub fn decodificar(endereco: u32, w: u16, w2: Option<u16>) -> Result<Decodificada, WordAusente> {
    if precisa_segunda_word(w) {
        let w2 = w2.ok_or(WordAusente)?;
        return Ok(decodificar_longa(w, w2));
    }
    Ok(decodificar_curta(endereco, w).unwrap_or_else(|| word_invalida(w)))
}

fn decodificar_longa(w: u16, w2: u16) -> Decodificada {
    let d = rd5(w);
    let longa = |mnemonico, operandos, comentario| Decodificada {
        tamanho: 4,
        mnemonico,
        operandos,
        comentario: Some(comentario),
    };
    if w & 0xfe00 == 0x9000 || w & 0xfe00 == 0x9200 {
        let k = u32::from(w2);
        let comentario = format!("  0x{:x}", 0x80_0000 | k);
        return if w & 0x0200 == 0 {
            longa("lds", format!("r{d}, 0x{k:04X}"), comentario)
        } else {
            longa("sts", format!("0x{k:04X}, r{d}"), comentario)
        };
    }
    let alto = (u32::from(w >> 4) & 0x1f) << 1 | u32::from(w & 1);
    let destino = (alto << 16 | u32::from(w2)) * 2;
    let mnemonico = if w & 0x0002 == 0 { "jmp" } else { "call" };
    let operando = if destino == 0 {
        "0".to_owned()
    } else {
        format!("{destino:#x}")
    };
    longa(mnemonico, operando, format!("  0x{destino:x}"))
}

fn decodificar_curta(endereco: u32, w: u16) -> Option<Decodificada> {
    let d = rd5(w);
    let r = w & 0x0f | (w >> 5) & 0x10;
    let d4 = 16 + ((w >> 4) & 0x0f);

    let instr = match w {
        0x0000 => sem_operandos("nop"),
        0x0100..=0x01ff => com(
            "movw",
            format!("r{}, r{}", 2 * ((w >> 4) & 0x0f), 2 * (w & 0x0f)),
        ),
        0x0200..=0x02ff => com("muls", format!("r{d4}, r{}", 16 + (w & 0x0f))),
        0x0300..=0x03ff => {
            let mnemonico = match w & 0x88 {
                0x00 => "mulsu",
                0x08 => "fmul",
                0x80 => "fmuls",
                _ => "fmulsu",
            };
            let regs = format!("r{}, r{}", 16 + ((w >> 4) & 7), 16 + (w & 7));
            com(mnemonico, regs)
        }
        0x0400..=0x2fff | 0x9c00..=0x9fff => {
            let mnemonico = match w >> 10 {
                0x01 => "cpc",
                0x02 => "sbc",
                0x03 => "add",
                0x04 => "cpse",
                0x05 => "cp",
                0x06 => "sub",
                0x07 => "adc",
                0x08 => "and",
                0x09 => "eor",
                0x0a => "or",
                0x0b => "mov",
                _ => "mul",
            };
            com(mnemonico, format!("r{d}, r{r}"))
        }
        0x3000..=0x7fff | 0xe000..=0xefff => {
            let mnemonico = match w >> 12 {
                0x3 => "cpi",
                0x4 => "sbci",
                0x5 => "subi",
                0x6 => "ori",
                0x7 => "andi",
                _ => "ldi",
            };
            let k = (w >> 4) & 0xf0 | w & 0x0f;
            com_comentario(mnemonico, format!("r{d4}, 0x{k:02X}"), format!(" {k}"))
        }
        _ if w & 0xd000 == 0x8000 => ld_st_deslocamento(w, d),
        0x9000..=0x93ff => ld_st_ponteiro(w, d)?,
        0x9400..=0x95ff => grupo_94(w, d)?,
        0x9600..=0x97ff => {
            let mnemonico = if w & 0x0100 == 0 { "adiw" } else { "sbiw" };
            let k = (w >> 2) & 0x30 | w & 0x0f;
            let reg = 24 + 2 * ((w >> 4) & 3);
            com_comentario(mnemonico, format!("r{reg}, 0x{k:02x}"), format!(" {k}"))
        }
        0x9800..=0x9bff => {
            let mnemonico = ["cbi", "sbic", "sbi", "sbis"][usize::from((w >> 8) & 3)];
            let porta = (w >> 3) & 0x1f;
            com_comentario(
                mnemonico,
                format!("0x{porta:02x}, {}", w & 7),
                format!(" {porta}"),
            )
        }
        0xb000..=0xbfff => {
            let porta = (w >> 5) & 0x30 | w & 0x0f;
            let (mnemonico, operandos) = if w & 0x0800 == 0 {
                ("in", format!("r{d}, 0x{porta:02x}"))
            } else {
                ("out", format!("0x{porta:02x}, r{d}"))
            };
            com_comentario(mnemonico, operandos, format!(" {porta}"))
        }
        0xc000..=0xdfff => {
            let mnemonico = if w & 0x1000 == 0 { "rjmp" } else { "rcall" };
            // Desloca para estender o sinal dos 12 bits.
            let k = i32::from(((w << 4) as i16) >> 4) * 2;
            relativo(endereco, mnemonico, k)
        }
        0xf000..=0xf7ff => {
            const SE_SETADO: [&str; 8] = [
                "brcs", "breq", "brmi", "brvs", "brlt", "brhs", "brts", "brie",
            ];
            const SE_LIMPO: [&str; 8] = [
                "brcc", "brne", "brpl", "brvc", "brge", "brhc", "brtc", "brid",
            ];
            let bit = usize::from(w & 7);
            let mnemonico = if w & 0x0400 == 0 {
                SE_SETADO[bit]
            } else {
                SE_LIMPO[bit]
            };
            let k = i32::from(((w << 6) as i16) >> 9) * 2;
            relativo(endereco, mnemonico, k)
        }
        0xf800..=0xffff if w & 0x0008 == 0 => {
            let mnemonico = ["bld", "bst", "sbrc", "sbrs"][usize::from((w >> 9) & 3)];
            com(mnemonico, format!("r{d}, {}", w & 7))
        }
        _ => return None,
    };
    Some(instr)
}

/// `ldd`/`std` com deslocamento; sem deslocamento o objdump usa `ld`/`st`.
fn ld_st_deslocamento(w: u16, d: u16) -> Decodificada {
    let q = w & 7 | (w >> 7) & 0x18 | (w >> 8) & 0x20;
    let ponteiro = if w & 0x0008 == 0 { 'Z' } else { 'Y' };
    let carrega = w & 0x0200 == 0;
    match (q, carrega) {
        (0, true) => com("ld", format!("r{d}, {ponteiro}")),
        (0, false) => com("st", format!("{ponteiro}, r{d}")),
        (_, true) => com_comentario(
            "ldd",
            format!("r{d}, {ponteiro}+{q}"),
            format!(" 0x{q:02x}"),
        ),
        (_, false) => com_comentario(
            "std",
            format!("{ponteiro}+{q}, r{d}"),
            format!(" 0x{q:02x}"),
        ),
    }
}

/// `ld`/`st` com X, Y ou Z (com pré-decremento ou pós-incremento),
/// `lpm`/`elpm`, `push`/`pop` e as instruções atômicas do XMEGA.
fn ld_st_ponteiro(w: u16, d: u16) -> Option<Decodificada> {
    let carrega = w & 0x0200 == 0;
    let modo = w & 0x0f;
    let enderecamento = match modo {
        0x1 => "Z+",
        0x2 => "-Z",
        0x9 => "Y+",
        0xa => "-Y",
        0xc => "X",
        0xd => "X+",
        0xe => "-X",
        _ => "",
    };
    if !enderecamento.is_empty() {
        // Usar o próprio ponteiro com incremento/decremento é indefinido.
        let base = match enderecamento.trim_matches(['+', '-']) {
            "X" => 26,
            "Y" => 28,
            _ => 30,
        };
        let indefinido = modo != 0xc && (d == base || d == base + 1);
        let (mnemonico, operandos) = if carrega {
            ("ld", format!("r{d}, {enderecamento}"))
        } else {
            ("st", format!("{enderecamento}, r{d}"))
        };
        return Some(if indefinido {
            com_comentario(mnemonico, operandos, " undefined".into())
        } else {
            com(mnemonico, operandos)
        });
    }
    let instr = match (carrega, modo) {
        (true, 0x4..=0x7) => {
            let mnemonico = if modo < 0x6 { "lpm" } else { "elpm" };
            let pos_incremento = modo & 1 == 1;
            let operandos = format!("r{d}, Z{}", if pos_incremento { "+" } else { "" });
            if pos_incremento && d >= 30 {
                com_comentario(mnemonico, operandos, " undefined".into())
            } else {
                com(mnemonico, operandos)
            }
        }
        (false, 0x4..=0x7) => {
            let mnemonico = ["xch", "las", "lac", "lat"][usize::from(modo - 4)];
            com(mnemonico, format!("Z, r{d}"))
        }
        (true, 0xf) => com("pop", format!("r{d}")),
        (false, 0xf) => com("push", format!("r{d}")),
        _ => return None,
    };
    Some(instr)
}

/// Faixa 0x9400–0x95ff: operações de um registrador, flags e controle.
fn grupo_94(w: u16, d: u16) -> Option<Decodificada> {
    const FLAGS: [&str; 16] = [
        "sec", "sez", "sen", "sev", "ses", "seh", "set", "sei", //
        "clc", "clz", "cln", "clv", "cls", "clh", "clt", "cli",
    ];
    let instr = match w & 0x0f {
        0x0 => com("com", format!("r{d}")),
        0x1 => com("neg", format!("r{d}")),
        0x2 => com("swap", format!("r{d}")),
        0x3 => com("inc", format!("r{d}")),
        0x5 => com("asr", format!("r{d}")),
        0x6 => com("lsr", format!("r{d}")),
        0x7 => com("ror", format!("r{d}")),
        0xa => com("dec", format!("r{d}")),
        0x8 if w & 0x0100 == 0 => sem_operandos(FLAGS[usize::from((w >> 4) & 0x0f)]),
        0x8 => match w {
            0x9508 => sem_operandos("ret"),
            0x9518 => sem_operandos("reti"),
            0x9588 => sem_operandos("sleep"),
            0x9598 => sem_operandos("break"),
            0x95a8 => sem_operandos("wdr"),
            0x95c8 => sem_operandos("lpm"),
            0x95d8 => sem_operandos("elpm"),
            0x95e8 => sem_operandos("spm"),
            0x95f8 => com("spm", "Z+".into()),
            _ => return None,
        },
        0x9 => match w {
            0x9409 => sem_operandos("ijmp"),
            0x9419 => sem_operandos("eijmp"),
            0x9509 => sem_operandos("icall"),
            0x9519 => sem_operandos("eicall"),
            _ => return None,
        },
        0xb if w & 0x0100 == 0 => com("des", format!("{}", (w >> 4) & 0x0f)),
        _ => return None,
    };
    Some(instr)
}

fn relativo(endereco: u32, mnemonico: &'static str, deslocamento: i32) -> Decodificada {
    let destino = endereco.wrapping_add(2).wrapping_add_signed(deslocamento);
    com_comentario(
        mnemonico,
        format!(".{deslocamento:<+8}"),
        format!("  0x{destino:x}"),
    )
}

fn word_invalida(w: u16) -> Decodificada {
    com_comentario(".word", format!("0x{w:04x}"), " ????".into())
}

/// Registrador de 5 bits nos bits 8–4.
fn rd5(w: u16) -> u16 {
    (w >> 4) & 0x1f
}

fn sem_operandos(mnemonico: &'static str) -> Decodificada {
    com(mnemonico, String::new())
}

fn com(mnemonico: &'static str, operandos: String) -> Decodificada {
    Decodificada {
        tamanho: 2,
        mnemonico,
        operandos,
        comentario: None,
    }
}

fn com_comentario(mnemonico: &'static str, operandos: String, comentario: String) -> Decodificada {
    Decodificada {
        comentario: Some(comentario),
        ..com(mnemonico, operandos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texto(endereco: u32, w: u16, w2: Option<u16>) -> String {
        let d = decodificar(endereco, w, w2).unwrap();
        let mut s = d.mnemonico.to_owned();
        if !d.operandos.is_empty() {
            s += &format!("\t{}", d.operandos);
        }
        if let Some(c) = d.comentario {
            s += &format!("\t;{c}");
        }
        s
    }

    #[test]
    fn exemplos_do_objdump() {
        let casos: &[(u32, u16, Option<u16>, &str)] = &[
            (0x00, 0x940c, Some(0x003a), "jmp\t0x74\t;  0x74"),
            (0xae, 0x940c, Some(0x0000), "jmp\t0\t;  0x0"),
            (0x8a, 0xc002, None, "rjmp\t.+4      \t;  0x90"),
            (0x94, 0xf7d9, None, "brne\t.-10     \t;  0x8c"),
            (0x00, 0xcffe, None, "rjmp\t.-4      \t;  0xfffffffe"),
            (0x78, 0xefcf, None, "ldi\tr28, 0xFF\t; 255"),
            (0x76, 0xbe1f, None, "out\t0x3f, r1\t; 63"),
            (0xde, 0x9160, Some(0x0100), "lds\tr22, 0x0100\t;  0x800100"),
            (0x1c0, 0x8393, None, "std\tZ+3, r25\t; 0x03"),
            (0x00, 0x8000, None, "ld\tr0, Z"),
            (0x00, 0x91ad, None, "ld\tr26, X+\t; undefined"),
            (0x8c, 0x9005, None, "lpm\tr0, Z+"),
            (0x238, 0x9618, None, "adiw\tr26, 0x08\t; 8"),
            (0x182, 0xff85, None, "sbrs\tr24, 5"),
            (0xe1e, 0x94f8, None, "cli"),
            (0x72, 0x005b, None, ".word\t0x005b\t; ????"),
            (0x00, 0x95f8, None, "spm\tZ+"),
            (0x00, 0x940b, None, "des\t0"),
        ];
        for &(endereco, w, w2, esperado) in casos {
            assert_eq!(texto(endereco, w, w2), esperado, "word 0x{w:04x}");
        }
    }

    #[test]
    fn instrucao_longa_sem_segunda_word() {
        assert_eq!(decodificar(0, 0x940c, None), Err(WordAusente));
    }
}
