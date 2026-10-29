//! The catalog's list, `catalog.toml`: its categories and the platforms in
//! each. A tool's `catalog` job starts with a listed platform.

use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};
use std::{fmt, str::FromStr, sync::LazyLock};

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
