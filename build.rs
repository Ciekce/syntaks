use std::{env, fs, path::PathBuf, process::Command};

const NET: &str = "cnet013_0k";
const DOWNLOAD_URL: &str = "https://github.com/Ciekce/syntaks-nets/releases/download/";

fn main() {
    println!("cargo::rerun-if-env-changed=EVALFILE");

    if let Ok(p) = env::var("EVALFILE") {
        eprintln!("using {p}");
        println!("cargo::rustc-env=EVALFILE={}", fs::canonicalize(p).unwrap().display());
        return;
    }

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join(format!("{NET}.nnue"));

    if out.exists() {
        eprintln!("reusing {NET}");
    } else {
        let url = format!("{DOWNLOAD_URL}{NET}/{NET}.nnue");
        eprintln!("downloading {NET} from {url}");
        let status = Command::new("curl")
            .args(["-sSfL", "-o"])
            .arg(&out)
            .arg(&url)
            .status()
            .unwrap();
        assert!(status.success(), "failed to download {NET} from {url}");
    }

    println!("cargo::rustc-env=EVALFILE={}", out.display());
}
