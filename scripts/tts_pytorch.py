# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "moshi==0.2.11",
#     "torch",
#     "sphn",
#     "sounddevice",
# ]
# ///
import argparse
import json
import os
from pathlib import Path
import sys

import numpy as np
import queue
import sphn
import time
import torch


DEFAULT_DSM_TTS_REPO = "kyutai/tts-1.6b-en_fr"
DEFAULT_DSM_TTS_VOICE_REPO = "kyutai/tts-voices"


def main():
    parser = argparse.ArgumentParser(
        description="Run Kyutai TTS using the PyTorch implementation"
    )
    parser.add_argument("inp", type=str, help="Input file, use - for stdin.")
    parser.add_argument(
        "out", type=str, help="Output file to generate, use - for playing the audio"
    )
    parser.add_argument(
        "--hf-repo",
        type=str,
        default=DEFAULT_DSM_TTS_REPO,
        help="HF repo in which to look for the pretrained models.",
    )
    parser.add_argument(
        "--voice-repo",
        default=DEFAULT_DSM_TTS_VOICE_REPO,
        help="HF repo in which to look for pre-computed voice embeddings.",
    )
    parser.add_argument(
        "--voice",
        default="expresso/ex03-ex01_happy_001_channel1_334s.wav",
        help="The voice to use, relative to the voice repo root. "
        f"See {DEFAULT_DSM_TTS_VOICE_REPO}",
    )
    parser.add_argument(
        "--nq",
        type=int,
        default=32,
        help="Number of codebooks to generate. Lower is faster, higher is better quality.",
    )
    parser.add_argument(
        "--temp",
        type=float,
        default=0.6,
        help="Temperature for sampling.",
    )
    parser.add_argument(
        "--cfg-coef",
        type=float,
        default=2.0,
        help="CFG coefficient. For distillation models, this is used as conditioning.",
    )
    parser.add_argument(
        "--padding-between",
        type=int,
        default=1,
        help="Forces a minimal amount of fixed padding between words.",
    )
    parser.add_argument(
        "--half",
        action="store_const",
        const=torch.float16,
        default=torch.bfloat16,
        dest="dtype",
        help="Run inference with float16, not bfloat16, better for old GPUs.",
    )
    parser.add_argument(
        "--device",
        type=str,
        default="cuda",
        help="Device on which to run, defaults to 'cuda'.",
    )
    parser.add_argument(
        "--no-compile",
        action="store_true",
        help="Disable torch.compile (sets NO_TORCH_COMPILE=1).",
    )
    parser.add_argument(
        "--no-cuda-graph",
        action="store_true",
        help="Disable CUDA graphs (sets NO_CUDA_GRAPH=1).",
    )
    parser.add_argument(
        "--warmup-iters",
        type=int,
        default=0,
        help="Optional warmup iterations (helps stabilize steady-state performance).",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=299792458,
        help="Random seed for reproducible runs.",
    )
    parser.add_argument(
        "--report-memory",
        action="store_true",
        help="Report CUDA peak memory usage (allocated/reserved).",
    )
    parser.add_argument(
        "--metrics-json",
        action="store_true",
        help="Emit metrics as a single JSON line (prefixed with METRICS_JSON|).",
    )
    args = parser.parse_args()

    if args.no_compile:
        os.environ["NO_TORCH_COMPILE"] = "1"
    if args.no_cuda_graph:
        os.environ["NO_CUDA_GRAPH"] = "1"

    from moshi.models.loaders import CheckpointInfo
    from moshi.models.tts import TTSModel

    torch.manual_seed(args.seed)
    if torch.cuda.is_available():
        torch.cuda.manual_seed_all(args.seed)

    def _sync() -> None:
        if str(args.device).startswith("cuda") and torch.cuda.is_available():
            torch.cuda.synchronize()

    def _emit_metrics(config: dict, metrics: dict) -> None:
        if args.metrics_json:
            payload = {
                "config": config,
                "metrics": metrics,
                "schema_version": 1,
            }
            print(
                "METRICS_JSON|"
                + json.dumps(payload, sort_keys=True, separators=(",", ":"))
            )

    start_total = time.perf_counter()
    if args.report_memory and str(args.device).startswith("cuda") and torch.cuda.is_available():
        torch.cuda.reset_peak_memory_stats()
    _sync()

    print("Loading model...")
    start_load_checkpoint = time.perf_counter()
    checkpoint_info = CheckpointInfo.from_hf_repo(args.hf_repo)
    _sync()
    end_load_checkpoint = time.perf_counter()

    start_load_model = time.perf_counter()
    tts_model = TTSModel.from_checkpoint_info(
        checkpoint_info, voice_repo=args.voice_repo, n_q=args.nq, temp=args.temp, cfg_coef=args.cfg_coef,
        device=args.device, dtype=args.dtype
    )
    _sync()
    end_load_model = time.perf_counter()

    cfg_coef_conditioning = None
    if tts_model.valid_cfg_conditionings:
        cfg_coef_conditioning = tts_model.cfg_coef
        tts_model.cfg_coef = 1.0
        cfg_is_no_text = False
        cfg_is_no_prefix = False
    else:
        cfg_is_no_text = True
        cfg_is_no_prefix = True

    if args.inp == "-":
        if sys.stdin.isatty():  # Interactive
            print("Enter text to synthesize (Ctrl+D to end input):")
        text = sys.stdin.read().strip()
    else:
        with open(args.inp, "r", encoding="utf-8") as fobj:
            text = fobj.read().strip()

    # If you want to make a dialog, you can pass more than one turn [text_speaker_1, text_speaker_2, text_2_speaker_1, ...]
    entries = tts_model.prepare_script(
        [text], padding_between=args.padding_between)
    if args.voice.endswith(".safetensors"):
        voice_path = Path(args.voice)
    else:
        start_load_voice = time.perf_counter()
        voice_path = tts_model.get_voice_path(args.voice)
        _sync()
        end_load_voice = time.perf_counter()
    # CFG coef goes here because the model was trained with CFG distillation,
    # so it's not _actually_ doing CFG at inference time.
    # Also, if you are generating a dialog, you should have two voices in the list.
    condition_attributes = tts_model.make_condition_attributes(
        [voice_path], cfg_coef=cfg_coef_conditioning)

    if args.warmup_iters > 0:
        start_warmup = time.perf_counter()
        tts_model.warmup([condition_attributes],
                         iters=args.warmup_iters, batch_size=1)
        _sync()
        end_warmup = time.perf_counter()

    _frames_cnt = 0

    if args.out == "-":
        # Stream the audio to the speakers using sounddevice.
        import sounddevice as sd

        pcms = queue.Queue()

        def _on_frame(frame):
            nonlocal _frames_cnt
            if (frame != -1).all():
                pcm = tts_model.mimi.decode(frame[:, 1:, :]).cpu().numpy()
                pcms.put_nowait(np.clip(pcm[0, 0], -1, 1))
                _frames_cnt += 1
                print(f"generated {_frames_cnt / float(tts_model.mimi.frame_rate):.2f}s",
                      end="\r", flush=True)

        def audio_callback(outdata, _a, _b, _c):
            try:
                pcm_data = pcms.get(block=False)
                outdata[:, 0] = pcm_data
            except queue.Empty:
                outdata[:] = 0

        with sd.OutputStream(
            samplerate=tts_model.mimi.sample_rate,
            blocksize=1920,
            channels=1,
            callback=audio_callback,
        ):
            start_generate = time.perf_counter()
            with tts_model.mimi.streaming(1):
                tts_model.generate(
                    [entries], [condition_attributes],
                    cfg_is_no_prefix=cfg_is_no_prefix, cfg_is_no_text=cfg_is_no_text,
                    on_frame=_on_frame
                )
            _sync()
            end_generate = time.perf_counter()
            time.sleep(3)
            while True:
                if pcms.qsize() == 0:
                    break
                time.sleep(1)

        end_total = time.perf_counter()
        audio_seconds = _frames_cnt / float(tts_model.mimi.frame_rate)
        total_s = end_total - start_total
        gen_s = end_generate - start_generate
        print(
            f"\nMETRICS|audio_s={audio_seconds:.3f}|total_s={total_s:.3f}|gen_s={gen_s:.3f}"
            f"|rtf_total={(audio_seconds / total_s if total_s else 0.0):.3f}"
            f"|rtf_gen={(audio_seconds / gen_s if gen_s else 0.0):.3f}"
        )
        print(f"METRICS|load_checkpoint_s={(end_load_checkpoint - start_load_checkpoint):.3f}"
              f"|load_model_s={(end_load_model - start_load_model):.3f}")
        if "end_load_voice" in locals():
            print(
                f"METRICS|load_voice_s={(end_load_voice - start_load_voice):.3f}")
        if "end_warmup" in locals():
            print(f"METRICS|warmup_s={(end_warmup - start_warmup):.3f}")
        if args.report_memory and str(args.device).startswith("cuda") and torch.cuda.is_available():
            alloc_mb = torch.cuda.max_memory_allocated() / (1024 * 1024)
            reserv_mb = torch.cuda.max_memory_reserved() / (1024 * 1024)
            print(
                f"METRICS|cuda_peak_alloc_mb={alloc_mb:.1f}|cuda_peak_reserved_mb={reserv_mb:.1f}")

        config = {
            "cfg_coef": args.cfg_coef,
            "device": str(args.device),
            "dtype": str(args.dtype),
            "hf_repo": args.hf_repo,
            "metrics_json": bool(args.metrics_json),
            "nq": args.nq,
            "no_compile": bool(args.no_compile),
            "no_cuda_graph": bool(args.no_cuda_graph),
            "padding_between": args.padding_between,
            "seed": args.seed,
            "temp": args.temp,
            "voice": args.voice,
            "voice_repo": args.voice_repo,
            "warmup_iters": args.warmup_iters,
        }
        metrics = {
            "audio_s": float(audio_seconds),
            "cuda_peak_alloc_mb": float(alloc_mb) if args.report_memory and str(args.device).startswith("cuda") and torch.cuda.is_available() else None,
            "cuda_peak_reserved_mb": float(reserv_mb) if args.report_memory and str(args.device).startswith("cuda") and torch.cuda.is_available() else None,
            "decode_s": None,
            "gen_s": float(gen_s),
            "load_checkpoint_s": float(end_load_checkpoint - start_load_checkpoint),
            "load_model_s": float(end_load_model - start_load_model),
            "load_voice_s": float(end_load_voice - start_load_voice) if "end_load_voice" in locals() else None,
            "rtf_gen": float(audio_seconds / gen_s) if gen_s else None,
            "rtf_total": float(audio_seconds / total_s) if total_s else None,
            "total_s": float(total_s),
            "warmup_s": float(end_warmup - start_warmup) if "end_warmup" in locals() else None,
            "write_s": None,
        }
        _emit_metrics(config, metrics)
    else:

        def _on_frame(frame):
            nonlocal _frames_cnt
            if (frame != -1).all():
                _frames_cnt += 1
                print(
                    f"generated {_frames_cnt / float(tts_model.mimi.frame_rate):.2f}s", end="\r", flush=True)

        start_generate = time.perf_counter()
        result = tts_model.generate(
            [entries], [condition_attributes],
            cfg_is_no_prefix=cfg_is_no_prefix, cfg_is_no_text=cfg_is_no_text,
            on_frame=_on_frame
        )
        _sync()
        end_generate = time.perf_counter()

        start_decode = time.perf_counter()
        with tts_model.mimi.streaming(1), torch.no_grad():
            pcms = []
            for frame in result.frames[tts_model.delay_steps:]:
                pcm = tts_model.mimi.decode(frame[:, 1:, :]).cpu().numpy()
                pcms.append(np.clip(pcm[0, 0], -1, 1))
            pcm = np.concatenate(pcms, axis=-1)
        _sync()
        end_decode = time.perf_counter()

        start_write = time.perf_counter()
        sphn.write_wav(args.out, pcm, tts_model.mimi.sample_rate)
        end_write = time.perf_counter()
        end_total = time.perf_counter()

        audio_seconds = pcm.shape[-1] / float(tts_model.mimi.sample_rate)
        total_s = end_total - start_total
        gen_s = end_generate - start_generate
        decode_s = end_decode - start_decode
        write_s = end_write - start_write

        print(
            f"\nMETRICS|audio_s={audio_seconds:.3f}|total_s={total_s:.3f}|gen_s={gen_s:.3f}"
            f"|decode_s={decode_s:.3f}|write_s={write_s:.3f}"
            f"|rtf_total={(audio_seconds / total_s if total_s else 0.0):.3f}"
            f"|rtf_gen={(audio_seconds / gen_s if gen_s else 0.0):.3f}"
        )
        print(f"METRICS|load_checkpoint_s={(end_load_checkpoint - start_load_checkpoint):.3f}"
              f"|load_model_s={(end_load_model - start_load_model):.3f}")
        if "end_load_voice" in locals():
            print(
                f"METRICS|load_voice_s={(end_load_voice - start_load_voice):.3f}")
        if "end_warmup" in locals():
            print(f"METRICS|warmup_s={(end_warmup - start_warmup):.3f}")
        if args.report_memory and str(args.device).startswith("cuda") and torch.cuda.is_available():
            alloc = torch.cuda.max_memory_allocated() / (1024 * 1024)
            reserv = torch.cuda.max_memory_reserved() / (1024 * 1024)
            print(
                f"METRICS|cuda_peak_alloc_mb={alloc:.1f}|cuda_peak_reserved_mb={reserv:.1f}")

        config = {
            "cfg_coef": args.cfg_coef,
            "device": str(args.device),
            "dtype": str(args.dtype),
            "hf_repo": args.hf_repo,
            "metrics_json": bool(args.metrics_json),
            "nq": args.nq,
            "no_compile": bool(args.no_compile),
            "no_cuda_graph": bool(args.no_cuda_graph),
            "padding_between": args.padding_between,
            "seed": args.seed,
            "temp": args.temp,
            "voice": args.voice,
            "voice_repo": args.voice_repo,
            "warmup_iters": args.warmup_iters,
        }
        metrics = {
            "audio_s": float(audio_seconds),
            "cuda_peak_alloc_mb": float(alloc) if args.report_memory and str(args.device).startswith("cuda") and torch.cuda.is_available() else None,
            "cuda_peak_reserved_mb": float(reserv) if args.report_memory and str(args.device).startswith("cuda") and torch.cuda.is_available() else None,
            "decode_s": float(decode_s),
            "gen_s": float(gen_s),
            "load_checkpoint_s": float(end_load_checkpoint - start_load_checkpoint),
            "load_model_s": float(end_load_model - start_load_model),
            "load_voice_s": float(end_load_voice - start_load_voice) if "end_load_voice" in locals() else None,
            "rtf_gen": float(audio_seconds / gen_s) if gen_s else None,
            "rtf_total": float(audio_seconds / total_s) if total_s else None,
            "total_s": float(total_s),
            "warmup_s": float(end_warmup - start_warmup) if "end_warmup" in locals() else None,
            "write_s": float(write_s),
        }
        _emit_metrics(config, metrics)


if __name__ == "__main__":
    main()
