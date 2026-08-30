#!/usr/bin/env python3
"""Deterministic worker used only by OCR process-protocol tests."""

import json
import subprocess
import sys
import time


request = json.load(sys.stdin)
if request.get("mode") == "large_stderr":
    sys.stderr.write("diagnostic-start\n")
    sys.stderr.write("x" * (2 * 1024 * 1024))
    sys.stderr.write("\ndiagnostic-end\n")
    sys.stderr.flush()
elif request.get("mode") == "sleep":
    time.sleep(60)
elif request.get("mode") == "spawn_child":
    sys.stdout.write("partial-worker-output")
    sys.stdout.flush()
    child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
    sys.stderr.write(f"child_pid={child.pid}\n")
    sys.stderr.flush()
    time.sleep(60)

print(
    json.dumps(
        {
            "ok": True,
            "schema_version": 1,
            "status": "no_text_detected",
            "operation_id": request.get("operation_id", ""),
            "page_index": request.get("page_index", 0),
            "text": "",
            "blocks": [],
        }
    )
)
