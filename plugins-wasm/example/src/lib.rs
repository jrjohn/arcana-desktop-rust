//! Example third-party plugin. `run-command` also exposes the misbehaviours the host's
//! sandbox tests rely on (spin, alloc, denied fs/net, failing call).

wit_bindgen::generate!({ world: "plugin", path: "../../crates/arcana-plugin-wasm/wit" });

use arcana::plugin::{host_fs, host_log, host_net, host_state};

struct Example;

impl Guest for Example {
    fn abi_version() -> u32 {
        1
    }

    fn metadata() -> String {
        r#"{"id":"example.counter","name":"Counter","version":"1.0.0"}"#.to_string()
    }

    fn activate() -> Result<(), String> {
        host_log::log("example activated");
        host_state::set("activated", "true");
        Ok(())
    }

    fn deactivate() -> Result<(), String> {
        host_state::set("activated", "false");
        Ok(())
    }

    fn render() -> String {
        let count = host_state::get("count").unwrap_or_else(|| "0".to_string());
        format!(
            r#"{{"type":"form","title":"Counter","children":[{{"type":"text","text":"Count: {count}"}},{{"type":"button","label":"Increment","command":"inc"}},{{"type":"table","columns":["Key","Value"],"rows":[["count","{count}"]]}},{{"type":"list","items":["a","b"]}}]}}"#
        )
    }

    fn run_command(name: String, arg: String) -> Result<String, String> {
        match name.as_str() {
            "inc" => {
                let n: u64 = host_state::get("count")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                host_state::set("count", &(n + 1).to_string());
                Ok((n + 1).to_string())
            }
            "read" => host_fs::read_file(&arg),
            "net" => host_net::http_get(&arg),
            "spin" => {
                let mut i: u64 = 0;
                loop {
                    i = std::hint::black_box(i.wrapping_add(1));
                    if i == u64::MAX {
                        return Ok(String::new());
                    }
                }
            }
            "alloc" => {
                // Write state first so the test can prove the failed call left no trace.
                host_state::set("count", "999");
                let mb: usize = arg.parse().unwrap_or(64);
                let v = vec![1u8; mb * 1024 * 1024];
                Ok(std::hint::black_box(v).len().to_string())
            }
            "fail" => {
                host_state::set("count", "999");
                Err("requested failure".to_string())
            }
            _ => Err(format!("unknown command {name}")),
        }
    }
}

export!(Example);
