use crate::period::Window;
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema, caller_storage};

#[derive(Schema)]
pub struct Funnel {
    pub steps: Vec<Step>,
}

#[derive(Schema)]
pub struct Step {
    pub step: String,
    pub visitors: u64,
    /// Fraction of visitors from the previous step, when it had any visitors.
    pub conversion: Option<f64>,
}

impl Funnel {
    pub(crate) fn report(steps: Vec<String>, window: Window) -> Result<Self> {
        if !(2..=8).contains(&steps.len()) {
            return Err("Supply between two and eight funnel steps".into());
        }
        if steps
            .iter()
            .any(|step| step.is_empty() || step.len() > 2049)
        {
            return Err("Funnel steps must be 1 to 2049 bytes".into());
        }

        let counts = caller_storage::rows::<(u64,)>(
            "WITH RECURSIVE
             steps AS (SELECT key AS idx, value AS step FROM json_each(?1)),
             reached(visitor, idx, at, hit) AS (
               SELECT DISTINCT visitor, -1, ?2, 0 FROM hits WHERE at >= ?2 AND at < ?3
               UNION ALL
               SELECT reached.visitor, steps.idx, next.at, next.id
               FROM reached JOIN steps ON steps.idx = reached.idx + 1
               JOIN hits AS next ON next.id = (
                 SELECT h.id FROM hits AS h
                 WHERE h.visitor = reached.visitor AND h.at >= reached.at AND h.at < ?3
                   AND (h.at, h.id) > (reached.at, reached.hit)
                   AND CASE
                     WHEN substr(steps.step, 1, 1) <> '/' THEN h.kind = 'event' AND h.name = steps.step
                     WHEN substr(steps.step, -1) = '*' THEN h.kind = 'view'
                       AND substr(h.path, 1, length(steps.step) - 1) = substr(steps.step, 1, length(steps.step) - 1)
                     ELSE h.kind = 'view' AND h.path = steps.step
                   END
                 ORDER BY h.at, h.id LIMIT 1
               )
             )
             SELECT count(reached.hit) FROM steps LEFT JOIN reached USING (idx)
             GROUP BY steps.idx ORDER BY steps.idx",
            &(&steps, window.since, window.until),
        )?;
        let mut previous = 0;
        let steps = steps
            .into_iter()
            .zip(counts)
            .map(|(step, (visitors,))| {
                let conversion = (previous > 0).then(|| visitors as f64 / previous as f64);
                previous = visitors;
                Step {
                    step,
                    visitors,
                    conversion,
                }
            })
            .collect();
        Ok(Self { steps })
    }
}
