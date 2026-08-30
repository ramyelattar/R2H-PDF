import unittest
import sys
import io
import json
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import paddleocr_vl_worker as worker


class WorkerResultContractTests(unittest.TestCase):
    def test_cpu_dtype_selection_prefers_supported_bfloat16(self):
        class FakeTensor:
            def __add__(self, other):
                return self

        class FakeTorch:
            bfloat16 = object()
            float32 = object()

            @staticmethod
            def ones(shape, dtype, device):
                self = FakeTensor()
                self.dtype = dtype
                self.device = device
                return self

        self.assertIs(worker.select_cpu_dtype(FakeTorch()), FakeTorch.bfloat16)

    def test_offline_environment_is_enforced_over_caller_values(self):
        original = {
            name: worker.os.environ.get(name)
            for name in (
                "HF_HUB_OFFLINE",
                "TRANSFORMERS_OFFLINE",
                "HF_DATASETS_OFFLINE",
                "TOKENIZERS_PARALLELISM",
            )
        }
        try:
            worker.os.environ.update({
                "HF_HUB_OFFLINE": "0",
                "TRANSFORMERS_OFFLINE": "0",
                "HF_DATASETS_OFFLINE": "0",
                "TOKENIZERS_PARALLELISM": "true",
            })
            worker.enforce_offline_environment()
            self.assertEqual(worker.os.environ["HF_HUB_OFFLINE"], "1")
            self.assertEqual(worker.os.environ["TRANSFORMERS_OFFLINE"], "1")
            self.assertEqual(worker.os.environ["HF_DATASETS_OFFLINE"], "1")
            self.assertEqual(worker.os.environ["TOKENIZERS_PARALLELISM"], "false")
        finally:
            for name, value in original.items():
                if value is None:
                    worker.os.environ.pop(name, None)
                else:
                    worker.os.environ[name] = value

    def test_version_protocol_flushes_json_on_stdout_and_stages_on_stderr(self):
        completed = subprocess.run(
            [sys.executable, str(Path(worker.__file__)), "--version"],
            capture_output=True,
            text=True,
            check=True,
        )

        result = json.loads(completed.stdout)
        self.assertEqual(result["version"], worker.WORKER_VERSION)
        self.assertNotIn('"event"', completed.stdout)
        events = [json.loads(line) for line in completed.stderr.splitlines() if line.strip()]
        self.assertEqual([event["name"] for event in events][-3:], [
            "json_serialization",
            "stdout_flush",
            "process_exit",
        ])

    def test_transformers_progress_is_disabled_for_json_stdout_protocol(self):
        class FakeLogging:
            disabled = False

            @classmethod
            def disable_progress_bar(cls):
                cls.disabled = True

        worker.configure_transformers_logging(FakeLogging)
        self.assertTrue(FakeLogging.disabled)
        self.assertEqual(worker.CPU_ATTENTION_IMPLEMENTATION, "sdpa")

    def test_dependency_output_is_redirected_away_from_result_stdout(self):
        original_stdout = worker.sys.stdout
        original_stderr = worker.sys.stderr
        stdout = io.StringIO()
        stderr = io.StringIO()
        worker.sys.stdout = stdout
        worker.sys.stderr = stderr
        try:
            with worker.redirect_stdout(worker.sys.stderr):
                print("dependency diagnostic")
        finally:
            worker.sys.stdout = original_stdout
            worker.sys.stderr = original_stderr

        self.assertEqual(stdout.getvalue(), "")
        self.assertEqual(stderr.getvalue(), "dependency diagnostic\n")

    def test_stage_event_is_structured_and_stays_on_stderr(self):
        original_stderr = worker.sys.stderr
        original_stdout = worker.sys.stdout
        stderr = io.StringIO()
        stdout = io.StringIO()
        worker.sys.stderr = stderr
        worker.sys.stdout = stdout
        try:
            worker.emit_stage("test_stage", 12.5)
        finally:
            worker.sys.stderr = original_stderr
            worker.sys.stdout = original_stdout

        event = json.loads(stderr.getvalue())
        self.assertEqual(event["event"], "stage")
        self.assertEqual(event["name"], "test_stage")
        self.assertEqual(event["elapsed_ms"], 12.5)
        self.assertEqual(stdout.getvalue(), "")

    def test_text_only_transformer_output_does_not_create_full_page_boxes(self):
        blocks = worker.parse_ocr_output("real model text", 1000, 1000)
        self.assertEqual(blocks, [])

    def test_no_text_result_is_structured(self):
        result = worker.make_no_text_result({"operation_id": "op-1", "page_index": 0})
        self.assertTrue(result["ok"])
        self.assertEqual(result["status"], "no_text_detected")
        self.assertEqual(result["blocks"], [])
        self.assertNotIn("confidence", result)


if __name__ == "__main__":
    unittest.main()
