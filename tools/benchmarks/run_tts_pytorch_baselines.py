import argparse
import datetime as dt
import json
import os
import subprocess
import sys
from pathlib import Path


_METRICS_PREFIX = "METRICS_JSON|"
_DEFAULT_INPUT_PATH = Path("tmp/tts_pytorch_baselines/input.txt")


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _tts_script_path() -> Path:
    return Path(__file__).resolve().with_name("tts_pytorch.py")


def _extract_metrics_json(stdout: str) -> dict:
    lines = [line for line in stdout.splitlines(
    ) if line.startswith(_METRICS_PREFIX)]
    if not lines:
        raise RuntimeError("No METRICS_JSON line found in stdout.")
    if len(lines) > 1:
        raise RuntimeError(
            f"Expected exactly 1 METRICS_JSON line, got {len(lines)}.")
    return json.loads(lines[0][len(_METRICS_PREFIX):])


def _run_baseline(
    *,
    name: str,
    inp: Path,
    out_wav: Path,
    device: str,
    half: bool,
    nq: int,
    warmup_iters: int,
    cfg_coef: float,
    temp: float,
    padding_between: int,
    seed: int,
    hf_repo: str,
    voice_repo: str,
    voice: str,
    report_memory: bool,
    no_compile: bool,
    no_cuda_graph: bool,
    dry_run: bool,
    stdout_dir: Path | None,
) -> dict:
    cmd: list[str] = [
        sys.executable,
        str(_tts_script_path()),
        str(inp),
        str(out_wav),
        "--device",
        device,
        "--nq",
        str(nq),
        "--warmup-iters",
        str(warmup_iters),
        "--cfg-coef",
        str(cfg_coef),
        "--temp",
        str(temp),
        "--padding-between",
        str(padding_between),
        "--seed",
        str(seed),
        "--hf-repo",
        hf_repo,
        "--voice-repo",
        voice_repo,
        "--voice",
        voice,
        "--metrics-json",
    ]

    if half:
        cmd.append("--half")
    if report_memory:
        cmd.append("--report-memory")
    if no_compile:
        cmd.append("--no-compile")
    if no_cuda_graph:
        cmd.append("--no-cuda-graph")

    if dry_run:
        return {
            "baseline": name,
            "dry_run": True,
            "cmd": cmd,
        }

    proc = subprocess.run(
        cmd,
        cwd=_repo_root(),
        text=True,
        capture_output=True,
        env={
            **os.environ,
        },
    )

    stdout = proc.stdout
    stderr = proc.stderr

    if stdout_dir is not None:
        stdout_dir.mkdir(parents=True, exist_ok=True)
        (stdout_dir / f"{name}.stdout.txt").write_text(stdout,
                                                       encoding="utf-8")
        (stdout_dir / f"{name}.stderr.txt").write_text(stderr,
                                                       encoding="utf-8")

    if proc.returncode != 0:
        raise RuntimeError(
            "Baseline failed: "
            f"{name} (exit={proc.returncode})\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
        )

    payload = _extract_metrics_json(stdout)

    return {
        "baseline": name,
        "timestamp": dt.datetime.now(dt.UTC).isoformat(),
        "cmd": cmd,
        "returncode": proc.returncode,
        "payload": payload,
    }


