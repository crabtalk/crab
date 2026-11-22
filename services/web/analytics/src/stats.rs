use crate::period::Window;
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use berm_lang::{Result, Schema, caller_storage};

const DAY: i64 = 86_400;
const WEEK: i64 = 7 * DAY;
const PREVIOUS: i64 = -2;
const CURRENT: i64 = -1;

#[derive(Schema, Default)]
pub enum Interval {
    #[default]
    Day,
    Week,
}

#[derive(Schema)]
pub struct Stats {
    pub since: i64,
    pub until: i64,
    pub totals: Totals,
    pub previous: Period,
    /// Chronological UTC buckets that had activity, clipped to the reporting window.
    pub series: Vec<Period>,
}

#[derive(Schema)]
pub struct Period {
    pub since: i64,
    pub until: i64,
    pub totals: Totals,
}

#[derive(Schema, Default)]
pub struct Totals {
    pub visitors: u64,
    pub paying_customers: u64,
    /// Minor currency units, keyed by currency code.
    pub revenue: BTreeMap<String, u64>,
    /// Paying customers divided by visitors, when there were visitors.
    pub conversion_rate: Option<f64>,
    /// Minor currency units per visitor, keyed by currency code.
    pub revenue_per_visitor: BTreeMap<String, f64>,
    /// Fraction of sessions started in the period with exactly one recorded page view.
    pub bounce_rate: Option<f64>,
}

impl Stats {
    pub(crate) fn report(window: Window, interval: Interval) -> Result<Self> {
        let previous = window.previous();
        let (width, offset) = match interval {
            Interval::Day => (DAY, 0),
            Interval::Week => (WEEK, 3 * DAY),
        };
        let breakdown = caller_storage::rows::<(i64, u64, u64, u64, u64, Option<String>, u64)>(
            "WITH
             scopes(series) AS (VALUES (0), (1)),
             traffic AS MATERIALIZED (
               SELECT visitor, domain, session, kind,
                      CASE WHEN at < ?2 THEN ?6 ELSE ?7 END AS period,
                      at - (at % ?4 + ?5) % ?4 AS bucket
               FROM hits WHERE at >= ?1 AND at < ?3
             ),
             visits AS (
               SELECT visitor, domain, session, min(period) AS period, min(bucket) AS bucket,
                      count(*) AS views
               FROM traffic WHERE kind = 'view' GROUP BY visitor, domain, session
             ),
             bounces AS MATERIALIZED (
               SELECT v.period, v.bucket, v.views = 1 AND NOT EXISTS (
                 SELECT 1 FROM hits h WHERE h.kind = 'view' AND h.visitor = v.visitor
                   AND h.domain = v.domain AND h.session = v.session AND h.at >= ?3
               ) AS bounced
               FROM visits v WHERE NOT EXISTS (
                 SELECT 1 FROM hits h WHERE h.kind = 'view' AND h.visitor = v.visitor
                   AND h.domain = v.domain AND h.session = v.session AND h.at < ?1
               )
             ),
             sales AS MATERIALIZED (
               SELECT payer, currency, amount, CASE WHEN at < ?2 THEN ?6 ELSE ?7 END AS period,
                      at - (at % ?4 + ?5) % ?4 AS bucket
               FROM analytics_sales WHERE at >= ?1 AND at < ?3
             )
             SELECT CASE WHEN series THEN bucket ELSE period END AS slot,
                    count(DISTINCT visitor), 0, 0, 0, NULL, 0
             FROM traffic CROSS JOIN scopes WHERE NOT series OR period = ?7 GROUP BY slot
             UNION ALL
             SELECT CASE WHEN series THEN bucket ELSE period END AS slot,
                    0, 0, count(*), sum(bounced), NULL, 0
             FROM bounces CROSS JOIN scopes WHERE NOT series OR period = ?7 GROUP BY slot
             UNION ALL
             SELECT CASE WHEN series THEN bucket ELSE period END AS slot,
                    0, count(DISTINCT payer), 0, 0, NULL, 0
             FROM sales CROSS JOIN scopes WHERE NOT series OR period = ?7 GROUP BY slot
             UNION ALL
             SELECT CASE WHEN series THEN bucket ELSE period END AS slot,
                    0, 0, 0, 0, currency, sum(amount)
             FROM sales CROSS JOIN scopes WHERE NOT series OR period = ?7 GROUP BY slot, currency",
            &(
                previous.since,
                window.since,
                window.until.max(window.since),
                width,
                offset,
                PREVIOUS,
                CURRENT,
            ),
        )?;
        let mut buckets = BTreeMap::<i64, Totals>::new();
        for (bucket, visitors, payers, sessions, bounces, currency, amount) in breakdown {
            let totals = buckets.entry(bucket).or_default();
            if let Some(currency) = currency {
                totals.revenue.insert(currency, amount);
            } else {
                totals.visitors += visitors;
                totals.paying_customers += payers;
                if sessions > 0 {
                    totals.bounce_rate = Some(bounces as f64 / sessions as f64);
                }
            }
        }
        for totals in buckets.values_mut().filter(|t| t.visitors > 0) {
            totals.conversion_rate = Some(totals.paying_customers as f64 / totals.visitors as f64);
            totals.revenue_per_visitor = totals
                .revenue
                .iter()
                .map(|(currency, amount)| {
                    (currency.clone(), *amount as f64 / totals.visitors as f64)
                })
                .collect();
        }

        Ok(Self {
            since: window.since,
            until: window.until,
            totals: buckets.remove(&CURRENT).unwrap_or_default(),
            previous: Period {
                since: previous.since,
                until: previous.until,
                totals: buckets.remove(&PREVIOUS).unwrap_or_default(),
            },
            series: buckets
                .into_iter()
                .map(|(bucket, totals)| Period {
                    since: bucket.max(window.since),
                    until: bucket.saturating_add(width).min(window.until),
                    totals,
                })
                .collect(),
        })
    }
}
