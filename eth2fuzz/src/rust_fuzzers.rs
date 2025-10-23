use failure::{Error, ResultExt};
use std::env;
use std::ffi::OsStr;
use std::fs;
use crate::env::root_dir;
use std::path::PathBuf;
use std::process::Command;

use crate::env::{corpora_dir, state_dir};
use crate::fuzzers::{FuzzerConfig, FuzzerQuit};
use crate::targets::Targets;

static LANGUAGE: &str = "rust";

/***********************************************
name: honggfuzz-rs
github: https://github.com/rust-fuzz/honggfuzz-rs
***********************************************/

pub struct FuzzerHfuzz {
    /// Fuzzer name.
    pub name: String,
    /// Source code / template dir
    pub dir: PathBuf,
    /// Workspace dir
    pub work_dir: PathBuf,
    /// fuzzing config
    pub config: FuzzerConfig,
}

impl FuzzerHfuzz {
    /// Check if `cargo hfuzz` is installed
    pub fn is_available() -> Result<(), Error> {
        println!("[eth2fuzz] Testing FuzzerHfuzz is available");
        let fuzzer_output = Command::new("cargo").arg("hfuzz").arg("version").output()?;
        if !fuzzer_output.status.success() {
            bail!("hfuzz not available, install with `cargo install --force honggfuzz`");
        }
        Ok(())
    }

    /// Create a new FuzzerHfuzz
    pub fn new(config: FuzzerConfig) -> Result<FuzzerHfuzz, Error> {
        // Test if fuzzer engine installed
        FuzzerHfuzz::is_available()?;

        let cwd = env::current_dir().context("error getting current directory")?;
        let fuzzer = FuzzerHfuzz {
            name: "Honggfuzz".to_string(),
            dir: cwd.join("fuzzers").join("rust-honggfuzz"),
            work_dir: cwd.join("workspace").join("hfuzz"),
            config,
        };
        Ok(fuzzer)
    }

    pub fn run(&self, target: Targets) -> Result<(), Error> {
        // check if target is supported by this fuzzer
        if target.language() != LANGUAGE {
            bail!(format!("{} incompatible for this target", self.name));
        }

        // get path to corpora, allow override for RL binning
        let default_corpora_dir = corpora_dir()?.join(target.corpora());
        let corpora_override = env::var("ETH2FUZZ_CORPORA_OVERRIDE").ok();
        let corpora_dir = corpora_override
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .unwrap_or(default_corpora_dir);

        // copy targets folder into workspace
        // prepare_targets_workspace()?;

        // create hfuzz folder inside workspace/
        // self.prepare_fuzzer_workspace()?;

        // write all fuzz targets inside hfuzz folder
        // write_fuzzer_target(&self.dir, &self.work_dir, target)?;
        println!("[eth2fuzz] {}: {} created", self.name, target.name());

        // Remove stale Cargo.lock to avoid pinning old lighthouse git revisions
        let lock_path = self.work_dir.join("Cargo.lock");
        if lock_path.exists() {
            let _ = fs::remove_file(&lock_path);
        }

        // sanitizers
        let rust_args = format!(
            "{} \
            {}",
            if let Some(san) = self.config.sanitizer {
                format!("-Z sanitizer={}", san.name())
            } else {
                "".into()
            },
            env::var("RUSTFLAGS").unwrap_or_default()
        );

        // Handle seed option
        if self.config.seed != None {
            println!("[eth2fuzz] {}: seed not supported", self.name);
        }

        // prepare arguments
        let hfuzz_args = format!(
            "{} \
             {} \
             {} \
             {} \
             {}",
            if let Some(t) = self.config.timeout {
                format!("--run_time {}", t)
            } else {
                "".into()
            },
            "-t 60",
            // Use SIGVTALRM to kill timeouting processes
            "--tmout_sigvtalrm",
            // Set number of thread
            if let Some(n) = self.config.thread {
                format!("--threads {}", n)
            } else {
                "".into()
            },
            env::var("HFUZZ_RUN_ARGS").unwrap_or_default()
        );

        // Determine logs directory by tag/mode
        let cwd = env::current_dir().context("error getting current directory")?;
        let tag = env::var("ETH2FUZZ_TAG").unwrap_or_else(|_| "default".into());
        let mode = env::var("ETH2FUZZ_RUN_MODE").unwrap_or_else(|_| "base".into());
        let logs_dir = cwd
            .join("workspace")
            .join("logs")
            .join(tag)
            .join(mode)
            .join("hfuzz")
            .join("logs");
        fs::create_dir_all(&logs_dir).ok();
        let log_file = logs_dir.join(format!("{}.log", target.name()));

        // Build a shell command to run cargo hfuzz and tee output to log
        let cmd = format!(
            "export RUSTFLAGS=\"{}\"; \
             export HFUZZ_RUN_ARGS=\"{}\"; \
             export HFUZZ_INPUT=\"{}\"; \
             export ETH2FUZZ_BEACONSTATE=\"{}\"; \
             cargo +nightly hfuzz run {} 2>&1 | tee -a {}",
            rust_args,
            hfuzz_args,
            corpora_dir.display(),
            state_dir()?.display(),
            &target.name(),
            log_file.display()
        );

        let fuzzer_bin = Command::new("/bin/sh")
            .arg("-lc")
            .arg(cmd)
            .current_dir(&self.work_dir)
            .spawn()
            .context(format!(
                "error starting {} to run {}",
                self.name,
                target.name()
            ))?
            .wait()
            .context(format!(
                "error while waiting for {} running {}",
                self.name,
                target.name()
            ))?;

        if !fuzzer_bin.success() {
            return Err(FuzzerQuit.into());
        }
        Ok(())
    }
}

