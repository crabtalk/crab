use crate::period::Window;
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use berm_lang::{Result, Schema, caller_storage};

#[derive(Schema)]
pub enum Model {
    First,
    Last,
}

#[derive(Schema, Default)]
pub enum By {
    #[default]
    Channel,
    Source,
    Medium,
    Campaign,
    Landing,
}

#[derive(Schema, Default)]
pub struct Channels {
    pub channels: Vec<Channel>,
    /// Oldest completed Stripe endpoint cursor, in Unix seconds, if configured.
    pub revenue_synced_through: Option<u64>,
    pub revenue_fully_synced: bool,
}

#[derive(Schema, Default)]
pub struct Channel {
    /// Value of the requested grouping.
    pub channel: String,
    pub visitors: u64,
    pub paying_customers: u64,
    /// Minor currency units, keyed by currency code.
    pub revenue: BTreeMap<String, u64>,
}

impl Channels {
    pub(crate) fn report(window: Window, model: Model, by: By) -> Result<Self> {
        let dimension = match by {
            By::Channel => "channel",
            By::Source => "source",
            By::Medium => "medium",
            By::Campaign => "campaign",
            By::Landing => "landing",
        };
        let breakdown = caller_storage::rows::<(String, u64, u64, Option<String>, u64)>(
            "WITH
             activity AS (
               SELECT h.visitor, h.at, h.id AS hit, (?4 <> 'landing' OR h.kind = 'view') AS can_touch,
                      CASE ?4
                        WHEN 'source' THEN NULLIF(h.source, '')
                        WHEN 'medium' THEN NULLIF(h.medium, '')
                        WHEN 'campaign' THEN NULLIF(h.campaign, '')
                        WHEN 'landing' THEN h.path
                        ELSE h.channel
                      END AS dimension,
                      NULL AS payer, NULL AS currency, NULL AS amount
               FROM hits h WHERE h.at >= ?1 AND h.at < ?2
               UNION ALL
               SELECT visitor, at, NULL, 0, NULL, payer, currency, amount
               FROM analytics_sales WHERE at >= ?1 AND at < ?2
             ),
             chronology AS (
               SELECT *, min(at) FILTER (WHERE hit IS NULL) OVER (PARTITION BY visitor) AS first_payment
               FROM activity
             ),
             eligible AS (
               SELECT *, can_touch AND (NOT ?3 OR first_payment IS NULL OR at < first_payment) AS touch
               FROM chronology
             ),
             attributed AS MATERIALIZED (
               SELECT visitor, hit, payer, currency, amount,
                      coalesce(first_value(CASE WHEN touch THEN dimension END) OVER (
                        PARTITION BY visitor
                        ORDER BY touch DESC,
                          CASE WHEN ?3 AND ?4 <> 'landing' THEN at END DESC,
                          CASE WHEN ?3 AND ?4 <> 'landing' THEN hit END DESC, at, hit
                      ), 'unattributed') AS channel
               FROM eligible
             )
             SELECT channel, count(DISTINCT visitor) FILTER (WHERE hit IS NOT NULL),
                    count(DISTINCT payer), NULL, 0
             FROM attributed GROUP BY channel
             UNION ALL
             SELECT channel, 0, 0, currency, sum(amount)
             FROM attributed WHERE currency IS NOT NULL GROUP BY channel, currency",
            &(window.since, window.until, matches!(model, Model::Last), dimension),
        )?;
        let mut channels = BTreeMap::<String, Channel>::new();
        for (name, visitors, paying_customers, currency, amount) in breakdown {
            let channel = channels.entry(name).or_default();
            channel.visitors += visitors;
            channel.paying_customers += paying_customers;
            if let Some(currency) = currency {
                channel.revenue.insert(currency, amount);
            }
        }
        Ok(Self {
            channels: channels
                .into_iter()
                .map(|(name, channel)| Channel {
                    channel: name,
                    ..channel
                })
                .collect(),
            ..Self::default()
        })
    }
}
