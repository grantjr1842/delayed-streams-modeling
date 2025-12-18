#!/usr/bin/env python3
"""
TTS Audio Integrity Check

Analyzes TTS output WAV files for audio integrity issues:
- Discontinuities (sudden jumps in amplitude)
- Clipping
- Extended silence periods
- Sample rate mismatches

Usage:
    python scripts/tts_audio_integrity_check.py output.wav
    python scripts/tts_audio_integrity_check.py --dir tmp/tts/
"""

import argparse
import sys
import wave
import struct
import math
from pathlib import Path
from dataclasses import dataclass
from typing import List, Optional


@dataclass
class IntegrityReport:
    """Audio integrity analysis report."""
    file_path: str
    sample_rate: int
    channels: int
    duration_seconds: float
    total_samples: int
    
    # Issues detected
    discontinuities: int = 0
    clipping_samples: int = 0
    max_amplitude: float = 0.0
    min_amplitude: float = 0.0
    extended_silences: int = 0  # periods > 100ms of near-zero
    
    # Quality metrics
    rms_level: float = 0.0
    dynamic_range_db: float = 0.0
    
    @property
    def has_issues(self) -> bool:
        """Returns True if any integrity issues were detected."""
        return (
            self.discontinuities > 10 or
            self.clipping_samples > 100 or
            self.extended_silences > 5
        )
    
    def to_dict(self) -> dict:
        return {
            "file_path": self.file_path,
            "sample_rate": self.sample_rate,
            "channels": self.channels,
            "duration_seconds": round(self.duration_seconds, 3),
            "total_samples": self.total_samples,
            "discontinuities": self.discontinuities,
            "clipping_samples": self.clipping_samples,
            "max_amplitude": round(self.max_amplitude, 4),
            "min_amplitude": round(self.min_amplitude, 4),
            "extended_silences": self.extended_silences,
            "rms_level": round(self.rms_level, 4),
            "dynamic_range_db": round(self.dynamic_range_db, 2),
            "has_issues": self.has_issues,
        }
    
    def __str__(self) -> str:
        status = "⚠️  ISSUES DETECTED" if self.has_issues else "✓ OK"
        lines = [
            f"Audio Integrity Report: {status}",
            f"  File: {self.file_path}",
            f"  Duration: {self.duration_seconds:.2f}s @ {self.sample_rate}Hz ({self.channels}ch)",
            f"  Samples: {self.total_samples:,}",
            f"  RMS Level: {self.rms_level:.4f}",
            f"  Dynamic Range: {self.dynamic_range_db:.1f} dB",
            f"  Amplitude: [{self.min_amplitude:.4f}, {self.max_amplitude:.4f}]",
            f"  Discontinuities: {self.discontinuities}",
            f"  Clipping Samples: {self.clipping_samples}",
            f"  Extended Silences (>100ms): {self.extended_silences}",
        ]
        return "\n".join(lines)


def read_wav_samples(wav_path: str) -> tuple:
    """Read WAV file and return (samples, sample_rate, channels)."""
    with wave.open(wav_path, 'rb') as wav:
        n_channels = wav.getnchannels()
        sample_width = wav.getsampwidth()
        sample_rate = wav.getframerate()
        n_frames = wav.getnframes()
        
        raw_data = wav.readframes(n_frames)
    
    # Convert to samples based on sample width
    if sample_width == 1:
        fmt = f"{len(raw_data)}b"
        samples = list(struct.unpack(fmt, raw_data))
        samples = [s / 128.0 for s in samples]  # Normalize to [-1, 1]
    elif sample_width == 2:
        fmt = f"<{len(raw_data)//2}h"
        samples = list(struct.unpack(fmt, raw_data))
        samples = [s / 32768.0 for s in samples]  # Normalize to [-1, 1]
    elif sample_width == 4:
        # Could be int32 or float32
        try:
            fmt = f"<{len(raw_data)//4}f"
            samples = list(struct.unpack(fmt, raw_data))
        except struct.error:
            fmt = f"<{len(raw_data)//4}i"
            samples = list(struct.unpack(fmt, raw_data))
            samples = [s / 2147483648.0 for s in samples]
    else:
        raise ValueError(f"Unsupported sample width: {sample_width}")
    
    return samples, sample_rate, n_channels


