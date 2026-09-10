use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const HEADERS: &str = "e3b1eec08173d6b825cd3ac88c885a63b621504a"; // Vulkan 1.4.357
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn run(command: &mut Command) -> Result<Output> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "{command:?} failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn check_headers(root: &Path) -> Result {
    let vendor = root.join("vendor/Vulkan-Headers");
    if !vendor.join("include/vulkan/vulkan_core.h").exists() {
        return Err("Run git submodule update --init vendor/Vulkan-Headers first".into());
    }
    let revision = run(Command::new("git")
        .arg("-C")
        .arg(&vendor)
        .args(["rev-parse", "HEAD"]))?;
    if String::from_utf8(revision.stdout)?.trim() != HEADERS {
        return Err(format!("Expected Vulkan-Headers revision {HEADERS}").into());
    }
    run(Command::new("git")
        .arg("-C")
        .arg(&vendor)
        .args(["diff", "--exit-code", "HEAD", "--"]))?;
    Ok(())
}

fn bindings(root: &Path, check: bool) -> Result {
    check_headers(root)?;
    let mut builder = bindgen::Builder::default()
        .header(
            root.join("vendor/Vulkan-Headers/include/vulkan/vulkan_core.h")
                .to_string_lossy(),
        )
        .clang_arg(format!(
            "-I{}",
            root.join("vendor/Vulkan-Headers/include").display()
        ))
        .clang_arg("-DVK_NO_PROTOTYPES")
        .clang_arg("--target=x86_64-unknown-linux-gnu")
        .allowlist_var("VK_(TRUE|FALSE|HEADER_VERSION|MAX_PHYSICAL_DEVICE_NAME_SIZE|WHOLE_SIZE)")
        .derive_default(true)
        .formatter(bindgen::Formatter::Prettyplease)
        .raw_line(
            "// Derived from Vulkan-Headers 1.4.357, e3b1eec08173d6b825cd3ac88c885a63b621504a.",
        )
        .raw_line("// Copyright 2015-2026 The Khronos Group Inc. See LICENSE-KHRONOS.")
        .raw_line("// SPDX-License-Identifier: MIT")
        .generate_comments(false)
        .rust_target("1.85".parse()?)
        .layout_tests(true);
    for line in fs::read_to_string(root.join("tools/vulkan-types.txt"))?.lines() {
        if !line.is_empty() && !line.starts_with('#') {
            builder = builder.allowlist_type(line);
        }
    }
    for line in fs::read_to_string(root.join("tools/vulkan-commands.txt"))?.lines() {
        if !line.is_empty() && !line.starts_with('#') {
            builder = builder.allowlist_type(format!("PFN_{line}"));
        }
    }
    let generated = builder.generate()?.to_string();
    let destination = root.join("crates/vulkan-sys/src/bindings.rs");
    if check {
        if fs::read_to_string(destination)? != generated {
            return Err("Bindings are stale; run cargo xtask bindings".into());
        }
        println!("Pinned Vulkan bindings reproduce exactly.");
    } else {
        fs::write(destination, generated)?;
        println!("Generated Vulkan 1.4.357 bindings (Linux x86-64).");
    }
    Ok(())
}

fn build(root: &Path) -> Result {
    run(
        Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(root)
            .args(["build", "--locked", "-p", "ogpu", "-p", "ogpu-vulkan-sys"]),
    )?;
    Ok(())
}

fn compiler() -> Command {
    Command::new(env::var_os("CC").unwrap_or_else(|| "cc".into()))
}

fn abi(root: &Path) -> Result {
    check_headers(root)?;
    build(root)?;
    let target = root.join("target");
    let scratch = target.join("abi");
    fs::create_dir_all(&scratch)?;
    let mut c = String::from("#include <stddef.h>\n#include <stdio.h>\n#include <vulkan/vulkan_core.h>\n#include \"ogpu.h\"\nint main(void) {\n");
    let mut rust = String::from("fn main() {\n");
    for line in fs::read_to_string(root.join("tests/abi.txt"))?.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let namespace = words.next().ok_or("missing namespace")?;
        let name = words.next().ok_or("missing type")?;
        if namespace == "const" {
            c.push_str(&format!(
                "printf(\"{name} %llu\\n\", (unsigned long long)({name}));\n"
            ));
            rust.push_str(&format!(
                "println!(\"{name} {{}}\", ogpu_vulkan_sys::{name} as u64);\n"
            ));
            continue;
        }
        let ty = format!(
            "{}::{name}",
            if namespace == "vk" {
                "ogpu_vulkan_sys"
            } else {
                "ogpu"
            }
        );
        c.push_str(&format!(
            "printf(\"{name} %zu %zu\\n\", sizeof({name}), _Alignof({name}));\n"
        ));
        rust.push_str(&format!("println!(\"{name} {{}} {{}}\", std::mem::size_of::<{ty}>(), std::mem::align_of::<{ty}>());\n"));
        for field in words {
            c.push_str(&format!(
                "printf(\"{name}.{field} %zu\\n\", offsetof({name}, {field}));\n"
            ));
            rust.push_str(&format!(
                "println!(\"{name}.{field} {{}}\", std::mem::offset_of!({ty}, {field}));\n"
            ));
        }
    }
    c.push_str("return 0;\n}\n");
    rust.push_str("}\n");
    fs::write(scratch.join("layout.c"), c)?;
    fs::write(scratch.join("layout.rs"), rust)?;
    run(compiler()
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(root.join("include"))
        .arg("-I")
        .arg(root.join("vendor/Vulkan-Headers/include"))
        .arg(scratch.join("layout.c"))
        .arg("-o")
        .arg(scratch.join("c-layout")))?;
    run(Command::new("rustc")
        .arg("--edition=2021")
        .arg(scratch.join("layout.rs"))
        .arg("--extern")
        .arg(format!(
            "ogpu={}",
            target.join("debug/libogpu.rlib").display()
        ))
        .arg("--extern")
        .arg(format!(
            "ogpu_vulkan_sys={}",
            target.join("debug/libogpu_vulkan_sys.rlib").display()
        ))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            target.join("debug/deps").display()
        ))
        .arg("-o")
        .arg(scratch.join("rust-layout")))?;
    let c = run(&mut Command::new(scratch.join("c-layout")))?.stdout;
    let rust = run(&mut Command::new(scratch.join("rust-layout")))?.stdout;
    if c != rust {
        return Err(format!(
            "ABI mismatch:\nC:\n{}\nRust:\n{}",
            String::from_utf8_lossy(&c),
            String::from_utf8_lossy(&rust)
        )
        .into());
    }
    println!(
        "C/Rust ABI checks passed ({} layout values).",
        c.iter().filter(|&&b| b == b'\n').count()
    );
    Ok(())
}