def _maybe_git_sha(repo_root: Path) -> str | None:
    try:
        proc = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            text=True,
            capture_output=True,
            check=False,
        )
        if proc.returncode != 0:
            return None
        return proc.stdout.strip() or None
    except FileNotFoundError:
        return None


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run scripts/tts_pytorch.py baseline matrix and record METRICS_JSON.")
    parser.add_argument("--inp", type=Path, default=_DEFAULT_INPUT_PATH)
    parser.add_argument("--output-dir", type=Path,
                        default=Path("tmp/tts_pytorch_baselines"))
    parser.add_argument("--results-jsonl", type=Path, default=None)
    parser.add_argument("--stdout-dir", type=Path, default=None)

    parser.add_argument("--device", type=str, default="cuda")
    parser.add_argument("--half", default=True,
                        action=argparse.BooleanOptionalAction)
    parser.add_argument("--report-memory", default=True,
                        action=argparse.BooleanOptionalAction)

    parser.add_argument("--warmup-iters", type=int, default=3)
    parser.add_argument("--seed", type=int, default=299792458)

    parser.add_argument("--cfg-coef", type=float, default=2.0)
    parser.add_argument("--temp", type=float, default=0.6)
    parser.add_argument("--padding-between", type=int, default=1)

    parser.add_argument("--hf-repo", type=str, default="kyutai/tts-1.6b-en_fr")
    parser.add_argument("--voice-repo", type=str, default="kyutai/tts-voices")
    parser.add_argument(
        "--voice",
        type=str,
        default="expresso/ex03-ex01_happy_001_channel1_334s.wav",
    )

    parser.add_argument(
        "--only",
        nargs="+",
        choices=(
            "server_default",
            "quality_ceiling",
            "no_compile_no_graph",
            "steady_state_only",
        ),
        default=None,
        help="Run only a subset of baselines.",
    )
    parser.add_argument("--dry-run", action="store_true")

    args = parser.parse_args()

    repo_root = _repo_root()
    inp = (repo_root / args.inp).resolve() if not args.inp.is_absolute() else args.inp
    if not inp.exists():
        if (not args.inp.is_absolute()) and (args.inp == _DEFAULT_INPUT_PATH):
            inp.parent.mkdir(parents=True, exist_ok=True)
            inp.write_text("Hello world.\n", encoding="utf-8")
        else:
            raise FileNotFoundError(f"Input text file not found: {inp}")

    output_dir = (repo_root / args.output_dir).resolve(
    ) if not args.output_dir.is_absolute() else args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)

    ts = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
    results_path = args.results_jsonl
    if results_path is None:
        results_path = output_dir / f"results_{ts}.jsonl"
    if not results_path.is_absolute():
        results_path = (repo_root / results_path).resolve()

    stdout_dir = args.stdout_dir
    if stdout_dir is not None and not stdout_dir.is_absolute():
        stdout_dir = (repo_root / stdout_dir).resolve()

    baselines: list[dict] = [
        {
            "name": "server_default",
            "nq": 16,
            "warmup_iters": args.warmup_iters,
            "no_compile": False,
            "no_cuda_graph": False,
        },
        {
            "name": "quality_ceiling",
            "nq": 32,
            "warmup_iters": args.warmup_iters,
            "no_compile": False,
            "no_cuda_graph": False,
        },
        {
            "name": "no_compile_no_graph",
            "nq": 16,
            "warmup_iters": 0,
            "no_compile": True,
            "no_cuda_graph": True,
        },
        {
            "name": "steady_state_only",
            "nq": 16,
            "warmup_iters": 0,
            "no_compile": False,
            "no_cuda_graph": False,
        },
    ]

    if args.only is not None:
        baselines = [b for b in baselines if b["name"] in set(args.only)]

    git_sha = _maybe_git_sha(repo_root)

    results: list[dict] = []
    for b in baselines:
        out_wav = output_dir / f"{b['name']}.wav"
        result = _run_baseline(
            name=b["name"],
            inp=inp,
            out_wav=out_wav,
            device=args.device,
            half=args.half,
            nq=int(b["nq"]),
            warmup_iters=int(b["warmup_iters"]),
            cfg_coef=args.cfg_coef,
            temp=args.temp,
            padding_between=args.padding_between,
            seed=args.seed,
            hf_repo=args.hf_repo,
            voice_repo=args.voice_repo,
            voice=args.voice,
            report_memory=args.report_memory,
            no_compile=bool(b["no_compile"]),
            no_cuda_graph=bool(b["no_cuda_graph"]),
            dry_run=bool(args.dry_run),
            stdout_dir=stdout_dir,
        )

        if git_sha is not None:
            result["git_sha"] = git_sha

        results.append(result)

        results_path.parent.mkdir(parents=True, exist_ok=True)
        with results_path.open("a", encoding="utf-8") as f:
            f.write(json.dumps(result, sort_keys=True) + "\n")

    if not args.dry_run:
        for r in results:
            metrics = r["payload"]["metrics"]
            print(
                "BASELINE|"
                + f"name={r['baseline']}|"
                + f"audio_s={metrics.get('audio_s')}|"
                + f"gen_s={metrics.get('gen_s')}|"
                + f"rtf_gen={metrics.get('rtf_gen')}|"
                + f"total_s={metrics.get('total_s')}|"
                + f"warmup_s={metrics.get('warmup_s')}"
            )

    print(f"Wrote results: {results_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
