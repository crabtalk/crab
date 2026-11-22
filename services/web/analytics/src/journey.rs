use alloc::{string::String, vec::Vec};
use berm_lang::{Result, Schema, caller_storage, json};

const VISIT_LIMIT: usize = 25;
const PAGE_LIMIT: usize = 25;
const PAYMENT_LIMIT: usize = 100;
// Leave room for the report envelope in the service's 1 MiB output buffer.
const VISIT_BYTES: usize = 768 << 10;
const PAYMENT_BYTES: usize = 240 << 10;
const SECONDS_PER_DAY: f64 = 86_400.0;

#[derive(Schema)]
pub struct Journey {
    pub visits: Vec<Visit>,
    pub visits_truncated: bool,
    pub payments: Vec<Payment>,
    pub payments_truncated: bool,
    /// Elapsed days, including fractions, when the first recorded payment followed the first visit.
    pub days_to_first_payment: Option<f64>,
}

#[derive(Schema)]
pub struct Visit {
    pub visitor: String,
    pub domain: String,
    pub session: String,
    /// First page view time in Unix seconds.
    pub at: u64,
    pub landing: String,
    pub channel: String,
    pub pages: Vec<String>,
    pub pages_truncated: bool,
}

#[derive(Schema)]
pub struct Payment {
    pub id: String,
    pub customer: Option<String>,
    pub visitor: Option<String>,
    pub session: Option<String>,
    /// Minor currency units.
    pub amount: u64,
    pub currency: String,
    pub at: u64,
}

impl Journey {
    pub(crate) fn report(customer: &str) -> Result<Self> {
        let [(is_customer,)] = caller_storage::rows::<(u64,)>(
            "SELECT EXISTS (SELECT 1 FROM customers WHERE id = ?1)
                 OR EXISTS (SELECT 1 FROM payments WHERE customer = ?1)",
            &(customer,),
        )?[..] else {
            return Err("Could not resolve the customer or visitor ID".into());
        };

        let mut sessions = caller_storage::rows::<(String, String, String, u64)>(
            "WITH visitors(visitor) AS (
               SELECT visitor FROM customers WHERE ?2 AND id = ?1
               UNION
               SELECT visitor FROM analytics_sales WHERE ?2 AND customer = ?1 AND visitor IS NOT NULL
               UNION
               SELECT ?1 WHERE NOT ?2
             )
             SELECT h.visitor, h.domain, h.session, min(h.at) AS at
             FROM visitors v JOIN hits h ON v.visitor = h.visitor
             WHERE h.kind = 'view' AND h.path IS NOT NULL
             GROUP BY h.visitor, h.domain, h.session
             ORDER BY at, h.visitor, h.domain, h.session LIMIT ?3",
            &(customer, is_customer, VISIT_LIMIT + 1),
        )?;
        let mut visits_truncated = sessions.len() > VISIT_LIMIT;
        sessions.truncate(VISIT_LIMIT);

        let mut visits = Vec::with_capacity(sessions.len());
        let mut visit_bytes = 0;
        for (visitor, domain, session, at) in sessions {
            let views = caller_storage::rows::<(String, String)>(
                "SELECT path, channel FROM hits
                 WHERE kind = 'view' AND path IS NOT NULL
                   AND visitor = ?1 AND domain = ?2 AND session = ?3
                 ORDER BY at, id LIMIT ?4",
                &(&visitor, &domain, &session, PAGE_LIMIT + 1),
            )?;
            let Some((landing, channel)) = views.first().cloned() else {
                continue;
            };
            let pages_truncated = views.len() > PAGE_LIMIT;
            let visit = Visit {
                visitor,
                domain,
                session,
                at,
                landing,
                channel,
                pages: views
                    .into_iter()
                    .take(PAGE_LIMIT)
                    .map(|(path, _)| path)
                    .collect(),
                pages_truncated,
            };
            visit_bytes += json::to(&visit)?.len() + 1;
            if visit_bytes > VISIT_BYTES {
                visits_truncated = true;
                break;
            }
            visits.push(visit);
        }

        let mut payments = caller_storage::rows::<Payment>(
            "SELECT id, customer, visitor, session, amount, currency, at
             FROM analytics_sales
             WHERE ?2 AND customer = ?1
             UNION ALL
             SELECT id, customer, visitor, session, amount, currency, at
             FROM analytics_sales WHERE NOT ?2 AND visitor = ?1
             ORDER BY at, id LIMIT ?3",
            &(customer, is_customer, PAYMENT_LIMIT + 1),
        )?;
        let mut payment_bytes = 0;
        let mut keep = 0;
        for payment in payments.iter().take(PAYMENT_LIMIT) {
            payment_bytes += json::to(payment)?.len() + 1;
            if payment_bytes > PAYMENT_BYTES {
                break;
            }
            keep += 1;
        }
        let payments_truncated = payments.len() > keep;
        payments.truncate(keep);
        let days_to_first_payment =
            visits
                .first()
                .zip(payments.first())
                .and_then(|(visit, payment)| {
                    payment
                        .at
                        .checked_sub(visit.at)
                        .map(|seconds| seconds as f64 / SECONDS_PER_DAY)
                });
        Ok(Self {
            visits,
            visits_truncated,
            payments,
            payments_truncated,
            days_to_first_payment,
        })
    }
}
