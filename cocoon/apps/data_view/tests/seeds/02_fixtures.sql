-- Two loads per snapshot table, so timestamp / row-count / snapshot endpoints
-- have something to distinguish. The latest load (2026-06-08) is the default
-- the API serves when no timestamp is requested.

-- customer_snapshot — load 1 (2026-06-01): 8 rows
INSERT INTO stage.customer_snapshot
    (customer_id, customer_name, region, lifetime_value, orders_count, is_active,
     row_hash, load_timestamp, batch_group_id, snapshot_type, source_system)
VALUES
    (1, 'Acme Corp',      'emea',  1000.50,  4, true,  'h1a', '2026-06-01 00:00:00', 'b1', 'full', 'crm'),
    (2, 'Globex',         'emea',  2500.00, 11, true,  'h2a', '2026-06-01 00:00:00', 'b1', 'full', 'crm'),
    (3, 'Initech',        'amer',   320.75,  2, false, 'h3a', '2026-06-01 00:00:00', 'b1', 'full', 'crm'),
    (4, 'Umbrella',       'amer',  5400.20, 18, true,  'h4a', '2026-06-01 00:00:00', 'b1', 'full', 'crm'),
    (5, 'Hooli',          'apac',   880.00,  6, true,  'h5a', '2026-06-01 00:00:00', 'b1', 'full', 'crm'),
    (6, 'Stark Industries','apac', 9999.99, 30, true,  'h6a', '2026-06-01 00:00:00', 'b1', 'full', 'crm'),
    (7, 'Wayne Enterprises','emea',4200.00, 14, true,  'h7a', '2026-06-01 00:00:00', 'b1', 'full', 'crm'),
    (8, 'Soylent',        'amer',     0.00,  0, false, 'h8a', '2026-06-01 00:00:00', 'b1', 'full', 'crm');

-- customer_snapshot — load 2 (2026-06-08): 8 rows, values shifted
INSERT INTO stage.customer_snapshot
    (customer_id, customer_name, region, lifetime_value, orders_count, is_active,
     row_hash, load_timestamp, batch_group_id, snapshot_type, source_system)
VALUES
    (1, 'Acme Corp',      'emea',  1200.00,  5, true,  'h1b', '2026-06-08 00:00:00', 'b2', 'full', 'crm'),
    (2, 'Globex',         'emea',  2500.00, 11, true,  'h2b', '2026-06-08 00:00:00', 'b2', 'full', 'crm'),
    (3, 'Initech',        'amer',   415.25,  3, true,  'h3b', '2026-06-08 00:00:00', 'b2', 'full', 'crm'),
    (4, 'Umbrella',       'amer',  5600.00, 19, true,  'h4b', '2026-06-08 00:00:00', 'b2', 'full', 'crm'),
    (5, 'Hooli',          'apac',   910.50,  7, true,  'h5b', '2026-06-08 00:00:00', 'b2', 'full', 'crm'),
    (6, 'Stark Industries','apac',10500.00, 32, true,  'h6b', '2026-06-08 00:00:00', 'b2', 'full', 'crm'),
    (7, 'Wayne Enterprises','emea',4100.00, 13, true,  'h7b', '2026-06-08 00:00:00', 'b2', 'full', 'crm'),
    (8, 'Soylent',        'amer',    50.00,  1, false, 'h8b', '2026-06-08 00:00:00', 'b2', 'full', 'crm');

-- order_snapshot — two loads, fewer columns
INSERT INTO stage.order_snapshot (order_id, customer_id, amount, status, load_timestamp)
VALUES
    (101, 1,  250.00, 'shipped',   '2026-06-01 00:00:00'),
    (102, 2,  900.00, 'pending',   '2026-06-01 00:00:00'),
    (103, 4, 1200.00, 'shipped',   '2026-06-01 00:00:00'),
    (104, 1,  250.00, 'shipped',   '2026-06-08 00:00:00'),
    (105, 2,  950.00, 'cancelled', '2026-06-08 00:00:00'),
    (106, 6, 3000.00, 'shipped',   '2026-06-08 00:00:00');

-- region_lookup — unversioned reference data (no load_timestamp)
INSERT INTO marts.region_lookup (region, country, population)
VALUES
    ('emea', 'United Kingdom', 67000000),
    ('amer', 'United States',  331000000),
    ('apac', 'Japan',          125000000);
