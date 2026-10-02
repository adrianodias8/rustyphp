use std::process::ExitCode;

use ferro_edge::config::USAGE;
use ferro_edge::{Config, Edge};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    let cfg = match Config::parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) if e.is_empty() => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("ferro-edge: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(cfg.threads).enable_all().build();
    let rt = match rt {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("ferro-edge: runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(async move {
        let listener = match tokio::net::TcpListener::bind(&cfg.listen).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("ferro-edge: bind {}: {e}", cfg.listen);
                return ExitCode::FAILURE;
            }
        };
        eprintln!("ferro-edge: listening on {} -> {}", cfg.listen, cfg.upstream);
        ferro_edge::run(Edge::new(cfg), listener).await;
        ExitCode::SUCCESS
    })
}
