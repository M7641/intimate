import os
from pathlib import Path

from fastapi import FastAPI
from fastapi.staticfiles import StaticFiles


app = FastAPI()

if os.environ.get("HTML", "0") == "1":
    dist_path = Path(__file__).parents[2] / "dist"
else:
    dist_path = Path(__file__).parents[2] / "frontend" / "dist"

print(f"Files in directory: {list(dist_path.glob('*')) if dist_path.exists() else 'Directory does not exist'}")

@app.get("/health")
async def health() -> dict:
    return {"status": "ok"}

app.mount("/", StaticFiles(directory=dist_path, html=True), name="dist")
