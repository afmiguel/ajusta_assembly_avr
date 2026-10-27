use std::time::Duration;

use ajusta_asm_core::{Opcoes, Resultado, converter, desmontar, eh_intel_hex};
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, BlobPropertyBag, File, HtmlAnchorElement, HtmlDocument, HtmlTextAreaElement, Url,
};

const NOME_SAIDA_PADRAO: &str = "codigo_ajustado.asm";

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let entrada = RwSignal::new(String::new());
    let compactar = RwSignal::new(true);
    let comentar = RwSignal::new(false);
    let aviso = RwSignal::new(None::<String>);
    let nome_saida = RwSignal::new(NOME_SAIDA_PADRAO.to_owned());

    let resultado = Memo::new(move |_| {
        let opcoes = Opcoes {
            compactar_preenchimento: compactar.get(),
            comentar_endereco: comentar.get(),
        };
        entrada.with(|texto| converter(texto, &opcoes))
    });

    let notificar = move |mensagem: String| {
        aviso.set(Some(mensagem));
        set_timeout(move || aviso.set(None), Duration::from_millis(2200));
    };

    let carregar_arquivo = move |arquivo: File| {
        spawn_local(async move {
            match JsFuture::from(arquivo.text()).await {
                Ok(texto) => {
                    let texto = texto.as_string().unwrap_or_default();
                    let nome = arquivo.name();
                    nome_saida.set(nome_asm(&nome));
                    // Um .hex é desmontado na hora, para a listagem aparecer na entrada.
                    if eh_intel_hex(&texto) {
                        match desmontar(&texto, &nome) {
                            Ok(listagem) => {
                                entrada.set(listagem);
                                notificar(format!("{nome} desmontado"));
                            }
                            Err(erro) => {
                                entrada.set(texto);
                                notificar(format!("{nome}: Intel HEX inválido ({erro})"));
                            }
                        }
                    } else {
                        entrada.set(texto);
                        notificar(format!("{nome} carregado"));
                    }
                }
                Err(_) => notificar("Não foi possível ler o arquivo".into()),
            }
        });
    };

    let saida = NodeRef::<leptos::html::Textarea>::new();

    let copiar = move |_| {
        let Some(area) = saida.get() else { return };
        if copiar_por_selecao(&area) {
            notificar("Copiado para a área de transferência".into());
            return;
        }
        spawn_local(async move {
            if copiar_por_clipboard_api(&area.value()).await {
                notificar("Copiado para a área de transferência".into());
            } else {
                area.select();
                notificar("Texto selecionado: use Ctrl+C (⌘C) para copiar".into());
            }
        });
    };

    let baixar = move |_| {
        if resultado
            .with(|r| nome_saida.with(|nome| baixar_texto(&r.texto, nome)))
            .is_err()
        {
            notificar("Não foi possível gerar o download".into());
        }
    };

    let sem_saida = move || resultado.with(|r| r.texto.is_empty());

    view! {
        <header>
            <h1>"Ajusta Assembly " <span class="destaque">"AVR"</span></h1>
            <p>
                "Desmonta o " <code>".hex"</code> " gerado pelo compilador (ou converte a listagem do "
                <code>"avr-objdump -D"</code> ") em assembly que pode ser montado no Atmel Studio."
            </p>
        </header>

        <main>
            <section class="painel">
                <div class="painel-topo">
                    <h2>"Arquivo .hex ou listagem"</h2>
                    <div class="acoes">
                        <label class="botao secundario">
                            "Abrir .hex"
                            <input
                                type="file"
                                accept=".hex,.ihex,.ihx,.lst,.txt,.asm,.s,.dis,text/plain"
                                on:change:target=move |ev| {
                                    let input = ev.target();
                                    if let Some(arquivo) = input.files().and_then(|f| f.get(0)) {
                                        carregar_arquivo(arquivo);
                                    }
                                    input.set_value("");
                                }
                            />
                        </label>
                        <button
                            class="botao secundario"
                            disabled=move || entrada.with(String::is_empty)
                            on:click=move |_| {
                                entrada.set(String::new());
                                nome_saida.set(NOME_SAIDA_PADRAO.to_owned());
                            }
                        >
                            "Limpar"
                        </button>
                    </div>
                </div>
                <AreaEntrada entrada on_arquivo=carregar_arquivo />
            </section>

            <section class="painel">
                <div class="painel-topo">
                    <h2>"Assembly ajustado"</h2>
                    <div class="acoes">
                        <button class="botao" disabled=sem_saida on:click=copiar>
                            "Copiar"
                        </button>
                        <button class="botao secundario" disabled=sem_saida on:click=baixar>
                            "Baixar .asm"
                        </button>
                    </div>
                </div>
                <textarea
                    node_ref=saida
                    readonly
                    spellcheck="false"
                    placeholder="O resultado aparece aqui."
                    prop:value=move || resultado.with(|r| r.texto.clone())
                ></textarea>
            </section>
        </main>

        <footer>
            <div class="opcoes">
                <label>
                    <input type="checkbox" bind:checked=compactar />
                    <span>
                        "Compactar o preenchimento final (" <code>".dw 0xffff"</code> ")"
                    </span>
                </label>
                <label>
                    <input type="checkbox" bind:checked=comentar />
                    <span>"Comentar o endereço (em words) de cada linha"</span>
                </label>
            </div>
            <Resumo resultado />
        </footer>

        <div class="toast" class:visivel=move || aviso.with(Option::is_some) role="status">
            {move || aviso.get().unwrap_or_default()}
        </div>
    }
}

