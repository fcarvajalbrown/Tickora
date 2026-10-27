# /// script
# requires-python = ">=3.11"
# dependencies = ["mcp==1.30.0", "psutil"]
# ///
import argparse
import asyncio
import json
import os
import sys
import time
from pathlib import Path

import psutil
from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client

GIB = 1024**3


def parse_args():
    parser = argparse.ArgumentParser(
        description="Decompile every function of a binary through a pyghidra-mcp server, resumable."
    )
    parser.add_argument("--url", default="http://127.0.0.1:8000/mcp")
    parser.add_argument("--binary", help="Program name in the project; first one if omitted.")
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--connections", type=int, default=1)
    parser.add_argument("--limit", type=int, help="Stop after this many functions in this run.")
    parser.add_argument("--max-hours", type=float, default=8.0)
    parser.add_argument("--min-free-gb", type=float, default=1.0)
    parser.add_argument("--wait-free-gb", type=float, help="Pause under --min-free-gb and resume above this.")
    parser.add_argument("--wait-poll", type=float, default=600.0)
    parser.add_argument("--log-interval", type=float, default=300.0)
    parser.add_argument("--checkpoint-every", type=int, default=100)
    parser.add_argument("--timeout-sec", type=int, default=30)
    parser.add_argument("--port", type=int, default=8000)
    return parser.parse_args()


def tool_payload(result):
    if result.isError:
        raise RuntimeError(" ".join(getattr(c, "text", "") for c in result.content) or "tool error")
    if result.structuredContent is not None:
        data = result.structuredContent
        return data.get("result", data) if isinstance(data, dict) else data
    texts = [c.text for c in result.content if getattr(c, "text", None)]
    parsed = [json.loads(t) for t in texts]
    return parsed[0] if len(parsed) == 1 else parsed


def write_json_atomic(path, data):
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(data, indent=1), encoding="utf-8")
    os.replace(tmp, path)


def server_process(port):
    for conn in psutil.net_connections(kind="tcp"):
        if conn.laddr and conn.laddr.port == port and conn.status == psutil.CONN_LISTEN and conn.pid:
            return psutil.Process(conn.pid)
    return None


def server_memory(port):
    proc = server_process(port)
    if proc is None:
        return None
    total = 0
    for p in [proc, *proc.children(recursive=True)]:
        try:
            total += p.memory_info().rss
        except psutil.Error:
            pass
    return total


class Run:
    def __init__(self, args):
        self.args = args
        self.out = args.out
        self.code_dir = args.out / "code"
        self.state_path = args.out / "state.json"
        self.functions_path = args.out / "functions.json"
        self.log_path = args.out / "progress.jsonl"
        self.state = (
            json.loads(self.state_path.read_text(encoding="utf-8"))
            if self.state_path.exists()
            else {"done": {}, "failed": {}, "elapsed_sec": 0.0}
        )
        self.started = time.monotonic()
        self.base_elapsed = self.state.get("elapsed_sec", 0.0)
        self.run_done = 0
        self.run_failed = 0
        self.since_checkpoint = 0
        self.window_start = self.started
        self.window_done = 0
        self.peak_server = 0
        self.min_free = None
        self.stop_reason = None

    def log(self, event, **fields):
        record = {"time": time.strftime("%Y-%m-%dT%H:%M:%S"), "event": event, **fields}
        line = json.dumps(record)
        print(line, flush=True)
        with self.log_path.open("a", encoding="utf-8") as fh:
            fh.write(line + "\n")

    def sample(self):
        mem = server_memory(self.args.port)
        free = psutil.virtual_memory().available
        if mem:
            self.peak_server = max(self.peak_server, mem)
        self.min_free = free if self.min_free is None else min(self.min_free, free)
        return mem, free

    def checkpoint(self):
        self.sample()
        self.state["elapsed_sec"] = self.base_elapsed + time.monotonic() - self.started
        write_json_atomic(self.state_path, self.state)
        self.since_checkpoint = 0

    def record(self, address, name, entry):
        if entry.get("error") or not entry.get("code"):
            self.state["failed"][address] = {"name": name, "error": entry.get("error") or "empty code"}
            self.run_failed += 1
        else:
            safe = address.replace(":", "_")
            (self.code_dir / f"{safe}.c").write_text(entry["code"], encoding="utf-8")
            self.state["done"][address] = name
            self.state["failed"].pop(address, None)
        self.run_done += 1
        self.window_done += 1
        self.since_checkpoint += 1
        if self.since_checkpoint >= self.args.checkpoint_every:
            self.checkpoint()

    def should_stop(self):
        if self.stop_reason:
            return True
        if time.monotonic() - self.started >= self.args.max_hours * 3600:
            self.stop_reason = "max hours reached"
        elif psutil.virtual_memory().available < self.args.min_free_gb * GIB:
            self.stop_reason = "free RAM under threshold"
        elif self.args.limit is not None and self.run_done >= self.args.limit:
            self.stop_reason = "limit reached"
        return self.stop_reason is not None


async def open_session(stack_url):
    client = streamablehttp_client(stack_url, timeout=60, sse_read_timeout=3600)
    read, write, _ = await client.__aenter__()
    session = ClientSession(read, write)
    await session.__aenter__()
    await session.initialize()
    return client, session


