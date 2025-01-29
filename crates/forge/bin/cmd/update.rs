use alloy_primitives::map::HashMap;
use clap::{Parser, ValueHint};
use eyre::{Context, Result};
use forge::FOUNDRY_LOCK;
use foundry_cli::{
    opts::Dependency,
    utils::{Git, LoadConfig, Submodules, TagType},
};
use foundry_common::fs;
use foundry_config::{impl_figment_convert_basic, Config};
use std::path::PathBuf;

/// CLI arguments for `forge update`.
#[derive(Clone, Debug, Parser)]
pub struct UpdateArgs {
    /// The dependencies you want to update.
    dependencies: Vec<Dependency>,

    /// The project's root path.
    ///
    /// By default root of the Git repository, if in one,
    /// or the current working directory.
    #[arg(long, value_hint = ValueHint::DirPath, value_name = "PATH")]
    root: Option<PathBuf>,

    /// Override the up-to-date check.
    #[arg(short, long)]
    force: bool,

    /// Recursively update submodules.
    #[arg(short, long)]
    recursive: bool,
}
impl_figment_convert_basic!(UpdateArgs);

impl UpdateArgs {
    pub fn run(self) -> Result<()> {
        let config = self.load_config()?;
        // dep_overrides consists of absolute paths of dependencies and their tags
        let (root, paths, dep_overrides) = dependencies_paths(&self.dependencies, &config)?;
        // Mapping of relative path of lib to its tag type
        // e.g "lib/forge-std" -> TagType::Tag("v0.1.0")
        let git = Git::new(&root);
        let foundry_lock_path = root.join(FOUNDRY_LOCK);
        let (mut foundry_lock, out_of_sync) =
            crate::cmd::install::read_or_generate_foundry_lock(&foundry_lock_path, Some(&git))?;
        if out_of_sync {
            fs::write_json_file(&foundry_lock_path, &foundry_lock)?;
        }

        let prev_len = foundry_lock.len();

        // Mapping of relative path of dependency to its override tag
        let mut overrides: HashMap<PathBuf, TagType> = HashMap::default();
        // update the submodules' tags if any overrides are present
        for (dep_path, override_tag) in &dep_overrides {
            let rel_path = dep_path
                .strip_prefix(&root)
                .wrap_err("Dependency path is not relative to the repository root")?;
            if let Ok(tag_type) = TagType::resolve_type(&git, dep_path, override_tag) {
                foundry_lock.insert(rel_path.to_path_buf(), tag_type.clone());
                overrides.insert(rel_path.to_path_buf(), tag_type);
            } else {
                sh_warn!(
                    "Could not override submodule at {} with tag {}, try using forge install",
                    rel_path.display(),
                    override_tag
                )?;
            }
        }

        // fetch the latest changes for each submodule (recursively if flag is set)
        let git = Git::new(&root);
        let submodules = git.submodules()?;
        if self.recursive {
            // update submodules recursively
            let update_paths =
                self.update_paths(&paths, &submodules, &foundry_lock, &dep_overrides);
            if let Some(update_paths) = update_paths {
                git.submodule_update(self.force, true, false, true, update_paths)?;
            } else {
                git.submodule_update(self.force, true, false, true, Vec::<PathBuf>::new())?;
            }
        } else {
            let update_paths =
                self.update_paths(&paths, &submodules, &foundry_lock, &dep_overrides);
            if let Some(update_paths) = update_paths {
                // update root submodules
                git.submodule_update(self.force, true, false, false, update_paths)?;
            } else {
                // update all submodules
                git.submodule_update(self.force, true, false, false, Vec::<PathBuf>::new())?;
                // initialize submodules of each submodule recursively (otherwise direct submodule
                // dependencies will revert to last commit)
                git.submodule_foreach(false, "git submodule update --init --progress --recursive")?;
            }
        }

        // checkout the submodules at the correct tags
        for (path, tag) in &foundry_lock {
            git.checkout_at(tag.raw_string(), &root.join(path))?;
        }

        if prev_len != foundry_lock.len() || !overrides.is_empty() {
            fs::write_json_file(&foundry_lock_path, &foundry_lock)?;
        }

        Ok(())
    }

    /// Gets the relatives paths to the submodules that need to be updated.
    /// If None, it means all submodules need to be updated.
    fn update_paths(
        &self,
        paths: &[PathBuf],
        submodules: &Submodules,
        foundry_lock: &HashMap<PathBuf, TagType>,
        overrides: &HashMap<PathBuf, String>,
    ) -> Option<Vec<PathBuf>> {
        let paths_to_avoid = foundry_lock
            .iter()
            .filter_map(|(path, tag_type)| {
                // Don't update submodules that are pinned to a release tag / rev unless a override
                // has been specified.
                if let TagType::Tag(_) | TagType::Rev(_) = tag_type {
                    if !overrides.contains_key(path) {
                        return Some(path.clone());
                    }
                }
                None
            })
            .collect::<Vec<_>>();

        match (paths.is_empty(), paths_to_avoid.is_empty()) {
            (true, true) => {
                // running `forge update`
                None
            }
            (true, false) => {
                // running `forge update`
                Some(
                    submodules
                        .into_iter()
                        .filter_map(|s| {
                            if !paths_to_avoid.contains(s.path()) {
                                return Some(s.path().to_path_buf());
                            }
                            None
                        })
                        .collect(),
                )
            }
            (false, true) => {
                // running `forge update <deps>`
                Some(paths.to_vec())
            }
            (false, false) => {
                // running `forge update <deps>`
                Some(paths.iter().filter(|path| !paths_to_avoid.contains(path)).cloned().collect())
            }
        }
    }
}

/// Returns `(root, paths, overridden_deps_with_abosolute_paths)` where `root` is the root of the
/// Git repository and `paths` are the relative paths of the dependencies.
#[allow(clippy::type_complexity)]
pub fn dependencies_paths(
    deps: &[Dependency],
    config: &Config,
) -> Result<(PathBuf, Vec<PathBuf>, HashMap<PathBuf, String>)> {
    let git_root = Git::root_of(&config.root)?;
    let libs = config.install_lib_dir();

    let mut paths = Vec::with_capacity(deps.len());
    let mut overrides = HashMap::with_capacity_and_hasher(deps.len(), Default::default());
    for dep in deps {
        let name = dep.name();
        let dep_path = libs.join(name);
        let rel_path = dep_path
            .strip_prefix(&git_root)
            .wrap_err("Library directory is not relative to the repository root")?;
        if !dep_path.exists() {
            eyre::bail!("Could not find dependency {name:?} in {}", dep_path.display());
        }

        if let Some(tag) = &dep.tag {
            overrides.insert(dep_path.to_owned(), tag.to_owned());
        }
        paths.push(rel_path.to_owned());
    }
    Ok((git_root, paths, overrides))
}
