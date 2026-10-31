//! The catalog's list, `catalog.toml`: its categories and the platforms in
//! each. A tool's `catalog` job starts with a listed platform.

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor},
};
use std::{collections::BTreeMap, fmt, str::FromStr, sync::LazyLock};

/// The list as the SDK was built with it.
const LIST: &str = include_str!("../catalog.toml");

static LISTED: LazyLock<Catalog> =
    LazyLock::new(|| LIST.parse().unwrap_or_else(|e| panic!("catalog.toml: {e}")));

/// The categories, in the order the list writes them.
#[derive(Debug, Clone)]
pub struct Catalog {
    pub categories: Vec<Category>,
}

/// One category and its platforms.
#[derive(Debug, Clone)]
pub struct Category {
    /// Its table's key, as `social`.
    pub id: String,
    /// As `Social`.
    pub title: String,
    pub platforms: Vec<String>,
}

/// A category of `GET /catalog`: its platforms, each with its jobs.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryJobs {
    pub id: String,
    pub title: String,
    pub platforms: Vec<PlatformJobs>,
}

/// A platform and the jobs its services' tools do, most called first.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformJobs {
    pub id: String,
    pub jobs: Vec<Job>,
}

/// One job: every tool whose `catalog` names it, most called first.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// The full catalog id, as `company.enrich`.
    pub id: String,
    /// The most called provider's title, or its tool's name without one.
    pub title: String,
    pub providers: Vec<Provider>,
}

/// A service's tool that does a job.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// `owner/name`, the owner by its handle, or its id when it has none.
    pub service: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    /// What a call costs the caller, in USD micros; absent when it is free.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<u64>,
    /// Its calls over the last 30 days.
    pub calls: u64,
}

/// One category's table in the list.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    title: String,
    platforms: Vec<String>,
}

impl Catalog {
    /// The list compiled into the SDK.
    pub fn listed() -> &'static Catalog {
        &LISTED
    }

    /// The category `platform` is listed in.
    pub fn category(&self, platform: &str) -> Option<&Category> {
        self.categories
            .iter()
            .find(|category| category.platforms.iter().any(|p| p == platform))
    }

    /// Every category with its platforms, each holding the jobs of
    /// `providers`, by job id. A provider whose job starts with no listed
    /// platform is left out.
    pub fn jobs(&self, providers: Vec<(String, Provider)>) -> Vec<CategoryJobs> {
        let mut jobs: BTreeMap<String, Vec<Provider>> = BTreeMap::new();
        for (job, provider) in providers {
            jobs.entry(job).or_default().push(provider);
        }
        let mut platforms: BTreeMap<&str, Vec<Job>> = BTreeMap::new();
        for (id, mut providers) in jobs {
            let Some((platform, _)) = id.split_once('.') else {
                continue;
            };
            let Some(platform) = self
                .categories
                .iter()
                .flat_map(|category| &category.platforms)
                .find(|listed| *listed == platform)
            else {
                continue;
            };
            providers.sort_by(|a, b| {
                b.calls
                    .cmp(&a.calls)
                    .then_with(|| a.service.cmp(&b.service))
            });
            let title = providers[0]
                .title
                .clone()
                .unwrap_or_else(|| providers[0].tool.clone());
            platforms.entry(platform).or_default().push(Job {
                id,
                title,
                providers,
            });
        }
        let calls = |job: &Job| job.providers.iter().map(|p| p.calls).sum::<u64>();
        self.categories
            .iter()
            .map(|category| CategoryJobs {
                id: category.id.clone(),
                title: category.title.clone(),
                platforms: category
                    .platforms
                    .iter()
                    .map(|platform| {
                        let mut jobs = platforms.remove(platform.as_str()).unwrap_or_default();
                        jobs.sort_by(|a, b| calls(b).cmp(&calls(a)).then_with(|| a.id.cmp(&b.id)));
                        PlatformJobs {
                            id: platform.clone(),
                            jobs,
                        }
                    })
                    .collect(),
            })
            .collect()
    }

    /// The category of the platform `job` starts with, refused when `job` is
    /// not a platform, `.`, then the job, or the platform is not listed.
    pub fn check(&self, job: &str) -> Result<&Category, String> {
        let Some((platform, _)) = job
            .split_once('.')
            .filter(|(platform, rest)| !platform.is_empty() && !rest.is_empty())
        else {
            return Err(format!(
                "{job:?} is not a platform, `.`, then the job, as \"company.enrich\""
            ));
        };
        self.category(platform).ok_or_else(|| {
            format!("{job:?} starts with {platform:?}, which the catalog does not list")
        })
    }
}

/// The list as TOML, refused when a platform is listed twice.
impl FromStr for Catalog {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let catalog: Catalog = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut seen: Vec<&str> = Vec::new();
        for platform in catalog.categories.iter().flat_map(|c| &c.platforms) {
            if seen.contains(&platform.as_str()) {
                return Err(format!("the platform {platform} is listed twice"));
            }
            seen.push(platform);
        }
        Ok(catalog)
    }
}

/// A table per category, kept in the order written.
impl<'de> Deserialize<'de> for Catalog {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Tables;

        impl<'de> Visitor<'de> for Tables {
            type Value = Catalog;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a table per category")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Catalog, A::Error> {
                let mut categories = Vec::new();
                while let Some((id, Table { title, platforms })) =
                    map.next_entry::<String, Table>()?
                {
                    categories.push(Category {
                        id,
                        title,
                        platforms,
                    });
                }
                Ok(Catalog { categories })
            }
        }

        deserializer.deserialize_map(Tables)
    }
}