/// Área de texto da entrada, que também aceita arquivos arrastados.
#[component]
fn AreaEntrada(
    entrada: RwSignal<String>,
    on_arquivo: impl Fn(File) + Copy + 'static,
) -> impl IntoView {
    let arrastando = RwSignal::new(false);

    view! {
        <div class="soltar" class:ativo=move || arrastando.get()>
            <textarea
                spellcheck="false"
                placeholder="Abra o arquivo .hex, arraste-o para cá ou cole o conteúdo (Intel HEX ou listagem do avr-objdump)."
                bind:value=entrada
                on:dragover=move |ev| {
                    ev.prevent_default();
                    arrastando.set(true);
                }
                on:dragleave=move |_| arrastando.set(false)
                on:drop=move |ev| {
                    ev.prevent_default();
                    arrastando.set(false);
                    if let Some(arquivo) = ev
                        .data_transfer()
                        .and_then(|d| d.files())
                        .and_then(|f| f.get(0))
                    {
                        on_arquivo(arquivo);
                    }
                }
            ></textarea>
        </div>
    }
}

/// Estatísticas da conversão e lista das linhas que não puderam ser lidas.
#[component]
fn Resumo(resultado: Memo<Resultado>) -> impl IntoView {
    const MAX_AVISOS: usize = 100;

    let estatisticas = move || {
        resultado.with(|r| {
            let e = &r.estatisticas;
            let mut partes = vec![
                format!("{} linhas lidas", milhar(e.linhas_lidas)),
                format!("{} instruções geradas", milhar(e.instrucoes)),
            ];
            if e.preenchimento_removido > 0 {
                partes.push(format!(
                    "{} words de preenchimento compactadas",
                    milhar(e.preenchimento_removido)
                ));
            }
            partes.join(" · ")
        })
    };
    let quantidade = move || resultado.with(|r| r.avisos.len());

    view! {
        <div class="resumo">
            <span>{estatisticas}</span>
            <Show when=move || { quantidade() > 0 }>
                <details class="avisos">
                    <summary>
                        {move || match quantidade() {
                            1 => "1 linha ignorada".to_owned(),
                            n => format!("{} linhas ignoradas", milhar(n)),
                        }}
                    </summary>
                    <ul>
                        {move || {
                            resultado.with(|r| {
                                r.avisos
                                    .iter()
                                    .take(MAX_AVISOS)
                                    .map(|a| {
                                        view! {
                                            <li>
                                                <b>"linha " {a.linha}</b>
                                                ": "
                                                {a.mensagem.clone()}
                                            </li>
                                        }
                                    })
                                    .collect_view()
                            })
                        }}
                    </ul>
                </details>
            </Show>
        </div>
    }
}

/// `prog.hex` → `prog.asm`.
fn nome_asm(nome: &str) -> String {
    let base = nome.rsplit_once('.').map_or(nome, |(base, _)| base);
    format!("{base}.asm")
}

/// Formata com separador de milhar, como em "16.276".
fn milhar(n: usize) -> String {
    let digitos = n.to_string();
    let mut saida = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            saida.push('.');
        }
        saida.push(c);
    }
    saida
}

/// Copia selecionando o texto da área. `execCommand` é obsoleto, mas, ao
/// contrário da Clipboard API, funciona dentro de iframes que não têm a
/// permissão `clipboard-write` (como as páginas do Canvas LMS).
fn copiar_por_selecao(area: &HtmlTextAreaElement) -> bool {
    area.select();
    let copiou = document()
        .unchecked_into::<HtmlDocument>()
        .exec_command("copy")
        .unwrap_or(false);
    let _ = area.set_selection_range(0, 0);
    copiou
}

async fn copiar_por_clipboard_api(texto: &str) -> bool {
    let navigator = window().navigator();
    // Ausente em contextos inseguros (http) de alguns navegadores.
    let disponivel =
        js_sys::Reflect::get(&navigator, &"clipboard".into()).is_ok_and(|c| !c.is_undefined());
    disponivel
        && JsFuture::from(navigator.clipboard().write_text(texto))
            .await
            .is_ok()
}

fn baixar_texto(texto: &str, nome: &str) -> Result<(), JsValue> {
    let partes = js_sys::Array::of1(&texto.into());
    let opcoes = BlobPropertyBag::new();
    opcoes.set_type("text/plain;charset=utf-8");
    let blob = Blob::new_with_str_sequence_and_options(&partes, &opcoes)?;
    let url = Url::create_object_url_with_blob(&blob)?;

    let link: HtmlAnchorElement = document().create_element("a")?.unchecked_into();
    link.set_href(&url);
    link.set_download(nome);
    link.click();
    Url::revoke_object_url(&url)
}
