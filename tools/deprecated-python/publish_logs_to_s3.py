#!/usr/bin/env python3
"""Publish friendly Moshi logs to an S3 bucket."""

from __future__ import annotations

import argparse
import hashlib
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List

try:  # Optional dependency that callers install via `uv run --with boto3`
    import boto3
    from botocore.exceptions import BotoCoreError, ClientError, NoCredentialsError
except ModuleNotFoundError:  # pragma: no cover - handled at runtime
    boto3 = None  # type: ignore[assignment]
    BotoCoreError = ClientError = NoCredentialsError = Exception  # type: ignore[assignment]

S3_NOT_FOUND_CODES = {"404", "NotFound", "NoSuchKey"}


@dataclass
class PublishStats:
    uploaded: int = 0
    skipped: int = 0
    dry_run_uploads: int = 0
    bytes_uploaded: int = 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Upload friendly Moshi logs (excluding raw traces by default) to an S3 bucket. "
            "Stores the local file's MD5 hash in object metadata so future runs skip unchanged logs."
        )
    )
    parser.add_argument(
        "--bucket",
        required=True,
        help="Destination S3 bucket name.",
    )
    parser.add_argument(
        "--prefix",
        default="",
        help="Optional S3 prefix (e.g., 'moshi/logs').",
    )
    parser.add_argument(
        "--source",
        default="logs/moshi-logs",
        help="Directory containing friendly logs (default: logs/moshi-logs).",
    )
    parser.add_argument(
        "--include-raw",
        action="store_true",
        help="Also upload raw traces under logs/moshi-logs/raw.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the actions without uploading anything.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Upload every file even when the remote copy already matches the local hash.",
    )
    parser.add_argument(
        "--profile",
        help="Optional AWS profile name to use when creating the boto3 session.",
    )
    parser.add_argument(
        "--region",
        help="Optional AWS region override for the boto3 session.",
    )
    parser.add_argument(
        "--endpoint-url",
        dest="endpoint_url",
        help="Custom S3 endpoint URL (useful for S3-compatible storage).",
    )
    parser.add_argument(
        "--acl",
        help="Optional canned ACL to apply (e.g., public-read).",
    )
    return parser.parse_args()


def ensure_boto3_available() -> None:
    if boto3 is None:
        raise SystemExit(
            "boto3 is required for this helper. Install it via 'uv run --with boto3 "
            "scripts/publish_logs_to_s3.py ...' or add boto3 to your environment."
        )


def gather_log_files(base_dir: Path, include_raw: bool) -> List[Path]:
    if not base_dir.exists():
        raise SystemExit(f"Source directory {base_dir} does not exist.")
    files: List[Path] = []
    for candidate in sorted(base_dir.rglob("*")):
        if not candidate.is_file():
            continue
        if not include_raw:
            rel = candidate.relative_to(base_dir)
            if "raw" in rel.parts:
                continue
        files.append(candidate)
    return files


def compute_md5(path: Path) -> str:
    digest = hashlib.md5()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def human_bytes(value: int) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    size = float(value)
    for unit in units:
        if size < 1024.0 or unit == units[-1]:
            return f"{size:.2f} {unit}"
        size /= 1024.0
    return f"{value} B"


class LogPublisher:
    def __init__(
        self,
        s3_client,
        bucket: str,
        prefix: str,
        source_dir: Path,
        dry_run: bool,
        force: bool,
        acl: str | None,
    ) -> None:
        self.s3 = s3_client
        self.bucket = bucket
        self.prefix = prefix.strip("/")
        self.source_dir = source_dir
        self.dry_run = dry_run
        self.force = force
        self.acl = acl
        self.stats = PublishStats()
        self.errors: list[str] = []

    def publish(self, files: Iterable[Path]) -> PublishStats:
        for path in files:
            try:
                self._publish_single(path)
            except (ClientError, BotoCoreError, NoCredentialsError) as exc:
                message = f"{path}: {exc}"
                self.errors.append(message)
                print(f"[error] {message}", file=sys.stderr)
        return self.stats

    def _publish_single(self, path: Path) -> None:
        rel_path = path.relative_to(self.source_dir)
        s3_key = self._build_key(rel_path)
        md5_hex = compute_md5(path)
        if not self.force and self._remote_matches(s3_key, md5_hex):
            self.stats.skipped += 1
            print(f"[skip] {rel_path} unchanged (s3://{self.bucket}/{s3_key})")
            return
        if self.dry_run:
            self.stats.dry_run_uploads += 1
            print(
                f"[dry-run] Would upload {rel_path} -> s3://{self.bucket}/{s3_key} "
                f"(md5={md5_hex})"
            )
            return
        extra_args = {
            "ContentType": "text/plain",
            "Metadata": {"local-md5": md5_hex},
        }
        if self.acl:
            extra_args["ACL"] = self.acl
        size = path.stat().st_size
        self.s3.upload_file(str(path), self.bucket, s3_key, ExtraArgs=extra_args)
        self.stats.uploaded += 1
        self.stats.bytes_uploaded += size
        print(
            f"[upload] {rel_path} -> s3://{self.bucket}/{s3_key} "
            f"({human_bytes(size)})"
        )

    def _remote_matches(self, key: str, md5_hex: str) -> bool:
        if self.force:
            return False
        try:
            response = self.s3.head_object(Bucket=self.bucket, Key=key)
        except ClientError as exc:
            error_code = exc.response.get("Error", {}).get("Code", "")
            if error_code in S3_NOT_FOUND_CODES:
                return False
            raise
        metadata = response.get("Metadata", {}) or {}
        remote_md5 = metadata.get("local-md5")
        if remote_md5:
            return remote_md5 == md5_hex
        etag = (response.get("ETag") or "").strip('"')
        return bool(etag) and etag == md5_hex

    def _build_key(self, relative_path: Path) -> str:
        relative_posix = relative_path.as_posix()
        if not self.prefix:
            return relative_posix
        return f"{self.prefix}/{relative_posix}"


def summarize(stats: PublishStats, errors: list[str]) -> None:
    print()
    print("Summary:")
    print(f"  Uploaded: {stats.uploaded}")
    print(f"  Skipped (unchanged): {stats.skipped}")
    print(f"  Dry-run uploads: {stats.dry_run_uploads}")
    print(f"  Bytes uploaded: {human_bytes(stats.bytes_uploaded)}")
    if errors:
        print(f"  Errors: {len(errors)} (see stderr for details)")


def main() -> int:
    args = parse_args()
    ensure_boto3_available()
    source_dir = Path(args.source).expanduser()
    files = gather_log_files(source_dir, include_raw=args.include_raw)
    if not files:
        print(
            f"No logs found under {source_dir}. Run scripts/format_moshi_log.py first "
            "or point --source at a directory containing friendly logs."
        )
        return 0
    session_kwargs = {}
    if args.profile:
        session_kwargs["profile_name"] = args.profile
    if args.region:
        session_kwargs["region_name"] = args.region
    session = boto3.session.Session(**session_kwargs)  # type: ignore[union-attr]
    client_kwargs = {}
    if args.endpoint_url:
        client_kwargs["endpoint_url"] = args.endpoint_url
    s3_client = session.client("s3", **client_kwargs)
    publisher = LogPublisher(
        s3_client=s3_client,
        bucket=args.bucket,
        prefix=args.prefix,
        source_dir=source_dir,
        dry_run=args.dry_run,
        force=args.force,
        acl=args.acl,
    )
    stats = publisher.publish(files)
    summarize(stats, publisher.errors)
    return 1 if publisher.errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
