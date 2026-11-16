use berm_lang::{Branch, Result, Schema, tool, workflow};
use webpage::{page::Page, reading::Reading};

#[derive(Schema)]
pub struct Input {
    pub urls: Vec<String>,
}

#[derive(Clone, Schema)]
pub struct Snapshot {
    pub url: String,
    pub markdown: String,
}

#[derive(Schema)]
pub struct Changes {
    pub urls: Vec<String>,
}

#[derive(Schema)]
struct Changed {}
#[derive(Schema)]
struct Unchanged {}
#[derive(Schema, Branch)]
enum Route {
    Changed(Changed),
    Unchanged(Unchanged),
}

#[derive(Schema)]
pub struct Report {
    pub checked: usize,
    pub changed: Vec<String>,
    pub saved: bool,
}

/// Read server HTML using the existing Markdown service library.
#[tool(query)]
pub fn read(url: String) -> Result<Snapshot> {
    let reading = Reading::try_from(&Page::fetch(&url)?)?;
    Ok(Snapshot {
        url: reading.url,
        markdown: reading.markdown,
    })
}

/// Find changes against the deployment's shared saved snapshots.
#[tool(query)]
pub fn compare(pages: Vec<Snapshot>) -> Result<Changes> {
    let mut urls = Vec::new();
    for page in pages {
        let previous: Option<String> = berm_lang::get_json(&page.url)?;
        if previous.as_ref() != Some(&page.markdown) {
            urls.push(page.url);
        }
    }
    Ok(Changes { urls })
}

/// Replace the deployment's shared snapshots with this batch.
#[tool]
pub fn save(pages: Vec<Snapshot>) -> Result<usize> {
    for page in &pages {
        berm_lang::set_json(&page.url, &page.markdown)?;
    }
    Ok(pages.len())
}

struct PageWatch;
#[workflow]
impl PageWatch {
    #[step(each, max = 3)]
    fn pages(input: &Input) -> Vec<ReadCall> {
        input
            .urls
            .iter()
            .map(|url| ReadCall { url: url.clone() })
            .collect()
    }

    #[step]
    fn difference(pages: &[Snapshot]) -> CompareCall {
        CompareCall {
            pages: pages.to_vec(),
        }
    }

    #[branch]
    fn route(difference: &Changes) -> Route {
        if difference.urls.is_empty() {
            Route::Unchanged(Unchanged {})
        } else {
            Route::Changed(Changed {})
        }
    }

    #[step]
    fn save(route: &Changed, pages: &[Snapshot]) -> SaveCall {
        let _ = route;
        SaveCall {
            pages: pages.to_vec(),
        }
    }

    #[output]
    fn report(pages: &[Snapshot], difference: &Changes, save: Option<&usize>) -> Report {
        Report {
            checked: pages.len(),
            changed: difference.urls.clone(),
            saved: save.is_some(),
        }
    }
}
