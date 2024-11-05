const HELLO_NCL: &str = r#"
{ hello = "hello, nickel world!" }
"#;

fn main() {
    println!("starting mi...");

    let field_path_raw = format!("hello");

    use nickel_lang_core::{
        error::report::ErrorFormat, eval::cache::lazy::CBNCache, identifier::LocIdent,
        pretty::ident_quoted, program::Program as Prog,
    };
    let field_path = ident_quoted(&LocIdent::new(field_path_raw));
    use std::io::stderr;
    let mut prog =
        Prog::<CBNCache>::new_from_source(HELLO_NCL.as_bytes(), "hello-string", stderr()).unwrap();
    let res_field = prog.parse_field_path(field_path.clone());
    let Ok(field) = res_field else {
        prog.report(res_field.unwrap_err(), ErrorFormat::Text);
        panic!("failed to parse {field_path:?} as Nickel path");
    };
    prog.field = field;
    let res_term = prog.eval_full_for_export();
    let Ok(term) = res_term else {
        prog.report(res_term.unwrap_err(), ErrorFormat::Text);
        panic!("script failed");
    };
    let simple_term = &*term.term;
    println!("RESULT: {simple_term:?}");
}
