# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "msgpack",
#     "numpy",
#     "sounddevice",
#     "websockets",
# ]
# ///
import argparse
import asyncio
import contextlib
import signal

from typing import Any, AsyncIterator, Callable, Protocol, cast

import msgpack as _msgpack  # type: ignore[reportMissingTypeStubs]
import numpy as np
import numpy.typing as npt
import sounddevice as _sounddevice  # type: ignore[reportMissingTypeStubs]
import websockets
from websockets.exceptions import ConnectionClosed


class MsgpackModule(Protocol):
    def packb(self, obj: Any, **kwargs: Any) -> bytes:
        ...

    def unpackb(self, obj: bytes | memoryview, **kwargs: Any) -> Any:
        ...


class CallbackFlags:
    ...


class SoundDeviceInputStream(Protocol):
    def __enter__(self) -> "SoundDeviceInputStream":
        ...

    def __exit__(
        self, exc_type: Any, exc_value: Any, exc_traceback: Any
    ) -> bool | None:
        ...


class SoundDeviceDefault(Protocol):
    device: list[int | None]


class SoundDeviceModule(Protocol):
    CallbackFlags: type[CallbackFlags]
    default: SoundDeviceDefault
    InputStream: Callable[..., SoundDeviceInputStream]
    def query_devices(
        self, device: int | None = None, kind: str | None = None
    ) -> list[dict[str, Any]]:
        ...


class WebSocketClientProtocol(Protocol):
    closed: bool

    async def send(self, message: bytes | Any) -> None:
        ...

    async def close(self, code: int = 1000, reason: str = "") -> None:
        ...

    async def wait_closed(self) -> None:
        ...

    def __aiter__(self) -> AsyncIterator[Any]:
        ...


msgpack: MsgpackModule = cast(MsgpackModule, _msgpack)
sd: SoundDeviceModule = cast(SoundDeviceModule, _sounddevice)

SAMPLE_RATE = 24000
FRAME_SIZE = 1920
WS_CLOSE_REASON = "client finished streaming"
SILENCE_SECOND = [0.0] * SAMPLE_RATE

# The VAD has several prediction heads, each of which tries to determine whether there
# has been a pause of a given length. The lengths are 0.5, 1.0, 2.0, and 3.0 seconds.
# Lower indices predict pauses more aggressively. In Unmute, we use 2.0 seconds = index 2.
PAUSE_PREDICTION_HEAD_INDEX = 2


AudioChunk = npt.NDArray[np.float32]
AudioQueue = asyncio.Queue[AudioChunk | None]


async def receive_messages(
    websocket: WebSocketClientProtocol,
    show_vad: bool = False,
) -> None:
    """Receive and process messages from the WebSocket server."""
    try:
        speech_started = False
        async for message in websocket:
            if not isinstance(message, (bytes, bytearray, memoryview)):
                continue
            data = cast(dict[str, Any], msgpack.unpackb(message, raw=False))

            # The Step message only gets sent if the model has semantic VAD available
            if data["type"] == "Step" and show_vad:
                prs = cast(list[float], data["prs"])
                pause_prediction: float = float(
                    prs[PAUSE_PREDICTION_HEAD_INDEX]
                )
                if pause_prediction > 0.5 and speech_started:
                    print("| ", end="", flush=True)
                    speech_started = False

            elif data["type"] == "Word":
                print(data["text"], end=" ", flush=True)
                speech_started = True
            elif data["type"] == "Marker":
                print("\nServer reached the end of the stream.")
                break
    except ConnectionClosed:
        print("Connection closed while receiving messages.")


async def send_messages(
    websocket: WebSocketClientProtocol,
    audio_queue: AudioQueue,
    stop_event: asyncio.Event,
) -> None:
    """Send audio data from microphone to WebSocket server."""
    finalized = False

    async def send_audio(audio: AudioChunk | list[float]) -> None:
        await websocket.send(
            msgpack.packb(
                {"type": "Audio", "pcm": [float(x) for x in audio]},
                use_bin_type=True,
                use_single_float=True,
            )
        )

    marker_msg: bytes = msgpack.packb(
        {"type": "Marker", "id": 0}, use_single_float=True)

    async def finish_stream() -> None:
        nonlocal finalized
        if finalized:
            return
        finalized = True
        try:
            for _ in range(5):
                await send_audio(SILENCE_SECOND)
            with contextlib.suppress(ConnectionClosed):
                await websocket.send(marker_msg)
            for _ in range(35):
                await send_audio(SILENCE_SECOND)
        except ConnectionClosed:
            return

    try:
        # Start by draining the queue to avoid lags
        while not audio_queue.empty():
            await audio_queue.get()

        print("Starting the transcription")

        while True:
            audio_data = await audio_queue.get()
            if audio_data is None:
                break
            await send_audio(audio_data)
            if stop_event.is_set():
                while True:
                    try:
                        audio_queue.get_nowait()
                    except asyncio.QueueEmpty:
                        break
                break

    except ConnectionClosed:
        if not stop_event.is_set():
            print("Connection closed while sending messages.")
    except asyncio.CancelledError:
        await asyncio.shield(finish_stream())
        raise
    finally:
        await asyncio.shield(finish_stream())