/***********************************************
name: afl-rs
github: https://github.com/rust-fuzz/afl.rs
***********************************************/

pub struct FuzzerAfl {
    /// Fuzzer name.
    pub name: String,
    /// Source code / template dir
    pub dir: PathBuf,
    /// Workspace dir
    pub work_dir: PathBuf,
    /// fuzzing config
    pub config: FuzzerConfig,
}

impl FuzzerAfl {
    /// Check if `cargo afl` is installed
    pub fn is_available() -> Result<(), Error> {
        println!("[eth2fuzz] Testing FuzzerAfl is available");
        let fuzzer_output = Command::new("cargo").arg("afl").arg("--version").output()?;
        if !fuzzer_output.status.success() {
            bail!("afl-rs not available, install with `cargo install --force afl`");
        }
        Ok(())
    }

    /// Create a new FuzzerAfl
    pub fn new(config: FuzzerConfig) -> Result<FuzzerAfl, Error> {
        // Test if fuzzer engine installed
        FuzzerAfl::is_available()?;

        let cwd = env::current_dir().context("error getting current directory")?;
        let fuzzer = FuzzerAfl {
            name: "Afl++".to_string(),
            dir: cwd.join("fuzzers").join("rust-afl"),
            work_dir: cwd.join("workspace").join("afl"),
            config,
        };
        Ok(fuzzer)
    }

    /// Build single target with afl
    pub fn build_afl(&self, target: Targets) -> Result<(), Error> {
        // prepare_targets_workspace()?;
        // create afl folder inside workspace/
        // self.prepare_fuzzer_workspace()?;

        // write_fuzzer_target(&self.dir, &self.work_dir, target)?;

        // sanitizers
        let rust_args = format!(
            "{} \
            {}",
            if let Some(san) = self.config.sanitizer {
                format!("-Z sanitizer={}", san.name())
            } else {
                "".into()
            },
            env::var("RUSTFLAGS").unwrap_or_default()
        );

        let build_cmd = Command::new("cargo")
            .args(&["+nightly", "afl", "build", "--bin", &target.name()]) // TODO: not sure we want to compile afl in "--release"
            .env("RUSTFLAGS", &rust_args)
            .current_dir(&self.work_dir)
            .spawn()
            .context(format!(
                "error starting build for {} of {}",
                self.name,
                target.name()
            ))?
            .wait()
            .context(format!(
                "error while waiting for build for {} of {}",
                self.name,
                target.name()
            ))?;

        if !build_cmd.success() {
            return Err(FuzzerQuit.into());
        }

        Ok(())
    }

