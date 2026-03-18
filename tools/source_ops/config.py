# Where: tools/source_ops/config.py
# What: Runtime configuration loader for source collection and refresh automation.
# Why: Centralize paths, thresholds, command templates, and environment-specific settings.
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
    memory_writer_template: str
    memory_rollback_template: str
    memory_reset_dim: int
    kinic_identity: str
    cli_bin: str
    staging_catalog_canister_id: str
    prod_catalog_canister_id: str
    staging_ic_host: str
    prod_ic_host: str
    staging_fetch_root_key: bool
    prod_fetch_root_key: bool
    staging_icp_environment: str
    prod_icp_environment: str


def load_settings() -> Settings:
    artifacts_dir = ensure_dir(SOURCE_OPS_DIR / "artifacts")
    default_writer = (
        "python3 tools/source_ops/kinic_writer.py "
        "--env {environment} --identity {identity} --memory-id {memory_id} "
        "--payload-path {payload_path} --tag {tag}"
    )
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
        memory_writer_template=os.getenv("SOURCE_OPS_MEMORY_WRITER_TEMPLATE", default_writer),
        memory_rollback_template=os.getenv("SOURCE_OPS_MEMORY_ROLLBACK_TEMPLATE", default_writer),
        memory_reset_dim=int(os.getenv("SOURCE_OPS_MEMORY_RESET_DIM", "1024")),
        kinic_identity=os.getenv("SOURCE_OPS_KINIC_IDENTITY", "default"),
        cli_bin=os.getenv(
            "SOURCE_OPS_CLI_BIN",
            "cargo run --quiet --bin kinic-context-cli --",
        ),
        staging_catalog_canister_id=os.getenv("SOURCE_OPS_STAGING_CATALOG_CANISTER_ID", ""),
        prod_catalog_canister_id=os.getenv("SOURCE_OPS_PROD_CATALOG_CANISTER_ID", ""),
        staging_ic_host=os.getenv("SOURCE_OPS_STAGING_IC_HOST", "http://127.0.0.1:8000"),
        prod_ic_host=os.getenv("SOURCE_OPS_PROD_IC_HOST", "https://ic0.app"),
        staging_fetch_root_key=_env_flag("SOURCE_OPS_STAGING_FETCH_ROOT_KEY", True),
        prod_fetch_root_key=_env_flag("SOURCE_OPS_PROD_FETCH_ROOT_KEY", False),
        staging_icp_environment=os.getenv("SOURCE_OPS_STAGING_ICP_ENVIRONMENT", "local"),
        prod_icp_environment=os.getenv("SOURCE_OPS_PROD_ICP_ENVIRONMENT", "ic"),
    )
