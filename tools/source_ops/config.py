# Where: tools/source_ops/config.py
# What: Runtime configuration loader for source collection and wiki refresh automation.
# Why: Centralize paths, thresholds, and environment-specific wiki CLI settings.
from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from .common import SOURCE_OPS_DIR, ensure_dir


def _env_flag(name: str, default: bool) -> bool:
    raw = os.getenv(name)
    if raw is None:
        return default
    return raw.lower() in {"1", "true", "yes", "on"}


@dataclass(frozen=True)
class Settings:
    registry_path: Path
    raw_dir: Path
    normalized_dir: Path
    reports_dir: Path
    state_path: Path
    snapshots_dir: Path
    http_timeout_seconds: int
    write_timeout_seconds: int
    max_changed_records: int
    max_deleted_records: int
    wiki_cli_bin: str
    wiki_nodes_dir: Path
    kinic_identity: str
    cli_bin: str
    staging_database_id: str
    prod_database_id: str
    staging_ic_host: str
    prod_ic_host: str
    staging_fetch_root_key: bool
    prod_fetch_root_key: bool
    staging_icp_environment: str
    prod_icp_environment: str


def load_settings() -> Settings:
    artifacts_dir = ensure_dir(SOURCE_OPS_DIR / "artifacts")
    return Settings(
        registry_path=SOURCE_OPS_DIR / "registry.yaml",
        raw_dir=ensure_dir(artifacts_dir / "raw"),
        normalized_dir=ensure_dir(artifacts_dir / "normalized"),
        reports_dir=ensure_dir(artifacts_dir / "reports"),
        state_path=ensure_dir(SOURCE_OPS_DIR / "state") / "manifest.json",
        snapshots_dir=ensure_dir(SOURCE_OPS_DIR / "state" / "snapshots"),
        http_timeout_seconds=int(os.getenv("SOURCE_OPS_HTTP_TIMEOUT", "20")),
        write_timeout_seconds=int(os.getenv("SOURCE_OPS_WRITE_TIMEOUT", "180")),
        max_changed_records=int(os.getenv("SOURCE_OPS_MAX_CHANGED_RECORDS", "200")),
        max_deleted_records=int(os.getenv("SOURCE_OPS_MAX_DELETED_RECORDS", "25")),
        wiki_cli_bin=os.getenv("SOURCE_OPS_WIKI_CLI_BIN", "kinic-vfs-cli"),
        wiki_nodes_dir=ensure_dir(artifacts_dir / "wiki_nodes"),
        kinic_identity=os.getenv("SOURCE_OPS_KINIC_IDENTITY", "default"),
        cli_bin=os.getenv(
            "SOURCE_OPS_CLI_BIN",
            "cargo run --quiet --bin kinic-context-cli --",
        ),
        staging_database_id=os.getenv("SOURCE_OPS_STAGING_DATABASE_ID", ""),
        prod_database_id=os.getenv("SOURCE_OPS_PROD_DATABASE_ID", ""),
        staging_ic_host=os.getenv("SOURCE_OPS_STAGING_IC_HOST", "http://127.0.0.1:8000"),
        prod_ic_host=os.getenv("SOURCE_OPS_PROD_IC_HOST", "https://ic0.app"),
        staging_fetch_root_key=_env_flag("SOURCE_OPS_STAGING_FETCH_ROOT_KEY", True),
        prod_fetch_root_key=_env_flag("SOURCE_OPS_PROD_FETCH_ROOT_KEY", False),
        staging_icp_environment=os.getenv("SOURCE_OPS_STAGING_ICP_ENVIRONMENT", "local"),
        prod_icp_environment=os.getenv("SOURCE_OPS_PROD_ICP_ENVIRONMENT", "ic"),
    )
