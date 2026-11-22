CREATE INDEX hits_view_session ON hits (visitor, domain, session, at) WHERE kind = 'view';
CREATE INDEX payments_at ON payments (at);

CREATE VIEW analytics_sales AS
SELECT p.id, p.customer, COALESCE(p.visitor, c.visitor) AS visitor,
       p.session, p.amount, p.currency, p.at,
       CASE
         WHEN p.customer IS NOT NULL THEN 'c:' || p.customer
         WHEN p.visitor IS NOT NULL THEN 'v:' || p.visitor
         ELSE 'p:' || p.id
       END AS payer
FROM payments p LEFT JOIN customers c ON c.id = p.customer;
