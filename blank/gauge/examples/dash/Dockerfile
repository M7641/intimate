# Minimal image for the Dash example, so `gauge run` can measure it with exact
# cgroup accounting. Build it directly, or let `gauge run --build` do it.
FROM python:3.12-slim

WORKDIR /app
RUN pip install --no-cache-dir "dash>=2"
COPY app.py .

# Bind all interfaces so a published port reaches the app (see app.py).
ENV GAUGE_HOST=0.0.0.0
EXPOSE 8050
CMD ["python", "app.py", "8050"]
