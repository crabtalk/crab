use crate::{TARGET, source};
use anyhow::{Context, Result, bail, ensure};
use std::{
    env,
    ffi::OsString,
    fs, io, iter,
    path::{self, Path, PathBuf},
    process::{Command, Stdio},
};

#[cfg(unix)]
const INSTALLER: &str = "https://sh.rustup.rs";
#[cfg(windows)]
const INSTALLER: &str =
    "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe";

pub(crate) struct Toolchain {
    home: PathBuf,
    path: OsString,
}

impl Toolchain {
    pub fn new(dir: &Path) -> Result<Self> {
        let home = match env::var_os("CARGO_HOME").filter(|home| !home.is_empty()) {
            Some(home) => PathBuf::from(home),
            None => env::home_dir()
                .context("cannot find the home directory")?
                .join(".cargo"),
        };
        let home = path::absolute(home).context("cannot resolve the build tools directory")?;
        let bin = home.join("bin");
        let mut paths: Vec<_> =
            env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
        paths.push(bin.clone());
        let mut toolchain = Self {
            home: home.clone(),
            path: env::join_paths(paths).context("cannot prepare the build PATH")?,
        };
        if toolchain.installed(dir).is_ok() {
            return Ok(toolchain);
        }
        let _lock = toolchain.lock()?;
        if toolchain.installed(dir).is_err() {
            if toolchain.has_rustup(dir) {
                if !toolchain
                    .command("rustup", dir)
                    .arg("default")
                    .output()?
                    .status
                    .success()
                {
                    eprintln!("Installing build tools…");
                    let status = toolchain
                        .command("rustup", dir)
                        .args(["toolchain", "install", "stable", "--profile", "minimal"])
                        .stdout(io::stderr())
                        .status()?;
                    ensure!(status.success(), "could not install the build toolchain");
                    let status = toolchain
                        .command("rustup", dir)
                        .args(["default", "stable"])
                        .stdout(io::stderr())
                        .status()?;
                    ensure!(status.success(), "could not install the build toolchain");
                }
            } else {
                toolchain.install(dir)?;
            }
            toolchain.prefer_installed()?;
        }
        toolchain
            .installed(dir)
            .context("could not prepare the build tools")?;
        Ok(toolchain)
    }

    pub fn command(&self, tool: &str, dir: &Path) -> Command {
        let program = match tool {
            "cargo" => env::var_os("CARGO"),
            "rustc" => env::var_os("RUSTC"),
            _ => None,
        }
        .unwrap_or_else(|| tool.into());
        let mut command = Command::new(program);
        command
            .env("PATH", &self.path)
            .env("CARGO_HOME", &self.home)
            .current_dir(dir);
        command
    }

    fn installed(&self, dir: &Path) -> Result<()> {
        for tool in ["cargo", "rustc"] {
            let output = self
                .command(tool, dir)
                .arg("--version")
                .output()
                .with_context(|| format!("cannot run {tool}"))?;
            ensure!(
                output.status.success(),
                "{tool}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    fn has_rustup(&self, dir: &Path) -> bool {
        self.command("rustup", dir)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn lock(&self) -> Result<fs::File> {
        fs::create_dir_all(&self.home).context("cannot create the build tools directory")?;
        let lock = fs::File::create(self.home.join("crab-build.lock"))?;
        lock.lock()?;
        Ok(lock)
    }

    fn prefer_installed(&mut self) -> Result<()> {
        self.path =
            env::join_paths(iter::once(self.home.join("bin")).chain(env::split_paths(&self.path)))
                .context("cannot prepare the installed build tools PATH")?;
        Ok(())
    }

    fn install(&self, dir: &Path) -> Result<()> {
        eprintln!("Installing build tools…");
        let temporary = tempfile::tempdir().context("cannot create the installer directory")?;
        #[cfg(unix)]
        let installer = temporary.path().join("rustup-init.sh");
        #[cfg(windows)]
        let installer = temporary.path().join("rustup-init.exe");
        source::download(INSTALLER, &installer)
            .context("cannot download the build tools installer")?;
        #[cfg(unix)]
        let mut command = self.command("sh", dir);
        #[cfg(unix)]
        command.arg(&installer);
        #[cfg(windows)]
        let mut command = Command::new(&installer);
        let status = command
            .args([
                "-y",
                "--profile",
                "minimal",
                "--default-toolchain",
                "stable",
                "--no-modify-path",
            ])
            .current_dir(dir)
            .env("CARGO_HOME", &self.home)
            .env("PATH", &self.path)
            .stdin(Stdio::null())
            .stdout(io::stderr())
            .status()
            .context("cannot start the build tools installer")?;
        ensure!(status.success(), "could not install the build tools");
        Ok(())
    }

    /// A project's rust-toolchain.toml selects which toolchain gets the target.
    pub fn target(&mut self, dir: &Path) -> Result<()> {
        let output = self
            .command("rustc", dir)
            .args(["--print", "sysroot"])
            .output()
            .context("cannot run the build compiler")?;
        ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let sysroot =
            String::from_utf8(output.stdout).context("the compiler printed a non-UTF-8 sysroot")?;
        if Path::new(sysroot.trim())
            .join("lib/rustlib")
            .join(TARGET)
            .is_dir()
        {
            return Ok(());
        }
        if !self.has_rustup(dir) {
            let _lock = self.lock()?;
            if !self.has_rustup(dir) {
                self.install(dir)?;
            }
        }
        self.prefer_installed()?;
        eprintln!("Installing the build target…");
        let status = self
            .command("rustup", dir)
            .args(["target", "add", TARGET])
            .stdout(io::stderr())
            .status()
            .context("cannot install the build target")?;
        if !status.success() {
            bail!("could not install the build target {TARGET}");
        }
        Ok(())
    }
}