fn smoke_executable(root: &Path) -> Result<PathBuf> {
    build(root)?;
    let target = root.join("target/debug");
    let executable = target.join("ogpu-probe-c");
    run(compiler()
        .args(["-std=c11", "-DNDEBUG", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(root.join("include"))
        .arg(root.join("examples/probe.c"))
        .arg("-L")
        .arg(&target)
        .arg(format!("-Wl,-rpath,{}", target.display()))
        .arg("-logpu")
        .arg("-o")
        .arg(&executable))?;
    Ok(executable)
}

fn smoke(root: &Path, arguments: &[String]) -> Result {
    let output = run(Command::new(smoke_executable(root)?).args(arguments))?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    Ok(())
}

fn mock(root: &Path) -> Result {
    check_headers(root)?;
    let executable = smoke_executable(root)?;
    let library = root.join("target/debug/libogpu-mock-vulkan.so");
    run(compiler()
        .args([
            "-std=c11", "-Wall", "-Wextra", "-Werror", "-fPIC", "-shared",
        ])
        .arg("-I")
        .arg(root.join("vendor/Vulkan-Headers/include"))
        .arg(root.join("tests/mock_vulkan.c"))
        .arg("-o")
        .arg(&library))?;
    for (mode, argument) in [
        ("normal", "--expect-mock"),
        ("instance11", "--expect-mock"),
        ("empty", "--expect-empty"),
        ("error", "--expect-vulkan-error"),
    ] {
        let output = run(Command::new(&executable)
            .env("OGPU_VULKAN_LIBRARY", &library)
            .env("OGPU_MOCK_MODE", mode)
            .arg(argument))?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    let output = run(Command::new(&executable)
        .env(
            "OGPU_VULKAN_LIBRARY",
            root.join("target/nonexistent-loader/libvulkan.so"),
        )
        .arg("--expect-loader-error"))?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    println!("Mock loader, empty enumeration, cleanup-on-error, and missing loader tests passed.");
    Ok(())
}

fn compute(root: &Path) -> Result {
    c_execution(root, "compute", &["roundtrip"])
}

fn batch(root: &Path) -> Result {
    c_execution(root, "batch", &["produce", "consume"])
}

fn c_execution(root: &Path, name: &str, shaders: &[&str]) -> Result {
    build(root)?;
    let target = root.join("target/debug");
    let executable = target.join(format!("ogpu-{name}-c"));
    run(compiler()
        .args(["-std=c11", "-DNDEBUG", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(root.join("include"))
        .arg(root.join(format!("examples/{name}.c")))
        .arg("-L")
        .arg(&target)
        .arg(format!("-Wl,-rpath,{}", target.display()))
        .arg("-logpu")
        .arg("-o")
        .arg(&executable))?;
    let output = run(Command::new(executable).args(
        shaders
            .iter()
            .map(|name| root.join(format!("examples/shaders/{name}.spv"))),
    ))?;
    checked_vulkan_output(output)
}

fn gpu_tests(root: &Path) -> Result {
    let output = run(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "ogpu",
        "--",
        "--ignored",
        "--nocapture",
    ]))?;
    checked_vulkan_output(output)
}

fn checked_vulkan_output(output: Output) -> Result {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    print!("{stdout}");
    eprint!("{stderr}");
    if stdout.contains("Validation Error:") || stderr.contains("Validation Error:") {
        return Err("Vulkan validation reported an error during execution".into());
    }
    Ok(())
}

fn main() -> Result {
    let root = root();
    // These tasks use a predictable location for the C/Rust test artifacts.
    if env::var_os("CARGO_TARGET_DIR").is_some() {
        return Err(
            "Unset CARGO_TARGET_DIR for xtask (artifacts use the repository's target/)".into(),
        );
    }
    let args: Vec<_> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bindings") if args.len() == 1 => bindings(&root, false),
        Some("bindings")
            if args.get(1).map(String::as_str) == Some("--check") && args.len() == 2 =>
        {
            bindings(&root, true)
        }
        Some("abi") if args.len() == 1 => abi(&root),
        Some("mock") if args.len() == 1 => mock(&root),
        Some("compute") if args.len() == 1 => compute(&root),
        Some("batch") if args.len() == 1 => batch(&root),
        Some("gpu-tests") if args.len() == 1 => gpu_tests(&root),
        Some("smoke") => smoke(&root, &args[1..]),
        _ => Err(
            "Usage: cargo xtask bindings [--check] | abi | mock | compute | batch | gpu-tests | smoke [--expect-loader-error]"
                .into(),
        ),
    }
}
