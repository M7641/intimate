import asyncio


def run_in_thread(func, *args, **kwargs):
    func = asyncio.to_thread(func, *args, **kwargs)
    return func