async def close_session(client, session):
    for ctx in (session, client):
        try:
            await ctx.__aexit__(None, None, None)
        except Exception:
            pass


async def list_functions(session, binary):
    payload = tool_payload(
        await session.call_tool(
            "search_symbols_by_name",
            {"binary_name": binary, "query": ".*", "functions_only": True, "offset": 0, "limit": 1000000},
        )
    )
    symbols = payload["symbols"] if isinstance(payload, dict) else payload
    seen = {}
    for sym in symbols:
        seen.setdefault(sym["address"], sym["name"])
    return [{"address": a, "name": n} for a, n in sorted(seen.items())]


async def resolve_binary(session, requested):
    if requested:
        return requested
    payload = tool_payload(await session.call_tool("list_project_binaries", {}))
    programs = payload.get("programs", payload) if isinstance(payload, dict) else payload
    return programs[0]["name"]


async def worker(run, queue, binary):
    client, session = await open_session(run.args.url)
    try:
        while not run.should_stop():
            try:
                func = queue.get_nowait()
            except asyncio.QueueEmpty:
                return
            try:
                payload = tool_payload(
                    await session.call_tool(
                        "decompile_function",
                        {
                            "binary_name": binary,
                            "name_or_address": func["address"],
                            "timeout_sec": run.args.timeout_sec,
                        },
                    )
                )
                entry = payload[0] if isinstance(payload, list) and payload else {"error": "no result"}
            except Exception as exc:
                entry = {"error": f"{type(exc).__name__}: {exc}"}
            run.record(func["address"], func["name"], entry)
    finally:
        await close_session(client, session)


async def reporter(run):
    while True:
        await asyncio.sleep(run.args.log_interval)
        now = time.monotonic()
        mem, free = run.sample()
        rate = run.window_done / ((now - run.window_start) / 60)
        run.window_start, run.window_done = now, 0
        run.log(
            "progress",
            done_total=len(run.state["done"]),
            failed_total=len(run.state["failed"]),
            run_done=run.run_done,
            functions_per_min=round(rate, 2),
            server_rss_gb=round(mem / GIB, 3) if mem else None,
            free_ram_gb=round(free / GIB, 3),
        )


async def main():
    args = parse_args()
    run = Run(args)
    run.code_dir.mkdir(parents=True, exist_ok=True)
    client, session = await open_session(args.url)
    try:
        binary = await resolve_binary(session, args.binary)
        if run.functions_path.exists():
            functions = json.loads(run.functions_path.read_text(encoding="utf-8"))
        else:
            listed_at = time.monotonic()
            functions = await list_functions(session, binary)
            write_json_atomic(run.functions_path, functions)
            run.log("listed", binary=binary, functions=len(functions), seconds=round(time.monotonic() - listed_at, 1))
    finally:
        await close_session(client, session)

    finished = run.state["done"]
    pending = [f for f in functions if f["address"] not in finished and f["address"] not in run.state["failed"]]
    queue = asyncio.Queue()
    for func in pending:
        queue.put_nowait(func)
    mem, free = run.sample()
    run.log(
        "start",
        binary=binary,
        connections=args.connections,
        total=len(functions),
        pending=len(pending),
        server_rss_gb=round(mem / GIB, 3) if mem else None,
        free_ram_gb=round(free / GIB, 3),
    )
    report = asyncio.create_task(reporter(run))
    began = time.monotonic()
    paused = 0.0
    try:
        while True:
            await asyncio.gather(*(worker(run, queue, binary) for _ in range(args.connections)))
            if run.stop_reason != "free RAM under threshold" or args.wait_free_gb is None:
                break
            run.checkpoint()
            pause_start = time.monotonic()
            run.log("pause", free_ram_gb=round(psutil.virtual_memory().available / GIB, 3))
            while psutil.virtual_memory().available < args.wait_free_gb * GIB:
                if time.monotonic() - run.started >= args.max_hours * 3600:
                    run.stop_reason = "max hours reached"
                    break
                await asyncio.sleep(args.wait_poll)
            paused += time.monotonic() - pause_start
            if run.stop_reason == "max hours reached":
                break
            run.stop_reason = None
            run.log("resume", free_ram_gb=round(psutil.virtual_memory().available / GIB, 3))
    finally:
        report.cancel()
        run.checkpoint()
    seconds = time.monotonic() - began
    active = seconds - paused
    mem, free = run.sample()
    run.log(
        "end",
        reason=run.stop_reason or "all functions processed",
        connections=args.connections,
        run_done=run.run_done,
        run_failed=run.run_failed,
        run_seconds=round(seconds, 1),
        paused_seconds=round(paused, 1),
        functions_per_min=round(run.run_done / (active / 60), 2) if active > 0 else None,
        done_total=len(run.state["done"]),
        failed_total=len(run.state["failed"]),
        total=len(functions),
        elapsed_total_sec=round(run.state["elapsed_sec"], 1),
        peak_server_rss_gb=round(run.peak_server / GIB, 3),
        min_free_ram_gb=round(run.min_free / GIB, 3),
        server_rss_gb=round(mem / GIB, 3) if mem else None,
    )


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
