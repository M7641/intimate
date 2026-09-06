-- DuckDB seed. Must mirror seed/postgres.sql row-for-row so the same query is
-- comparable across engines. Runs once at startup into an in-memory database.

CREATE TABLE customers (
    id          INTEGER PRIMARY KEY,
    name        VARCHAR,
    country     VARCHAR,
    segment     VARCHAR,
    signup_date DATE
);

CREATE TABLE products (
    id       INTEGER PRIMARY KEY,
    name     VARCHAR,
    category VARCHAR,
    price    DOUBLE
);

CREATE TABLE orders (
    id          INTEGER PRIMARY KEY,
    customer_id INTEGER,
    order_date  DATE,
    status      VARCHAR,
    total       DOUBLE
);

CREATE TABLE order_items (
    id         INTEGER PRIMARY KEY,
    order_id   INTEGER,
    product_id INTEGER,
    quantity   INTEGER,
    line_total DOUBLE
);

-- generate_series yields BIGINT in DuckDB, so we cast to INTEGER for the
-- INTEGER columns and for the day offset (DATE + INTEGER, not DATE + BIGINT).
INSERT INTO customers
SELECT i::INTEGER,
       'Customer ' || i,
       'C' || (i % 20),
       CASE (i % 3) WHEN 0 THEN 'smb' WHEN 1 THEN 'mid' ELSE 'enterprise' END,
       DATE '2020-01-01' + (i % 1500)::INTEGER
FROM generate_series(1, 20000) AS t(i);

INSERT INTO products
SELECT i::INTEGER,
       'Product ' || i,
       'cat' || (i % 10),
       round(5 + random() * 495, 2)
FROM generate_series(1, 2000) AS t(i);

INSERT INTO orders
SELECT i::INTEGER,
       (random() * 19999)::INTEGER + 1,
       DATE '2023-01-01' + (i % 365)::INTEGER,
       CASE (i % 4) WHEN 0 THEN 'pending' WHEN 1 THEN 'paid' WHEN 2 THEN 'shipped' ELSE 'cancelled' END,
       round(10 + random() * 990, 2)
FROM generate_series(1, 200000) AS t(i);

INSERT INTO order_items
SELECT i::INTEGER,
       (((i - 1) / 3) + 1)::INTEGER,
       (random() * 1999)::INTEGER + 1,
       (random() * 4)::INTEGER + 1,
       round(1 + random() * 199, 2)
FROM generate_series(1, 600000) AS t(i);
