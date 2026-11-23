use std::path::Path;

/// Um `rerun-if-changed` explicito desliga o watch do pacote inteiro, entao
/// tudo que entra no binario precisa ser declarado a mao. Sem o dist aqui,
/// mudar o frontend nao recompilava o crate e o release embutia o index.html
/// velho (o `tauri build` parecia ter atualizado, mas nao tinha).
fn watch(p: &str) {
    println!("cargo:rerun-if-changed={p}");
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                watch(&path.to_string_lossy());
            } else {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

fn main() {
    for p in ["src", "../dist", "capabilities", "icons"] {
        watch(p);
    }
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=Cargo.toml");
    let mut attrs = tauri_build::Attributes::new();
    // Build de release pede elevação (UAC), como um app que precisa colar em
    // janelas elevadas. Debug fica comoInvoker para não atrapalhar o dev.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        attrs = attrs.windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(
            r#"<?xml version="1.0" encoding="utf-8"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>"#,
        ));
    }
    build_sqlcipher();
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}

/// Compila o SQLCipher (amalgamação) + libtomcrypt e linka estaticamente como
/// `sqlite3`. É isso que dá o banco cifrado sem OpenSSL nem Perl no build.
fn build_sqlcipher() {
    let dir = sqlcipher_src::source_dir();
    let ltc_include = dir.join(sqlcipher_src::LIBTOMCRYPT_INCLUDE_DIR);
    println!(
        "cargo:rerun-if-changed={}",
        dir.join(sqlcipher_src::SOURCE_FILE).display()
    );

    // 1) libtomcrypt: cada arquivo é uma unidade de compilação própria.
    let mut ltc = cc::Build::new();
    ltc.include(&ltc_include)
        .define("LTC_SOURCE", None)
        .define("LTC_NO_PROTOTYPES", None)
        .warnings(false)
        .files(
            sqlcipher_src::LIBTOMCRYPT_SOURCES
                .iter()
                .map(|s| dir.join(s)),
        );
    ltc.compile("tomcrypt");

    // 2) Amalgamação do SQLCipher apontando para o provider libtomcrypt.
    let mut sql = cc::Build::new();
    sql.file(dir.join(sqlcipher_src::SOURCE_FILE))
        .include(dir)
        .include(&ltc_include)
        .define("SQLITE_HAS_CODEC", "1")
        .define("SQLCIPHER_CRYPTO_LIBTOMCRYPT", "1")
        .define("SQLITE_EXTRA_INIT", "sqlcipher_extra_init")
        .define("SQLITE_EXTRA_SHUTDOWN", "sqlcipher_extra_shutdown")
        .define("SQLITE_TEMP_STORE", "2")
        .define("SQLITE_THREADSAFE", "1")
        .define("SQLITE_ENABLE_FTS5", None)
        .define("SQLITE_ENABLE_STAT4", None)
        .define("SQLITE_ENABLE_RTREE", None)
        .define("SQLITE_ENABLE_COLUMN_METADATA", None)
        .define("SQLITE_USE_URI", None)
        .define("HAVE_USLEEP", "1")
        .warnings(false)
        .compile("sqlite3");
    let _ = Path::new(".");
}
