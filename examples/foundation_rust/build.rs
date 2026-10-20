//! Extract the exact M4 SPIR-V arrays, without invoking C or a shader compiler.
//! This narrow consumer bridge is not a general Rust shader-interface generator.
use std::{env, fs, path::PathBuf};
fn main() {
    let path = "../graphics_scene/textured_scene.generated.h";
    println!("cargo:rerun-if-changed={path}");
    let source = fs::read_to_string(path).unwrap();
    let mut generated = String::new();
    for stage in ["vertex", "fragment"] {
        for (field, offset) in [
            ("vertices", 0),
            ("image", 8),
            ("samplerIndex", 12),
            ("lod", 16),
            ("overlay", 20),
        ] {
            assert!(
                source.contains(&format!(
                    "offsetof(Textured_scene_{stage}Arguments, arg_{field}) == {offset},"
                )),
                "M4 argument layout changed"
            );
        }
        assert!(source.contains(&format!("sizeof(Textured_scene_{stage}Arguments) == 24,")));
        assert!(source.contains(&format!(
            "#define textured_scene_{stage}_required_features (OGPU_NEXT_FEATURE_RASTER)"
        )));
        let marker = format!("static const uint32_t textured_scene_{stage}_code[] = {{");
        let body = source
            .split_once(&marker)
            .expect("M4 shader array")
            .1
            .split_once("};")
            .unwrap()
            .0;
        let words: Vec<_> = body
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                u32::from_str_radix(s.strip_prefix("0x").unwrap().strip_suffix('u').unwrap(), 16)
                    .unwrap()
            })
            .collect();
        assert_eq!(words[0], 0x07230203);
        generated.push_str(&format!(
            "pub const {}: &[u32] = &{:?};\n",
            stage.to_uppercase(),
            words
        ));
    }
    // The D3D12 variant artifacts of the same M4 scene (byte arrays; DXIL
    // containers are byte-granular). Source: graphics_scene/textured_dxil.slang.
    for (name, path) in [
        ("VERTEX_DXIL", "../graphics_scene/textured.vs.dxil"),
        ("FRAGMENT_DXIL", "../graphics_scene/textured.fs.dxil"),
    ] {
        println!("cargo:rerun-if-changed={path}");
        let bytes = fs::read(path).expect("M4 DXIL artifact");
        generated.push_str(&format!("pub const {name}: &[u8] = &{:?};\n", bytes));
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("textured.rs"),
        generated,
    )
    .unwrap();
}