    pub fn run(&self, target: Targets) -> Result<(), Error> {
        // check if target is supported by this fuzzer
        if target.language() != LANGUAGE {
            bail!(format!("{} incompatible for this target", self.name));
        }

        let dir = &self.work_dir;
        let corpora_dir = corpora_dir()?.join(target.corpora());

        self.build_afl(target)?;

        // TODO - modify to use same corpus than other fuzzer
        // let corpus_dir = &self.workspace_dir;
        let corpus_dir = env::current_dir()?
            .join("workspace")
            .join("afl")
            .join("afl_workspace");
        fs::create_dir_all(&corpus_dir)
            .context(format!("unable to create {} dir", corpus_dir.display()))?;

        // Determined if existing fuzzing session exist
        let queue_dir = corpus_dir.join("queue");
        let input_arg: &OsStr = if queue_dir.is_dir() && fs::read_dir(queue_dir)?.next().is_some() {
            "-".as_ref()
        } else {
            corpora_dir.as_ref()
        };

        let mut args: Vec<String> = Vec::new();
        args.push("+nightly".to_string());
        args.push("afl".to_string());
        args.push("fuzz".to_string());
        if let Some(t) = self.config.timeout {
            args.push(format!("-V {}", t));
        };
        if let Some(seed) = self.config.seed {
            args.push(format!("-s {}", seed));
        };

        // Run the fuzzer using cargo
        let fuzzer_bin = Command::new("cargo")
            .args(args)
            //.arg("-t 30000+" ) // increase timeout to let the fuzzer pick a valid beaconstate
            .arg("-m") // remove memory limit
            .arg("none")
            .arg("-i")
            .arg(&input_arg)
            .arg("-o")
            .arg(&corpus_dir)
            .args(&["--", &format!("./target/debug/{}", target.name())])
            .env(
                "ETH2FUZZ_BEACONSTATE",
                format!("{}", state_dir()?.display()),
            )
            // env variable to skip afl checking
            .env("AFL_SKIP_CPUFREQ", "1")
            .env("AFL_SKIP_CRASHES", "1")
            .env("AFL_I_DONT_CARE_ABOUT_MISSING_CRASHES", "1")
            .current_dir(&dir)
            .spawn()
            .context(format!(
                "error starting {:?} to run {}",
                self.name,
                target.name()
            ))?
            .wait()
            .context(format!(
                "error while waiting for {:?} running {}",
                self.name,
                target.name()
            ))?;

        if !fuzzer_bin.success() {
            return Err(FuzzerQuit.into());
        }
        Ok(())
    }
}

/***********************************************
name: libfuzzer/cargo-fuzz
github: https://github.com/rust-fuzz/cargo-fuzz
***********************************************/

pub struct FuzzerLibfuzzer {
    /// Fuzzer name.
    pub name: String,
    /// Source code / template dir
    pub dir: PathBuf,
    /// Workspace dir
    pub work_dir: PathBuf,
    /// fuzzing config
    pub config: FuzzerConfig,
}

impl FuzzerLibfuzzer {
    /// Check if `cargo fuzz` is installed
    pub fn is_available() -> Result<(), Error> {
        println!("[eth2fuzz] Testing FuzzerLibfuzzer is available");
        let fuzzer_output = Command::new("cargo")
            .arg("fuzz")
            .arg("--version")
            .output()?;
        if !fuzzer_output.status.success() {
            bail!("cargo-fuzz not available, install with `cargo install --force cargo-fuzz`");
        }
        Ok(())
    }

    /// Create a new FuzzerLibfuzzer
    pub fn new(config: FuzzerConfig) -> Result<FuzzerLibfuzzer, Error> {
        // Test if fuzzer engine installed
        FuzzerLibfuzzer::is_available()?;

        let cwd = env::current_dir().context("error getting current directory")?;
        let fuzzer = FuzzerLibfuzzer {
            name: "Libfuzzer".to_string(),
            dir: cwd.join("fuzzers").join("rust-libfuzzer"),
            work_dir: cwd.join("workspace").join("libfuzzer"),
            config,
        };
        Ok(fuzzer)
    }

