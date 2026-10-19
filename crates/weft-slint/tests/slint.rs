//! Weft → Slint → Weft over corpus screens and a fixture of the other mapped controls: the
//! generated `.slint` matches its reviewed golden file, compiles with the Slint compiler, and
//! reads back to the canonical document, which the Weft validator accepts. `WEFT_UPDATE_FIXTURES=1`
//! rewrites the golden files.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use i_slint_core::input::key_codes::LeftArrow;
use i_slint_core::platform::WindowEvent;
use slint_interpreter::{ComponentHandle, LogicalPosition, PhysicalSize, SharedString, Value};
use weft_catalog::token_types;
use weft_core::{Mode, ValidateOptions, serialize, validate_document};
use weft_slint::{
    CanvasChromeError, CanvasFrame, ComposeError, EmbeddedComponent, GenerateError,
    GenerateOptions, ImportError, ImportOptions, MAX_SOURCE_LENGTH, canvas_chrome, compose,
    generate, generate_embedded, import_slint, read_slint, runtime,
};

use common::{document, ensure_backend, exported_component, here, screens, setup};

/// Error-level diagnostics of the Slint compiler, and whether it built `component`.
fn compile(source: &str, component: &str) -> (Vec<String>, bool) {
    match runtime::compile(source, "fluent") {
        Ok(definition) => (vec![], definition.name() == component),
        Err(errors) => (errors, false),
    }
}

#[test]
fn generated_slint_matches_the_golden_files() {
    let (catalog, tokens) = setup();
    let update = std::env::var_os("WEFT_UPDATE_FIXTURES").is_some();
    for (name, path) in screens() {
        let doc = document(&std::fs::read_to_string(&path).unwrap(), &catalog, &tokens);
        let options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        };
        let slint = generate(&doc, &options).unwrap();
        let golden = here(&format!("tests/fixtures/{name}.slint"));
        if update {
            std::fs::write(&golden, &slint).unwrap();
        }
        assert_eq!(slint, std::fs::read_to_string(&golden).unwrap(), "{name}");
    }
}

#[test]
fn generated_slint_compiles() {
    let (catalog, tokens) = setup();
    for (name, path) in screens() {
        let doc = document(&std::fs::read_to_string(&path).unwrap(), &catalog, &tokens);
        let options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        };
        let slint = generate(&doc, &options).unwrap();
        let (errors, built) = compile(&slint, exported_component(&slint));
        assert!(errors.is_empty() && built, "{name}: {errors:#?}");
    }
}

#[test]
fn generated_slint_reads_back_to_the_canonical_document() {
    let (catalog, tokens) = setup();
    for (name, path) in screens() {
        let markup = std::fs::read_to_string(&path).unwrap();
        let doc = document(&markup, &catalog, &tokens);
        let gen_options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        };
        let slint = generate(&doc, &gen_options).unwrap();
        let options = ImportOptions {
            catalog: &catalog,
            tokens: &tokens,
        };
        let back = import_slint(&slint, &options).unwrap();
        assert_eq!(serialize(&back), markup, "{name}");
        let types = token_types(&tokens);
        let checked = ValidateOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            tokens: Some(&types),
            actions: None,
        };
        assert!(validate_document(&back, &checked).is_empty(), "{name}");
        // A reindented file is the same file.
        let indented: String = slint.lines().map(|l| format!("  {l}\n")).collect();
        assert_eq!(import_slint(&indented, &options).unwrap(), back, "{name}");
    }
}

#[test]
fn a_named_screen_keeps_its_name_through_the_round_trip() {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: Some("sign-in"),
    };
    let slint = generate(&doc, &options).unwrap();
    assert!(slint.contains("export component SignInScreen inherits Window"));
    let import = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    assert_eq!(serialize(&import_slint(&slint, &import).unwrap()), markup);
}

