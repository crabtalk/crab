use alloc::{collections::BTreeMap, string::String, vec::Vec};
use berm_lang::{Result, Schema, caller_storage};

#[derive(Schema)]
pub enum Model {
    First,
    Last,
}

#[derive(Schema)]
pub struct Channels {
    pub channels: Vec<Channel>,
}

#[derive(Schema, Default)]
pub struct Channel {
    pub channel: String,
    pub visitors: u64,
    pub paying_customers: u64,
    /// Minor currency units, keyed by currency code.
    pub revenue: BTreeMap<String, u64>,
}

impl Channels {
    pub(crate) fn report(since: u64, model: Model) -> Result<Self> {
        let breakdown = caller_storage::rows::<(String, u64, u64, Option<String>, u64)>(
            "WITH
             sales AS (
               SELECT p.id, p.customer, COALESCE(p.visitor, c.visitor) AS visitor,
                      p.amount, p.currency, p.at
               FROM payments p LEFT JOIN customers c ON c.id = p.customer WHERE p.at >= ?1
             ),
             first_payment AS (SELECT visitor, min(at) AS at FROM sales GROUP BY visitor),
             ranked AS (
               SELECT h.visitor, h.channel, row_number() OVER (
                 PARTITION BY h.visitor
                 ORDER BY CASE WHEN ?2 THEN h.at END DESC, CASE WHEN ?2 THEN h.id END DESC, h.at, h.id
               ) AS rank
               FROM hits h LEFT JOIN first_payment p ON h.visitor = p.visitor
               WHERE h.at >= ?1 AND (NOT ?2 OR p.at IS NULL OR h.at < p.at)
             ),
             touches AS (SELECT visitor, channel FROM ranked WHERE rank = 1),
             attributed AS (
               SELECT COALESCE(t.channel, 'unattributed') AS channel, s.amount, s.currency,
                      CASE
                        WHEN s.customer IS NOT NULL THEN 'c:' || s.customer
                        WHEN s.visitor IS NOT NULL THEN 'v:' || s.visitor
                        ELSE 'p:' || s.id
                      END AS payer
               FROM sales s LEFT JOIN touches t ON s.visitor = t.visitor
             )
             SELECT COALESCE(t.channel, 'unattributed') AS channel, count(*), 0, NULL, 0
             FROM (SELECT DISTINCT visitor FROM hits WHERE at >= ?1) v
             LEFT JOIN touches t ON v.visitor = t.visitor GROUP BY COALESCE(t.channel, 'unattributed')
             UNION ALL
             SELECT channel, 0, count(DISTINCT payer), NULL, 0 FROM attributed GROUP BY channel
             UNION ALL
             SELECT channel, 0, 0, currency, sum(amount) FROM attributed GROUP BY channel, currency",
            &(since, matches!(model, Model::Last)),
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
        })
    }
}
