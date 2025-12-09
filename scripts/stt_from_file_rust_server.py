# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "msgpack",
#     "numpy",
#     "sphn",
#     "websockets",
# ]
# ///
import argparse
import asyncio
import contextlib
import time

import msgpack
import numpy as np
import sphn
import websockets

SAMPLE_RATE = 24000
FRAME_SIZE = 1920  # Send data in chunks
WS_CLOSE_REASON = "client finished streaming"
SILENCE_SECOND = [0.0] * SAMPLE_RATE


def load_and_process_audio(file_path):
    """Load an MP3 file, resample to 24kHz, convert to mono, and extract PCM float32 data."""
    pcm_data, _ = sphn.read(file_path, sample_rate=SAMPLE_RATE)
    return pcm_data[0]


async def receive_messages(websocket):
    transcript = []

    async for message in websocket:
        data = msgpack.unpackb(message, raw=False)
        if data["type"] == "Step":
            # This message contains the signal from the semantic VAD, and tells us how
            # much audio the server has already processed. We don't use either here.
            continue
        if data["type"] == "Word":
            print(data["text"], end=" ", flush=True)
            transcript.append(
                {
                    "text": data["text"],
                    "timestamp": [data["start_time"], data["start_time"]],
                }
            )
        if data["type"] == "EndWord":
            if len(transcript) > 0:
                transcript[-1]["timestamp"][1] = data["stop_time"]
        if data["type"] == "Marker":
            # Received marker, stopping stream
            break

    return transcript


async def send_messages(websocket, rtf: float):
    audio_data = load_and_process_audio(args.in_file)

    async def send_audio(audio: np.ndarray | list[float]):
        await websocket.send(
            msgpack.packb(
                {"type": "Audio", "pcm": [float(x) for x in audio]},
                use_single_float=True,
            )
        )

    marker_msg = msgpack.packb({"type": "Marker", "id": 0}, use_single_float=True)
    stream_closed = False

    async def send_stream_end():
        nonlocal stream_closed
        if stream_closed:
            return
        stream_closed = True
        try:
            for _ in range(5):
                await send_audio(SILENCE_SECOND)
            with contextlib.suppress(websockets.ConnectionClosed):
                await websocket.send(marker_msg)
            for _ in range(35):
                await send_audio(SILENCE_SECOND)
        except asyncio.CancelledError:
            # Allow a later shielded retry to flush the trailer.
            stream_closed = False
            raise
        except websockets.ConnectionClosed:
            # Connection already closed; nothing else to send.
            return

    # Start with a second of silence.
    # This is needed for the 2.6B model for technical reasons.
    await send_audio(SILENCE_SECOND)

    start_time = time.time()
    try:
        for i in range(0, len(audio_data), FRAME_SIZE):
            await send_audio(audio_data[i : i + FRAME_SIZE])

            expected_send_time = start_time + (i + 1) / SAMPLE_RATE / rtf
            current_time = time.time()
            if current_time < expected_send_time:
                await asyncio.sleep(expected_send_time - current_time)
            else:
                await asyncio.sleep(0.001)

        await send_stream_end()
    except asyncio.CancelledError:
        await asyncio.shield(send_stream_end())
        return
    finally:
        await asyncio.shield(send_stream_end())


async def _cancel_task(task: asyncio.Task | None) -> None:
    if task is None or task.done():
        return
    task.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await task


async def _close_websocket(websocket) -> None:
    if websocket.closed:
        return
    with contextlib.suppress(websockets.ConnectionClosed):
        await websocket.close(code=1000, reason=WS_CLOSE_REASON)
    with contextlib.suppress(websockets.ConnectionClosed):
        await websocket.wait_closed()


async def _shutdown_session(websocket, *tasks: asyncio.Task) -> None:
    for task in tasks:
        await _cancel_task(task)
    await _close_websocket(websocket)


async def stream_audio(url: str, token: str | None, rtf: float):
    """Stream audio data to a WebSocket server."""
    # Authenticate via Better Auth JWT token in query string
    ws_url = f"{url}?token={token}" if token else url
    async with websockets.connect(ws_url) as websocket:
        send_task = asyncio.create_task(send_messages(websocket, rtf))
        receive_task = asyncio.create_task(receive_messages(websocket))
        cancelled = False
        gather_future = asyncio.gather(
            send_task,
            receive_task,
            return_exceptions=True,
        )
        try:
            send_result, transcript_result = await asyncio.shield(gather_future)
        except asyncio.CancelledError:
            cancelled = True
            send_result, transcript_result = await asyncio.shield(gather_future)
        finally:
            await asyncio.shield(
                _shutdown_session(websocket, send_task, receive_task)
            )

        if isinstance(send_result, asyncio.CancelledError):
            cancelled = True
        elif isinstance(send_result, Exception):
            raise send_result

        if isinstance(transcript_result, asyncio.CancelledError):
            cancelled = True
        elif isinstance(transcript_result, Exception):
            raise transcript_result

        if cancelled:
            raise asyncio.CancelledError

        return transcript_result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("in_file")
    parser.add_argument(
        "--url",
        help="The url of the server to which to send the audio",
        default="ws://127.0.0.1:8080",
    )
    parser.add_argument(
        "--token",
        help="Better Auth JWT token for authentication (get from browser session)",
        default=None,
    )
    parser.add_argument(
        "--rtf",
        type=float,
        default=1.01,
        help="The real-time factor of how fast to feed in the audio.",
    )
    args = parser.parse_args()

    url = f"{args.url}/api/asr-streaming"
    transcript = asyncio.run(stream_audio(url, args.token, args.rtf))

    print()
    print()
    for word in transcript:
        print(
            f"{word['timestamp'][0]:7.2f} -{word['timestamp'][1]:7.2f}  {word['text']}"
        )
