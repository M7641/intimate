# Minimal image for the FastAPI example, so `gauge run` can measure it with
# exact cgroup accounting. Build it directly, or let `gauge run --build` do it.
FROM python:3.14-slim

WORKDIR /app
RUN pip install --no-cache-dir "fastapi>=0.110" "uvicorn>=0.29"
COPY app.py .

# Bind all interfaces so a published port reaches the app (see app.py).
ENV GAUGE_HOST=0.0.0.0
ENV PORT=8080
EXPOSE 8080
CMD ["python", "app.py"]
