//! Rebuilding and redeploying the workspace's services as its files change.

use crate::{
    dev::{Dev, Event},
    project, utils,
};
use anyhow::{Context, Result, bail};
use berm::syscall::call;
use berm_build::Workspace;
use notify::{Event as Changed, RecursiveMode, Watcher};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

/// How long the files stay unchanged before a rebuild: an editor's save, or a
/// checkout, is many events.
const QUIET: Duration = Duration::from_millis(300);

impl Dev {
    /// Build and deploy every service of `workspace`, then again on each
    /// change under its root outside its target directory and `.git`, until
    /// `stopped` is set. Returns early only when the watch cannot go on.
    pub(crate) fn watch(
        self: Arc<Self>,
        workspace: Workspace,
        stopped: Arc<AtomicBool>,
    ) -> Result<()> {
        let (send, changes) = mpsc::channel::<notify::Result<Changed>>();
        let mut watcher = notify::recommended_watcher(send).context("cannot watch files")?;
        watcher
            .watch(&workspace.root, RecursiveMode::Recursive)
            .with_context(|| format!("cannot watch {}", workspace.root.display()))?;

        // The digest each service last deployed, so a rebuild that changed
        // nothing of it is not redeployed.
        let mut deployed = BTreeMap::new();
        loop {
            for (index, dir) in workspace.services.iter().enumerate() {
                if stopped.load(Ordering::Relaxed) {
                    return Ok(());
                }
                self.rebuild(&workspace.root, index, dir, &mut deployed);
            }
            loop {
                if stopped.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let changed = match changes.recv_timeout(QUIET) {
                    Ok(changed) => changed?,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => bail!("the file watcher stopped"),
                };
                if changed.paths.iter().any(|path| {
                    !path.starts_with(&workspace.target)
                        && !path.components().any(|part| part.as_os_str() == ".git")
                }) {
                    break;
                }
            }
            while changes.recv_timeout(QUIET).is_ok() {}
        }
    }

    /// Build the service in `dir`, the `index`th of the workspace at `root`,
    /// and deploy it unless `deployed` holds its digest already, recording
    /// either outcome.
    fn rebuild(
        &self,
        root: &Path,
        index: usize,
        dir: &Path,
        deployed: &mut BTreeMap<usize, String>,
    ) {
        let Some(name) = self
            .status
            .lock()
            .watched(root, index)
            .map(|one| one.name.clone())
        else {
            return;
        };
        let result = (|| -> Result<_> {
            let project = project::read(dir)?;
            let image = berm_build::build(dir, None, project::FILE)?;
            let bytes =
                fs::read(&image).with_context(|| format!("cannot read {}", image.display()))?;
            let digest = berm::digest(&bytes);
            if deployed.get(&index) == Some(&digest) {
                return Ok(None);
            }
            let (program, breaks) = utils::wait(self.service.deploy(
                &name,
                bytes,
                &project.public,
                true,
                project.crons.clone(),
                project.database.size(),
            ))?;
            deployed.insert(index, digest);
            Ok(Some((program.manifest().version.clone(), breaks)))
        })();

        let now = call::system_now();
        let mut status = self.status.lock();
        let Some(one) = status.watched(root, index) else {
            return;
        };
        one.rebuilt = Some(now);
        let event = match result {
            Ok(None) => {
                one.error = None;
                return;
            }
            Ok(Some((version, breaks))) => {
                one.error = None;
                one.version = Some(version.clone());
                one.deployed = Some(now);
                Event::Deployed {
                    workspace: root.to_owned(),
                    service: name,
                    version,
                    breaks: breaks.iter().map(ToString::to_string).collect(),
                }
            }
            Err(error) => {
                let error = format!("{error:#}");
                one.error = Some(error.clone());
                Event::Failed {
                    workspace: root.to_owned(),
                    service: name,
                    error,
                }
            }
        };
        drop(status);
        self.emit(&event);
    }
}
