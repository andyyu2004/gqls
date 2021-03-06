// Generate node names from `parser.c`

use std::io::Write;

// TODO use the `node_ids` instead of the string representation for efficiency
#[test]
fn generate_node_kinds() -> std::io::Result<()> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tree-sitter-graphql/src/parser.c");
    let source = std::fs::read_to_string(path)?;
    let mut lines = source.lines();
    for line in lines.by_ref() {
        if line.contains("ts_symbol_names[]") {
            break;
        }
    }

    let mut f = std::fs::File::create(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/nodes.rs"),
    )?;

    writeln!(f, "#![allow(dead_code)]")?;
    writeln!(f, "pub enum NodeKind {{}}\n")?;
    writeln!(f, "impl NodeKind {{")?;

    for line in lines {
        if line.contains("};") {
            break;
        }
        let line = line.trim();
        if line.starts_with("[sym") {
            let (name, _) = line.split_once(']').unwrap();
            let name = name.trim_start_matches("[sym_");
            let upper = name.to_uppercase();
            writeln!(f, "    pub const {upper}: &'static str = \"{name}\";")?;
        }
    }

    writeln!(f, "}}")?;
    Ok(())
}