#[test]
fn a_bound_heading_level_is_refused_with_its_path() {
    let (catalog, tokens) = setup();
    let markup =
        r#"<screen id="s" weft="0.1"><heading id="h" level="{$.n}">Title</heading></screen>"#;
    let doc = document(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let Err(GenerateError::Unsupported(problems)) = generate(&doc, &options) else {
        panic!("a bound heading level must be refused");
    };
    assert_eq!(problems[0].path, "heading#h");
}

#[test]
fn ids_that_slint_reads_as_one_are_refused() {
    let (catalog, tokens) = setup();
    let markup =
        r#"<screen id="s" weft="0.1"><text id="a-b">A</text><text id="a_b">B</text></screen>"#;
    let doc = document(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let Err(GenerateError::Unsupported(problems)) = generate(&doc, &options) else {
        panic!("clashing ids must be refused");
    };
    assert_eq!(problems[0].path, "text#a_b");
}

#[test]
fn document_text_cannot_escape_its_string_literal() {
    let (catalog, tokens) = setup();
    let markup = r#"<screen id="s" weft="0.1"><text id="t">"; } export component X { \{root.y} &#10;</text></screen>"#;
    let doc = document(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let slint = generate(&doc, &options).unwrap();
    let (errors, built) = compile(&slint, "SScreen");
    assert!(errors.is_empty() && built, "{errors:#?}");
    assert_eq!(
        slint.lines().filter(|l| l.starts_with("export ")).count(),
        1
    );
}

#[test]
fn edited_or_foreign_slint_is_not_taken_for_its_comment() {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let slint = generate(&doc, &options).unwrap();
    let import = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    let edited = slint.replace("text: \"Sign in\";", "text: \"Log in\";");
    assert!(matches!(
        import_slint(&edited, &import),
        Err(ImportError::Edited)
    ));
    let tampered = slint.replace(
        "// <screen id=\"login\" label=\"Sign in\"",
        "// <screen id=\"login\" label=\"Other\"",
    );
    assert!(matches!(
        import_slint(&tampered, &import),
        Err(ImportError::Edited)
    ));
    let foreign = "export component A inherits Window {}\n";
    assert!(matches!(
        import_slint(foreign, &import),
        Err(ImportError::NoSource)
    ));
    let broken = "// weft:source slint\n// <screen id=\"s\">\n";
    assert!(matches!(
        import_slint(broken, &import),
        Err(ImportError::Invalid(_))
    ));
    let long = " ".repeat(MAX_SOURCE_LENGTH + 1);
    assert!(matches!(
        import_slint(&long, &import),
        Err(ImportError::TooLong)
    ));
}

#[test]
fn a_fragment_use_is_generated_as_its_expansion() {
    let (mut catalog, tokens) = setup();
    let fragment = r#"<fragment weft="0.2"><param name="back" type="action"/><stack id="bar"><button id="back" on-press="{$back}">Back</button></stack></fragment>"#;
    let fragment = document(fragment, &catalog, &tokens);
    catalog
        .fragments
        .insert("header".into(), weft_core::Fragment::new(fragment));
    let markup = r#"<screen id="s" weft="0.2"><use id="top" fragment="header" on-back="nav.back"/></screen>"#;
    let doc = document(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let source = generate(&doc, &options).unwrap();
    assert!(source.contains("top--back := Button"), "{source}");
    assert!(
        source.contains(r#"root.perform("nav.back", "top/back");"#),
        "{source}"
    );
    let (errors, built) = compile(&source, exported_component(&source));
    assert!(built, "{errors:?}");
    // The source comment keeps the use, so the file reads back to the screen as written.
    let import = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    assert_eq!(import_slint(&source, &import).unwrap(), doc);
}

#[test]
fn layout_props_read_back_without_the_source_comment() {
    let (catalog, tokens) = setup();
    let markup = r#"<screen id="lay" weft="0.1">
  <stack id="row" direction="row" gap="{token.space.sm}" justify="space-between" padding="{token.space.md}" max-width="{token.size.sm}">
    <text id="a">A</text>
    <button id="b" grow="true">B</button>
    <text id="c">C</text>
  </stack>
</screen>
"#;
    let doc = document(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let slint = generate(&doc, &options).unwrap();
    assert!(slint.contains("alignment: LayoutAlignment.space-between;"));
    assert!(slint.contains("horizontal-stretch: 1;"));
    assert!(slint.contains("padding:"));
    assert!(slint.contains("max-width:"));
    // Drop the source comment: the properties alone must give the layout props back.
    let bare: String = slint
        .lines()
        .filter(|l| !l.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let import = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    let back = read_slint(&bare, &import).document;
    let row = &back.root.children[0];
    let weft_core::Child::Node(row) = row else {
        panic!("expected a node");
    };
    assert_eq!(
        row.props.get("justify"),
        Some(&weft_core::Value::String("space-between".into()))
    );
    assert_eq!(
        row.props.get("padding"),
        Some(&weft_core::Value::Token("space.md".into()))
    );
    assert_eq!(
        row.props.get("max-width"),
        Some(&weft_core::Value::Token("size.sm".into()))
    );
    let weft_core::Child::Node(button) = &row.children[1] else {
        panic!("expected a node");
    };
    assert_eq!(
        button.props.get("grow"),
        Some(&weft_core::Value::Bool(true))
    );
}

/// One host window holding the embedded components, the shape milestone 2 compiles.
fn host_of(parts: &[EmbeddedComponent]) -> String {
    let mut imports = std::collections::BTreeSet::new();
    for part in parts {
        imports.extend(part.imports.iter().copied());
    }
    let mut out = String::new();
    if !imports.is_empty() {
        let list: Vec<&str> = imports.into_iter().collect();
        out.push_str(&format!(
            "import {{ {} }} from \"std-widgets.slint\";\n\n",
            list.join(", ")
        ));
    }
    for part in parts {
        out.push_str(&part.source);
        if !part.source.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str("export component Host inherits Window {\n");
    for part in parts {
        out.push_str(&format!("    {} {{}}\n", part.name));
    }
    out.push_str("}\n");
    out
}

#[test]
fn an_embedded_screen_compiles_inside_a_host_window() {
    let (catalog, tokens) = setup();
    let mut embedded = Vec::new();
    let mut refused = Vec::new();
    for (name, path) in screens() {
        let doc = document(&std::fs::read_to_string(&path).unwrap(), &catalog, &tokens);
        let options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        };
        match generate_embedded(&doc, &options) {
            Ok(part) => {
                assert!(
                    part.source
                        .starts_with(&format!("component {} inherits Rectangle {{", part.name)),
                    "{name}"
                );
                assert!(!part.source.contains("inherits Window"), "{name}");
                assert!(!part.source.contains("export component"), "{name}");
                let host = host_of(std::slice::from_ref(&part));
                let (errors, built) = compile(&host, "Host");
                assert!(errors.is_empty() && built, "{name}: {errors:#?}\n{host}");
                embedded.push((name, part));
            }
            Err(GenerateError::Unsupported(problems)) => {
                assert!(
                    problems
                        .iter()
                        .all(|problem| problem.message.contains("MenuBar")),
                    "{name}: {problems:?}"
                );
                assert!(generate(&doc, &options).is_ok(), "{name}");
                refused.push(name);
            }
            Err(error) => panic!("{name}: {error}"),
        }
    }
    // Corpus screens that carry a menu: a MenuBar cannot leave the Window form.
    assert_eq!(refused, ["inbox", "menu"]);
    let login = embedded
        .iter()
        .find(|(name, _)| name == "login")
        .expect("login");
    assert!(
        login
            .1
            .source
            .contains("accessible-role: AccessibleRole.main;")
    );
    assert!(login.1.source.contains("accessible-label: \"Sign in\";"));
    assert!(
        login
            .1
            .source
            .contains("preferred-width: login.preferred-width;")
    );
    assert!(login.1.source.contains("width: 100%;"));
    assert!(!login.1.source.contains("title:"));
    let other = embedded
        .iter()
        .find(|(name, _)| name != "login")
        .expect("a second screen");
    let host = host_of(&[login.1.clone(), other.1.clone()]);
    let (errors, built) = compile(&host, "Host");
    assert!(errors.is_empty() && built, "{errors:#?}\n{host}");
}

fn embedded<'a>(
    name: &str,
    catalog: &'a weft_core::Catalog,
    tokens: &'a indexmap::IndexMap<String, weft_catalog::Token>,
) -> weft_slint::EmbeddedComponent {
    let path = screens()
        .into_iter()
        .find(|(found, _)| found == name)
        .unwrap()
        .1;
    let doc = document(&std::fs::read_to_string(path).unwrap(), catalog, tokens);
    generate_embedded(
        &doc,
        &GenerateOptions {
            catalog,
            tokens,
            name: None,
        },
    )
    .unwrap()
}

#[test]
fn compose_refuses_a_duplicate_name_and_a_bad_length() {
    let (catalog, tokens) = setup();
    let login = embedded("login", &catalog, &tokens);
    let duplicate = compose(&[
        CanvasFrame {
            component: &login,
            x: 0.0,
            y: 0.0,
            width: 390.0,
        },
        CanvasFrame {
            component: &login,
            x: 400.0,
            y: 0.0,
            width: 390.0,
        },
    ]);
    assert_eq!(
        duplicate,
        Err(ComposeError::Duplicate("LoginScreen".into()))
    );
    let bad = compose(&[CanvasFrame {
        component: &login,
        x: 0.0,
        y: f64::NAN,
        width: 390.0,
    }]);
    assert_eq!(bad, Err(ComposeError::Length));
    let zero = compose(&[CanvasFrame {
        component: &login,
        x: 0.0,
        y: 0.0,
        width: 0.0,
    }]);
    assert_eq!(zero, Err(ComposeError::Length));
}

#[test]
fn a_canvas_of_two_screens_compiles_and_paints_both() {
    let (catalog, tokens) = setup();
    let login = embedded("login", &catalog, &tokens);
    let account = embedded("account", &catalog, &tokens);
    let source = compose(&[
        CanvasFrame {
            component: &login,
            x: 32.0,
            y: 24.0,
            width: 360.0,
        },
        CanvasFrame {
            component: &account,
            x: 440.0,
            y: 24.0,
            width: 360.0,
        },
    ])
    .unwrap();
    assert!(source.contains("export component Canvas inherits Window"));
    assert!(source.contains("in-out property <length> pan-x: 0px;"));
    assert!(source.contains("frame-0 := LoginScreen"));
    assert!(source.contains("frame-1 := AccountScreen"));
    assert!(source.contains("x: 32px + root.pan-x;"));
    assert!(source.contains("width: 360px * root.zoom;"));
    let (errors, built) = compile(&source, "Canvas");
    assert!(errors.is_empty() && built, "{errors:#?}\n{source}");

    ensure_backend();
    let instance = runtime::compile(&source, "fluent")
        .unwrap()
        .create()
        .unwrap();
    instance
        .window()
        .set_size(slint_interpreter::PhysicalSize::new(1600, 900));
    let buffer = instance.window().take_snapshot().unwrap();
    let bytes = buffer.as_bytes();
    let width = buffer.width();
    let height = buffer.height();
    assert!(
        paints(bytes, width, height, 32, 200, 24, 160),
        "login frame is blank"
    );
    assert!(
        paints(bytes, width, height, 440, 640, 24, 160),
        "account frame is blank"
    );
}

#[test]
fn dragging_the_canvas_background_pans_it() {
    let block = EmbeddedComponent {
        name: "BlockScreen".into(),
        imports: vec![],
        source: "\
component BlockScreen inherits Rectangle {
    background: red;
    preferred-width: 100px;
    preferred-height: 80px;
    control := TouchArea { width: 100%; height: 100%; }
}
"
        .into(),
    };
    let source = compose(&[CanvasFrame {
        component: &block,
        x: 200.0,
        y: 40.0,
        width: 100.0,
    }])
    .unwrap();
    let touch = source.find("touch := TouchArea").unwrap();
    let frame = source.find("frame-0 := BlockScreen").unwrap();
    assert!(touch < frame, "the background sits behind the frames");
    let (errors, built) = compile(&source, "Canvas");
    assert!(errors.is_empty() && built, "{errors:#?}\n{source}");

    ensure_backend();
    let instance = runtime::compile(&source, "fluent")
        .unwrap()
        .create()
        .unwrap();
    let window = instance.window();
    window.set_size(slint_interpreter::PhysicalSize::new(800, 600));
    // Layout runs with the snapshot, so the frame has a size before the pointer arrives.
    window.take_snapshot().unwrap();

    let at = |x, y| i_slint_core::api::LogicalPosition::new(x, y);
    let left = i_slint_core::items::PointerEventButton::Left;
    let right = i_slint_core::items::PointerEventButton::Right;
    let send = |event| window.dispatch_event(event);
    let pan = || {
        (
            instance.get_property("pan-x").unwrap(),
            instance.get_property("pan-y").unwrap(),
        )
    };
    let number = slint_interpreter::Value::Number;

    send(i_slint_core::platform::WindowEvent::PointerPressed {
        position: at(20.0, 20.0),
        button: right,
    });
    send(i_slint_core::platform::WindowEvent::PointerMoved {
        position: at(90.0, 90.0),
    });
    assert_eq!(pan(), (number(0.0), number(0.0)));
    send(i_slint_core::platform::WindowEvent::PointerReleased {
        position: at(90.0, 90.0),
        button: right,
    });

    // The frame's own TouchArea covers (200, 40) at 100 by 80, so this drag is not the background.
    send(i_slint_core::platform::WindowEvent::PointerPressed {
        position: at(220.0, 60.0),
        button: left,
    });
    send(i_slint_core::platform::WindowEvent::PointerMoved {
        position: at(280.0, 100.0),
    });
    assert_eq!(pan(), (number(0.0), number(0.0)));
    send(i_slint_core::platform::WindowEvent::PointerReleased {
        position: at(280.0, 100.0),
        button: left,
    });

    send(i_slint_core::platform::WindowEvent::PointerPressed {
        position: at(20.0, 20.0),
        button: left,
    });
    send(i_slint_core::platform::WindowEvent::PointerMoved {
        position: at(70.0, 50.0),
    });
    assert_eq!(pan(), (number(50.0), number(30.0)));
    assert_eq!(instance.get_property("zoom").unwrap(), number(1.0));
    send(i_slint_core::platform::WindowEvent::PointerReleased {
        position: at(70.0, 50.0),
        button: left,
    });

    send(i_slint_core::platform::WindowEvent::PointerPressed {
        position: at(70.0, 50.0),
        button: left,
    });
    send(i_slint_core::platform::WindowEvent::PointerMoved {
        position: at(80.0, 60.0),
    });
    assert_eq!(pan(), (number(60.0), number(40.0)));
}

#[test]
fn the_canvas_chrome_zooms_pans_with_keys_and_draws_rulers() {
    let (catalog, tokens) = setup();
    let login = embedded("login", &catalog, &tokens);
    let account = embedded("account", &catalog, &tokens);
    let composed = compose(&[
        CanvasFrame {
            component: &login,
            x: 32.0,
            y: 24.0,
            width: 360.0,
        },
        CanvasFrame {
            component: &account,
            x: 440.0,
            y: 24.0,
            width: 360.0,
        },
    ])
    .unwrap();
    let source = canvas_chrome(&composed).unwrap();
    assert!(source.contains("for tick in 40:"));
    assert!(source.contains("Key.LeftArrow"));
    assert!(source.contains("touch := TouchArea"));
    assert!(source.contains("frame-0 := LoginScreen"));
    assert!(!source.contains("MenuBar"));
    let window = generate(
        &document(
            &std::fs::read_to_string(
                screens()
                    .into_iter()
                    .find(|(name, _)| name == "login")
                    .unwrap()
                    .1,
            )
            .unwrap(),
            &catalog,
            &tokens,
        ),
        &GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        },
    )
    .unwrap();
    assert_eq!(canvas_chrome(&window), Err(CanvasChromeError::NotCanvas));

    let (errors, built) = compile(&source, "Canvas");
    assert!(errors.is_empty() && built, "{errors:#?}\n{source}");

    ensure_backend();
    let instance = runtime::compile(&source, "fluent")
        .unwrap()
        .create()
        .unwrap();
    instance.show().unwrap();
    instance.window().set_size(PhysicalSize::new(1600, 900));
    let buffer = instance.window().take_snapshot().unwrap();
    assert!(
        ruler_tick(buffer.as_bytes(), buffer.width(), buffer.height()),
        "ruler tick is missing"
    );

    instance.window().dispatch_event(WindowEvent::KeyPressed {
        text: SharedString::from(LeftArrow),
    });
    assert_eq!(instance.get_property("pan-x").unwrap(), Value::Number(40.0));

    instance
        .window()
        .dispatch_event(WindowEvent::PointerScrolled {
            position: LogicalPosition::new(400.0, 400.0),
            delta_x: 0.0,
            delta_y: 20.0,
        });
    let zoom = match instance.get_property("zoom").unwrap() {
        Value::Number(zoom) => zoom,
        other => panic!("zoom is {other:?}"),
    };
    assert!((zoom - 1.1).abs() < 0.001, "zoom is {zoom}");
}

/// A dark tick on the light top ruler, at the first mark past the corner.
fn ruler_tick(bytes: &[u8], width: u32, height: u32) -> bool {
    if height < 24 || width < 48 {
        return false;
    }
    let i = ((20 * width + 40) * 4) as usize;
    bytes.get(i).is_some_and(|red| *red < 200)
}

fn paints(bytes: &[u8], width: u32, height: u32, x0: u32, x1: u32, y0: u32, y1: u32) -> bool {
    let bg = &bytes[..4];
    for y in y0..y1.min(height) {
        for x in x0..x1.min(width) {
            let i = ((y * width + x) * 4) as usize;
            if bytes.get(i..i + 4).is_some_and(|px| px != bg) {
                return true;
            }
        }
    }
    false
}