    pub fn run(&self, target: Targets) -> Result<(), Error> {
        // check if target is supported by this fuzzer
        if target.language() != LANGUAGE {
            bail!(format!("{} incompatible for this target", self.name));
        }

        // prepare_targets_workspace()?;
        // create afl folder inside workspace/
        // self.prepare_fuzzer_workspace()?;

        /*
                let fuzz_dir = self.work_dir.join("fuzz");
                fs::create_dir_all(&fuzz_dir)
                    .context(format!("unable to create {} dir", fuzz_dir.display()))?;

                let target_dir = fuzz_dir.join("fuzz_targets");

                let _ = fs::remove_dir_all(&target_dir)
                    .context(format!("error removing {}", target_dir.display()));
                fs::create_dir_all(&target_dir)
                    .context(format!("unable to create {} dir", target_dir.display()))?;

                fs::create_dir_all(&fuzz_dir)
                    .context(format!("unable to create {} dir", fuzz_dir.display()))?;
                //println!("{:?}", fuzz_dir);

                fs::copy(
                    self.dir.join("fuzz").join("Cargo.toml"),
                    fuzz_dir.join("Cargo.toml"),
                )?;

                // Add all targets to libfuzzer
                for target in Targets::iter().filter(|x| x.language() == "rust") {
                    write_libfuzzer_target(&self.work_dir, target)?;
                }
        */
        let fuzz_dir = self.work_dir.join("fuzz");
        // Determine corpus dir; allow override for experiments (parity with Honggfuzz)
        let default_corpora_dir = corpora_dir()?.join(target.corpora());
        let corpora_override = env::var("ETH2FUZZ_CORPORA_OVERRIDE").ok();
        let corpus_dir = corpora_override
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .unwrap_or(default_corpora_dir);

        // sanitizers
        let rust_args = format!(
            "{} \
            {}",
            if let Some(san) = self.config.sanitizer {
                format!("-Z sanitizer={}", san.name())
            } else {
                "".into()
            },
            env::var("RUSTFLAGS").unwrap_or_default()
        );

        // create arguments
        // corpora dir
        // max_time if provided (i.e. continuously fuzzing)
        let mut args: Vec<String> = Vec::new();
        args.push(format!("{}", &corpus_dir.display()));
        if let Some(timeout) = self.config.timeout {
            args.push("--".to_string());
            args.push(format!("-max_total_time={}", timeout));
        };
        // threading
        if let Some(thread) = self.config.thread {
            args.push(format!("-workers={}", thread));
            args.push(format!("-jobs={}", thread));
        };
        // handle seed option
        if let Some(seed) = self.config.seed {
            args.push(format!("-seed={}", seed));
        };
        // Determine log location from mode/tag env (match Honggfuzz pattern for tooling compatibility)
        let mode = std::env::var("ETH2FUZZ_RUN_MODE").unwrap_or_else(|_| "base".to_string());
        let tag = std::env::var("ETH2FUZZ_TAG").unwrap_or_else(|_| "default".to_string());
        let proj_root = root_dir().unwrap_or(env::current_dir().unwrap_or(std::path::PathBuf::from(".")));
        let logs_dir = proj_root
            .join("workspace")
            .join("logs")
            .join(tag)
            .join(mode)
            .join("libfuzzer")
            .join("logs");
        fs::create_dir_all(&logs_dir).ok();
        let log_file = logs_dir.join(format!("{}.log", target.name()));

        // Launch libFuzzer and tee output to per-target log for downstream RL parsing
        let cmd = format!(
            "export ETH2FUZZ_BEACONSTATE=\"{}\"; \
             export RUSTFLAGS=\"{}\"; \
             cargo +nightly fuzz run {} {} 2>&1 | tee -a {}",
            state_dir()?.display(),
            rust_args,
            &target.name(),
            args.join(" "),
            log_file.display()
        );

        let status = Command::new("/bin/sh")
            .arg("-lc")
            .arg(cmd)
            .current_dir(&fuzz_dir)
            .spawn()
            .context(format!(
                "error starting {:?} to run {}",
                self.name,
                target.name()
            ))?
            .wait()
            .context(format!(
                "error while waiting for {:?} running {}",
                self.name,
                target.name()
            ))?;

        if !status.success() {
            return Err(FuzzerQuit.into());
        }
        Ok(())
    }
}
