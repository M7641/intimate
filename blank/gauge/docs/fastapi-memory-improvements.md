# FastAPI Memory Improvement

1. Reduce the number of workers to just 1 so that only one python process is running in memory.
2. Avoid large python imports and import only what you need.
3. Ensure uvloop and httptools are being used by uvicorn.