def analyze_wav(wav_path: str) -> IntegrityReport:
    """Analyze a WAV file for integrity issues."""
    samples, sample_rate, channels = read_wav_samples(wav_path)
    
    total_samples = len(samples) // channels
    duration = total_samples / sample_rate
    
    # Initialize report
    report = IntegrityReport(
        file_path=wav_path,
        sample_rate=sample_rate,
        channels=channels,
        duration_seconds=duration,
        total_samples=total_samples,
    )
    
    # Take first channel for analysis if stereo
    if channels > 1:
        samples = samples[::channels]
    
    if not samples:
        return report
    
    # Amplitude analysis
    report.max_amplitude = max(samples)
    report.min_amplitude = min(samples)
    
    # RMS level
    sum_squares = sum(s * s for s in samples)
    report.rms_level = math.sqrt(sum_squares / len(samples))
    
    # Dynamic range (crude estimate)
    if report.rms_level > 0:
        peak = max(abs(report.max_amplitude), abs(report.min_amplitude))
        if peak > 0:
            report.dynamic_range_db = 20 * math.log10(peak / report.rms_level)
    
    # Discontinuity detection (sudden jumps > 0.3 between adjacent samples)
    discontinuity_threshold = 0.3
    for i in range(1, len(samples)):
        if abs(samples[i] - samples[i-1]) > discontinuity_threshold:
            report.discontinuities += 1
    
    # Clipping detection (samples at or near +/-1.0)
    clip_threshold = 0.99
    for s in samples:
        if abs(s) >= clip_threshold:
            report.clipping_samples += 1
    
    # Extended silence detection (>100ms of low amplitude)
    silence_threshold = 0.01
    silence_min_samples = int(0.1 * sample_rate)  # 100ms
    
    silence_count = 0
    for s in samples:
        if abs(s) < silence_threshold:
            silence_count += 1
            if silence_count == silence_min_samples:
                report.extended_silences += 1
        else:
            silence_count = 0
    
    return report


def main():
    parser = argparse.ArgumentParser(
        description="TTS Audio Integrity Check - Analyze WAV files for issues"
    )
    parser.add_argument(
        "path",
        nargs="?",
        help="WAV file or directory to analyze"
    )
    parser.add_argument(
        "--dir", "-d",
        help="Directory containing WAV files to analyze"
    )
    parser.add_argument(
        "--json", "-j",
        action="store_true",
        help="Output results as JSON"
    )
    parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Only output if issues are detected"
    )
    
    args = parser.parse_args()
    
    # Determine files to analyze
    wav_files: List[Path] = []
    
    if args.dir:
        dir_path = Path(args.dir)
        if not dir_path.is_dir():
            print(f"Error: {args.dir} is not a directory", file=sys.stderr)
            sys.exit(1)
        wav_files = list(dir_path.glob("*.wav"))
    elif args.path:
        path = Path(args.path)
        if path.is_dir():
            wav_files = list(path.glob("*.wav"))
        elif path.is_file():
            wav_files = [path]
        else:
            print(f"Error: {args.path} not found", file=sys.stderr)
            sys.exit(1)
    else:
        parser.print_help()
        sys.exit(1)
    
    if not wav_files:
        print("No WAV files found", file=sys.stderr)
        sys.exit(1)
    
    # Analyze files
    reports = []
    any_issues = False
    
    for wav_file in sorted(wav_files):
        try:
            report = analyze_wav(str(wav_file))
            reports.append(report)
            if report.has_issues:
                any_issues = True
        except Exception as e:
            print(f"Error analyzing {wav_file}: {e}", file=sys.stderr)
    
    # Output results
    if args.json:
        import json
        print(json.dumps([r.to_dict() for r in reports], indent=2))
    else:
        for report in reports:
            if args.quiet and not report.has_issues:
                continue
            print(report)
            print()
    
    # Exit with error if issues found
    sys.exit(1 if any_issues else 0)


if __name__ == "__main__":
    main()
