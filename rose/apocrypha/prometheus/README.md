# Prometheus

## MacOS Installation

```terminal
brew install prometheus
```

Then run with:

```terminal
prometheus \
    --config.file=modules/prometheus/prometheus.yml \
    --web.listen-address=:8050 \
    --storage.tsdb.path=prometheus/data/
```

https://prometheus.io/docs/prometheus/latest/configuration/configuration/

Then navigate to http://localhost:8050/ to access the Prometheus UI.

This module has also been set up to deploy to the Nimbus platform using the CLI command:

```terminal
uv run prom deploy
```

There's no volumes set up as far as I am aware so this will be wiped on redeploy.


## Next

1. https://grafana.com/docs/mimir/latest/
2. https://grafana.com/docs/alloy/latest/
