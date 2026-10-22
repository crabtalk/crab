//! Rebuilding and redeploying the workspace's services as its files change.

use crate::{
    dev::{Dev, Event},
    utils,
};
use anyhow::{Context, Result};
use berm::syscall::call;
use berm_api::project::Project;
use berm_build::Workspace;
use notify::{Event as Changed, RecursiveMode, Watcher};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Arc, mpsc},
    time::Duration,
};

/// How long the files stay unchanged before a rebuild: an editor's save, or a
/// checkout, is many events.
const QUIET: Duration = Duration::from_millis(300);

impl Dev {
    /// Build and deploy every service, then again on each change under the
    /// workspace's root outside its target directory and `.git`. Returns only when the
    /// watch cannot go on.
    pub fn watch(self: Arc<Self>, workspace: Workspace) -> Result<()> {
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
                self.rebuild(index, dir, &mut deployed);
            }
            loop {
                let changed = changes.recv().context("the file watcher stopped")??;
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

    /// Build the service in `dir`, the `index`th, and deploy it unless
    /// `deployed` holds its digest already, recording either outcome.
    fn rebuild(&self, index: usize, dir: &Path, deployed: &mut BTreeMap<usize, String>) {
        let name = self.status.lock().services[index].name.clone();
        let result = (|| -> Result<_> {
            let project = Project::read(dir)?;
            let image = berm_build::build(dir, None)?;
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
        let watched = &mut status.services[index];
        watched.rebuilt = Some(now);
        match result {
            Ok(None) => watched.error = None,
            Ok(Some((version, breaks))) => {
                watched.error = None;
                watched.version = Some(version.clone());
                watched.deployed = Some(now);
                drop(status);
                self.emit(&Event::Deployed {
                    service: &name,
                    version: &version,
                    breaks: breaks.iter().map(ToString::to_string).collect(),
                });
            }
            Err(error) => {
                let error = format!("{error:#}");
                watched.error = Some(error.clone());
                drop(status);
                self.emit(&Event::Failed {
                    service: &name,
                    error: &error,
                });
            }
        }
    }
}
