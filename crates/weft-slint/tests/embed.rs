//! An embedded screen is a component with no window, and several of them assemble into one canvas
//! window that compiles.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use weft_core::serialize;
use weft_slint::{
    AssembleError, CanvasScreen, CanvasView, GenerateError, GenerateOptions, ImportOptions,
    assemble_canvas, generate, generate_embedded, import_slint, runtime,
};

use common::{document, here, screens, setup};

fn options<'a>(
    catalog: &'a weft_core::Catalog,
    tokens: &'a indexmap::IndexMap<String, weft_catalog::Token>,
    name: Option<&'a str>,
) -> GenerateOptions<'a> {
    GenerateOptions {
        catalog,
        tokens,
        name,
    }
}

#[test]
fn an_embedded_screen_has_no_window_and_reads_back() {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    let embedded = generate_embedded(&doc, &options(&catalog, &tokens, None)).unwrap();
    assert!(
        embedded.starts_with("// weft:source slint embed\n"),
        "{embedded}"
    );
    assert!(embedded.contains("export component LoginScreen {\n"));
    assert!(!embedded.contains("inherits Window"));
    assert!(!embedded.contains("title: \"Sign in\""));
    let back = import_slint(
        &embedded,
        &ImportOptions {
            catalog: &catalog,
            tokens: &tokens,
        },
    )
    .unwrap();
    assert_eq!(serialize(&back), serialize(&doc));
    // The window form is unchanged, and it is not the embedded form.
    let window = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    assert!(window.contains("export component LoginScreen inherits Window {"));
    assert!(!window.lines().next().unwrap().contains("embed"));
}

#[test]
fn a_named_embed_keeps_the_name_on_the_comment() {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    let embedded = generate_embedded(&doc, &options(&catalog, &tokens, Some("sign-in"))).unwrap();
    assert!(embedded.starts_with("// weft:source slint name=sign-in embed\n"));
    assert!(embedded.contains("export component SignInScreen {\n"));
    let back = import_slint(
        &embedded,
        &ImportOptions {
            catalog: &catalog,
            tokens: &tokens,
        },
    )
    .unwrap();
    assert_eq!(serialize(&back), serialize(&doc));
}

#[test]
fn a_menu_is_not_embedded() {
    let (catalog, tokens) = setup();
    for name in ["menu", "inbox"] {
        let markup =
            std::fs::read_to_string(here(&format!("../../corpus/{name}/screen.weft"))).unwrap();
        let doc = document(&markup, &catalog, &tokens);
        let error = generate_embedded(&doc, &options(&catalog, &tokens, None)).unwrap_err();
        let GenerateError::Unsupported(problems) = error else {
            panic!("{name}: {error:?}");
        };
        assert!(
            problems
                .iter()
                .any(|problem| problem.message.contains("menu bar")),
            "{name}: {problems:?}"
        );
    }
}

#[test]
fn every_embeddable_corpus_screen_compiles_inside_one_canvas() {
    let (catalog, tokens) = setup();
    let mut screens_out = Vec::new();
    for (index, (name, path)) in screens().into_iter().enumerate() {
        let doc = document(&std::fs::read_to_string(&path).unwrap(), &catalog, &tokens);
        match generate_embedded(&doc, &options(&catalog, &tokens, Some(&name))) {
            Ok(source) => screens_out.push(CanvasScreen {
                source,
                screen_id: name,
                index,
                x: index as f64 * 528.0,
                y: 0.0,
                width: 480.0,
                height: 800.0,
            }),
            Err(GenerateError::Unsupported(problems))
                if problems
                    .iter()
                    .any(|problem| problem.message.contains("menu bar")) => {}
            Err(error) => panic!("{name}: {error}"),
        }
    }
    assert!(screens_out.len() > 2, "{}", screens_out.len());
    let source = assemble_canvas(
        &screens_out,
        &CanvasView {
            pan_x: -48.0,
            pan_y: 0.0,
            zoom: 1.25,
            selected_frame: Some(0),
            selected_path: "/screen#login".to_owned(),
            hover_path: String::new(),
        },
    )
    .unwrap();
    assert!(source.contains("export component WeftStudioCanvas inherits Window {"));
    assert!(source.contains("content-x <=> root.pan-x;"));
    assert!(source.contains("in-out property <float> zoom: 1.25;"));
    assert!(source.contains("in-out property <length> pan-x: -48px;"));
    assert!(source.contains("selected-path: \"/screen#login\";"));
    // The screens are private components. The window is the file's one export.
    assert_eq!(source.matches("export component ").count(), 1);
    let compiled = runtime::compile(&source, "fluent");
    if let Err(errors) = &compiled {
        panic!("{errors:?}\n{source}");
    }
    assert_eq!(compiled.unwrap().name(), "WeftStudioCanvas");
}

#[test]
fn a_source_without_a_component_is_refused() {
    let error = assemble_canvas(
        &[CanvasScreen {
            source: "// weft:source slint embed\n".to_owned(),
            screen_id: "login".to_owned(),
            index: 0,
            x: 0.0,
            y: 0.0,
            width: 480.0,
            height: 800.0,
        }],
        &CanvasView {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            selected_frame: None,
            selected_path: String::new(),
            hover_path: String::new(),
        },
    )
    .unwrap_err();
    assert!(matches!(error, AssembleError::NoComponent));
}
