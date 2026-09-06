# Database

## Installation

```pyprojecttoml
[project]
dependencies = [
    "database[redshift] @ git+https://github.com/nimbus-labs/nimbus-monorepo.git#egg=database&subdirectory=blank/io/database",
]
```

All the DB clients will follow https://peps.python.org/pep-0249/, so we can base the abstraction on that.

Docs used:

1. https://www.pymssql.org/
2. https://duckdb.org/docs/stable/clients/python/dbapi
3. https://docs.snowflake.com/en/developer-guide/python-connector/python-connector-api#object-connection
4. https://docs.aws.amazon.com/redshift/latest/mgmt/python-connect-examples.html

Log on fail and when running with an env saying DEBUG_QUERIES?

## Thinking

Static parameters - There is an idea that we could split the concept of parameters into two. The first is static parameters which is handled by jinja2. Then there are live paramters which are done through the connection engine paramter binding. The problem is that I don't know if all the connectors share a parameter binding syntax.

Snowflake does appear to have a different paratmer binding syntax which does make things more complicated. It probably just highlights that where the abstraction happens is not yet correct.

https://github.com/nimbus-labs/nimbus-sdk/blob/056c4d0ddba01768df8eaf8dc6c1f80269c0caaf/nimbus/resources/tenants.py for the variable snowflake warehouse. An extra method to get the available warehouses on a tenant.
