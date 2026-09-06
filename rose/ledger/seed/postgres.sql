-- Postgres seed. Mirrors seed/duckdb.sql row-for-row. Loaded once at container
-- init (docker-entrypoint-initdb.d). The secondary indexes and the closing
-- ANALYZE are what let the row-store planner make production-like choices —
-- without statistics it would fall back to guesses and the plans would lie.

CREATE TABLE customers (
    id          INTEGER PRIMARY KEY,
    name        TEXT,
    country     TEXT,
    segment     TEXT,
    signup_date DATE
);

CREATE TABLE products (
    id       INTEGER PRIMARY KEY,
    name     TEXT,
    category TEXT,
    price    DOUBLE PRECISION
);

CREATE TABLE orders (
    id          INTEGER PRIMARY KEY,
    customer_id INTEGER,
    order_date  DATE,
    status      TEXT,
    total       DOUBLE PRECISION
);

CREATE TABLE order_items (
    id         INTEGER PRIMARY KEY,
    order_id   INTEGER,
    product_id INTEGER,
    quantity   INTEGER,
    line_total DOUBLE PRECISION
);

INSERT INTO customers
SELECT i,
       'Customer ' || i,
       'C' || (i % 20),
       CASE (i % 3) WHEN 0 THEN 'smb' WHEN 1 THEN 'mid' ELSE 'enterprise' END,
       DATE '2020-01-01' + (i % 1500)
FROM generate_series(1, 20000) AS t(i);

INSERT INTO products
SELECT i,
       'Product ' || i,
       'cat' || (i % 10),
       round((5 + random() * 495)::numeric, 2)
FROM generate_series(1, 2000) AS t(i);

INSERT INTO orders
SELECT i,
       (random() * 19999)::INTEGER + 1,
       DATE '2023-01-01' + (i % 365),
       CASE (i % 4) WHEN 0 THEN 'pending' WHEN 1 THEN 'paid' WHEN 2 THEN 'shipped' ELSE 'cancelled' END,
       round((10 + random() * 990)::numeric, 2)
FROM generate_series(1, 200000) AS t(i);

INSERT INTO order_items
SELECT i,
       ((i - 1) / 3) + 1,
       (random() * 1999)::INTEGER + 1,
       (random() * 4)::INTEGER + 1,
       round((1 + random() * 199)::numeric, 2)
FROM generate_series(1, 600000) AS t(i);

-- Secondary indexes: give the planner the option of index scans, the way a
-- real OLTP schema would. DuckDB has no equivalent (it scans columns), which
-- is exactly the contrast the tool is meant to show.
CREATE INDEX idx_orders_customer ON orders (customer_id);
CREATE INDEX idx_orders_date     ON orders (order_date);
CREATE INDEX idx_orders_status   ON orders (status);
CREATE INDEX idx_items_order      ON order_items (order_id);
CREATE INDEX idx_items_product    ON order_items (product_id);

ANALYZE;
