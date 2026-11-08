use std::{env, fs, path::PathBuf, process::Command};

const NET: &str = "cnet022";
const DATAGEN_NET: &str = "cdnet_001";
const DOWNLOAD_URL: &str = "https://github.com/Ciekce/syntaks-nets/releases/download/";

fn main() {
    println!("cargo::rerun-if-env-changed=EVALFILE");

    if let Ok(p) = env::var("EVALFILE") {
        eprintln!("using {p}");
        println!("cargo::rustc-env=EVALFILE={}", fs::canonicalize(p).unwrap().display());
        return;
    }

    let net = if env::var_os("CARGO_FEATURE_DATAGEN").is_some() { DATAGEN_NET } else { NET };

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join(format!("{net}.nnue"));

    if out.exists() {
        eprintln!("reusing {net}");
    } else {
        let url = format!("{DOWNLOAD_URL}{net}/{net}.nnue");
        eprintln!("downloading {net} from {url}");
        let status = Command::new("curl")
            .args(["-sSfL", "-o"])
            .arg(&out)
            .arg(&url)
            .status()
            .unwrap();
        assert!(status.success(), "failed to download {net} from {url}");
    }

    println!("cargo::rustc-env=EVALFILE={}", out.display());
}
