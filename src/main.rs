//
// Copyright (C) 2019 Kubos Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License")
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use getopts::Options;
use std::process::{exit, Command, Stdio};
use std::{env, fs};
use toml::Value;

const X86_TARGET_STR: &str = "x86-linux-native";

/// Take a kubos target and convert it
/// to a Rust/Clang target triplet
fn target_converter(kubos_target: &str) -> String {
    match kubos_target {
        X86_TARGET_STR => String::from("x86_64-unknown-linux-gnu"),
        "kubos-linux-beaglebone-gcc" => String::from("arm-unknown-linux-gnueabihf"),
        "kubos-linux-pumpkin-mbm2-gcc" => String::from("arm-unknown-linux-gnueabihf"),
        "kubos-linux-isis-gcc" => String::from("armv5te-unknown-linux-gnueabi"),
        _ => panic!(
            "Target '{}' not supported for cargo/yotta builds\
             \nCurrently supported targets are:\
             \nx86-linux-native\
             \nkubos-linux-beaglebone-gcc\
             \nkubos-linux-pumpkin-mbm2-gcc\
             \nkubos-linux-isis-gcc",
            kubos_target
        ),
    }
}

fn cargo_linker(target: &str) -> Result<String, String> {
    let cargo_home = env::var("CARGO_HOME").map_err(|e| format!("{}", e))?;
    let data =
        fs::read_to_string(format!("{}/config", cargo_home)).map_err(|e| format!("{}", e))?;
    let cfg = data.parse::<Value>().map_err(|e| format!("{}", e))?;
    let targets = cfg
        .get("target")
        .ok_or_else(|| String::from("no targets defined"))?;
    let target = targets
        .get(target)
        .ok_or_else(|| format!("target {} not defined", target))?;
    let linker = target
        .get("linker")
        .ok_or_else(|| String::from("no linker found"))?;

    linker
        .as_str()
        .ok_or_else(|| String::from("could not convert linker to string"))
        .map(String::from)
}

/// Perform `cargo 'command'` using the proper Rust/Clang target triplet
fn cargo_command(target: String, command: String, mut extra_params: Vec<String>) {
    let mut params = vec![command, String::from("--target"), target];
    params.append(&mut extra_params);

    let mut command = Command::new("cargo");
    if let Ok(linker) = cargo_linker(&params[2]) {
        command.env("CC", &linker);
        command.env("CXX", &linker);
        command.env("PKG_CONFIG_ALLOW_CROSS", "1");
    }

    let status = command
        .args(&params)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .unwrap();

    // Attempt to exit in a way which
    // honors the subprocess exit code
    if status.success() {
        exit(0)
    }
    exit(status.code().unwrap());
}

/// Displays usage message
fn print_usage(opts: Options) {
    let brief = "cargo-kubos is a helper utility for running \
        Cargo commands with a Kubos target attached.\nIt is \
        used when building/running/testing crates which either \
        contain a yotta module or depend on one. \
        \n\nUsage:\
        \n\tcargo kubos -c [cargo command] [options] -- [cargo options]
        \n\tcargo kubos -c build -t x86-linux-native -- -vv";
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();

    opts.reqopt("c", "command", "cargo command to run", "COMMAND");
    opts.optopt("t", "target", "sets (Kubos) target", "NAME");
    opts.optflag("h", "help", "Displays help");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("Error - {}\n", f);
            print_usage(opts);
            return;
        }
    };

    // Collect extra parameters
    let extra_params = if !matches.free.is_empty() {
        let mut params = matches.free.clone();
        // Remove extra kubos parameter
        params.retain(|x| x != "kubos");
        params
    } else {
        Vec::new()
    };

    if matches.opt_present("h") {
        print_usage(opts);
    } else {
        let k_target = match matches.opt_str("t") {
            Some(t) => t,
            None => String::from(X86_TARGET_STR),
        };
        let command = matches.opt_str("c").unwrap();
        let c_target = target_converter(&k_target);
        env::set_var("CARGO_KUBOS_TARGET", &k_target);
        cargo_command(c_target, command, extra_params);
    }
}