async def _cancel_task(task: asyncio.Task[Any] | None) -> None:
    if task is None or task.done():
        return
    task.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await task


async def _close_websocket(websocket: WebSocketClientProtocol) -> None:
    if websocket.closed:
        return
    with contextlib.suppress(ConnectionClosed):
        await websocket.close(code=1000, reason=WS_CLOSE_REASON)
    with contextlib.suppress(ConnectionClosed):
        await websocket.wait_closed()


async def _shutdown_session(
    websocket: WebSocketClientProtocol, *tasks: asyncio.Task[Any]
) -> None:
    for task in tasks:
        await _cancel_task(task)
    await _close_websocket(websocket)


async def stream_audio(url: str, api_key: str, show_vad: bool) -> None:
    """Stream audio data to a WebSocket server."""
    print("Starting microphone recording...")
    print("Press Ctrl+C to stop recording")
    audio_queue: AudioQueue = asyncio.Queue()
    stop_event = asyncio.Event()

    loop = asyncio.get_event_loop()

    def audio_callback(
        indata: AudioChunk,
        frames: int,
        time: Any,
        status: CallbackFlags,
    ) -> None:
        if stop_event.is_set():
            return
        loop.call_soon_threadsafe(
            audio_queue.put_nowait, indata[:, 0].astype(np.float32).copy()
        )

    def signal_handler(signum: int, frame: Any) -> None:
        if stop_event.is_set():
            print("Force exiting the transcription loop.")
            raise KeyboardInterrupt
        print("Stopping the transcription...")
        loop.call_soon_threadsafe(stop_event.set)
        loop.call_soon_threadsafe(audio_queue.put_nowait, None)

    previous_handler = signal.getsignal(signal.SIGINT)
    signal.signal(signal.SIGINT, signal_handler)

    try:
        # Start audio stream
        with sd.InputStream(
            samplerate=SAMPLE_RATE,
            channels=1,
            dtype="float32",
            callback=audio_callback,
            blocksize=FRAME_SIZE,  # 80ms blocks
        ):
            headers = {"kyutai-api-key": api_key}
            # Instead of using the header, you can authenticate by adding `?auth_id={api_key}` to the URL
            async with websockets.connect(url, additional_headers=headers) as websocket:
                send_task = asyncio.create_task(
                    send_messages(websocket, audio_queue, stop_event)
                )
                receive_task = asyncio.create_task(
                    receive_messages(
                        websocket, show_vad=show_vad
                    )
                )
                try:
                    await asyncio.gather(send_task, receive_task)
                finally:
                    await asyncio.shield(
                        _shutdown_session(websocket, send_task, receive_task)
                    )
    finally:
        signal.signal(signal.SIGINT, previous_handler)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Real-time microphone transcription")
    parser.add_argument(
        "--url",
        help="The URL of the server to which to send the audio",
        default="ws://127.0.0.1:8080",
    )
    parser.add_argument("--api-key", default="public_token")
    parser.add_argument(
        "--list-devices", action="store_true", help="List available audio devices"
    )
    parser.add_argument(
        "--device", type=int, help="Input device ID (use --list-devices to see options)"
    )
    parser.add_argument(
        "--show-vad",
        action="store_true",
        help="Visualize the predictions of the semantic voice activity detector with a '|' symbol",
    )

    args = parser.parse_args()

    if args.list_devices:
        print("Available audio devices:")
        print(sd.query_devices())
        exit(0)

    if args.device is not None:
        sd.default.device[0] = args.device  # Set input device

    url = f"{args.url}/api/asr-streaming"
    asyncio.run(stream_audio(url, args.api_key, args.show_vad))
