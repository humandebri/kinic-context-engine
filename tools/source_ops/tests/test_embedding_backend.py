# Where: tools/source_ops/tests/test_embedding_backend.py
# What: Focused coverage for local/remote embedding backend selection and helper IO.
# Why: Keep source_ops deterministic even when local embedding dependencies are optional.
from __future__ import annotations

import json
import os
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from tools.source_ops import embedding


class EmbeddingBackendTests(unittest.TestCase):
    def tearDown(self) -> None:
        for key in [
            "EMBEDDING_API_ENDPOINT",
            "KINIC_CONTEXT_EMBEDDING_MODEL",
            "KINIC_CONTEXT_EMBEDDING_MODEL_DIR",
            "KINIC_CONTEXT_EMBEDDING_QUERY_PREFIX",
            "KINIC_CONTEXT_EMBEDDING_DOCUMENT_PREFIX",
            "KINIC_CONTEXT_EMBEDDING_HELPER",
        ]:
            os.environ.pop(key, None)

    def test_fetch_embedding_uses_local_backend_by_default(self) -> None:
        with patch("tools.source_ops.embedding._run_local_helper", return_value=[0.1, 0.2]) as local:
            self.assertEqual(embedding.fetch_embedding("query: middleware", kind="query"), [0.1, 0.2])
        local.assert_called_once_with("query: middleware", "query")

    def test_fetch_embedding_uses_remote_backend_when_endpoint_is_set(self) -> None:
        os.environ["EMBEDDING_API_ENDPOINT"] = "https://embedding.example"
        with patch(
            "tools.source_ops.embedding._fetch_remote_embedding",
            return_value=[0.3, 0.4],
        ) as remote:
            self.assertEqual(embedding.fetch_embedding("passage: section", kind="section"), [0.3, 0.4])
        remote.assert_called_once_with("https://embedding.example", "passage: section")

    def test_encode_payload_returns_contract_shape(self) -> None:
        with patch("tools.source_ops.embedding._run_local_helper", return_value=[0.5, 0.6]):
            payload = embedding.encode_payload({"kind": "query", "text": "query: launchagent"})
        self.assertEqual(payload["model"], embedding.embedding_model())
        self.assertEqual(payload["embedding"], [0.5, 0.6])

    def test_cli_mode_reads_stdin_json(self) -> None:
        stdout = StringIO()
        with patch("tools.source_ops.embedding._run_local_helper", return_value=[1.0, 2.0]):
            with patch("sys.stdin", StringIO(json.dumps({"kind": "query", "text": "query: next"}))):
                with patch("sys.stdout", stdout):
                    with patch("sys.argv", ["embedding.py", "--mode", "query", "--stdin-json"]):
                        self.assertEqual(embedding.main(), 0)
        payload = json.loads(stdout.getvalue())
        self.assertEqual(payload["embedding"], [1.0, 2.0])

    def test_run_local_helper_uses_binary_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            helper = Path(temp_dir) / "kinic-embed"
            helper.write_text(
                "#!/usr/bin/env python3\n"
                "import json, sys\n"
                "payload=json.loads(sys.stdin.read())\n"
                "assert payload['kind'] == 'section'\n"
                "assert payload['text'] == 'passage: middleware'\n"
                "print(json.dumps({'embedding':[0.7,0.8],'model':'intfloat/multilingual-e5-large'}))\n",
                encoding="utf-8",
            )
            helper.chmod(0o755)
            os.environ["KINIC_CONTEXT_EMBEDDING_HELPER"] = str(helper)
            self.assertEqual(
                embedding._run_local_helper("passage: middleware", "section"),
                [0.7, 0.8],
            )
