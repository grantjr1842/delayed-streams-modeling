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
import signal

import msgpack
import numpy as np
import sounddevice as sd
import websockets

SAMPLE_RATE = 24000

# The VAD has several prediction heads, each of which tries to determine whether there
# has been a pause of a given length. The lengths are 0.5, 1.0, 2.0, and 3.0 seconds.
# Lower indices predict pauses more aggressively. In Unmute, we use 2.0 seconds = index 2.
PAUSE_PREDICTION_HEAD_INDEX = 2


async def receive_messages(websocket, show_vad: bool = False):
    """Receive and process messages from the WebSocket server."""
    try:
        speech_started = False
        async for message in websocket:
            data = msgpack.unpackb(message, raw=False)

            # The Step message only gets sent if the model has semantic VAD available
            if data["type"] == "Step" and show_vad:
                pause_prediction = data["prs"][PAUSE_PREDICTION_HEAD_INDEX]
                if pause_prediction > 0.5 and speech_started:
                    print("| ", end="", flush=True)
                    speech_started = False

            elif data["type"] == "Word":
                print(data["text"], end=" ", flush=True)
                speech_started = True
    except websockets.ConnectionClosed:
        print("Connection closed while receiving messages.")


def _level_dbfs(audio: np.ndarray) -> float:
    rms = np.sqrt(np.mean(np.square(audio)))
    if rms <= 1e-9:
        return -120.0
    return 20 * np.log10(rms)


def _render_meter(db: float) -> str:
    db = max(-60.0, min(0.0, db))
    filled = int((db + 60.0) / 60.0 * 20)
    bar = "#" * filled + "-" * (20 - filled)
    return f"[{bar}] {db:6.1f} dBFS"


async def send_messages(websocket, audio_queue, meter: bool):
    """Send audio data from microphone to WebSocket server."""
    try:
        # Start by draining the queue to avoid lags
        while not audio_queue.empty():
            await audio_queue.get()

        print("Starting the transcription")

        while True:
            audio_data = await audio_queue.get()
            if meter:
                db = _level_dbfs(audio_data)
                print("\r" + _render_meter(db), end="", flush=True)

            chunk = {"type": "Audio", "pcm": [float(x) for x in audio_data]}
            msg = msgpack.packb(chunk, use_bin_type=True, use_single_float=True)
            await websocket.send(msg)

    except websockets.ConnectionClosed:
        print("Connection closed while sending messages.")


def _resolve_input_device(device_arg: str | None, device_name: str | None) -> int | None:
    """Return an input device index from an integer or fuzzy-matched name."""
    if device_arg is not None:
        try:
            return int(device_arg)
        except ValueError:
            device_name = device_arg

    if device_name:
        wanted = device_name.lower()
        for idx, info in enumerate(sd.query_devices()):
            if wanted in info["name"].lower():
                return idx
        raise ValueError(f"Could not find input device containing '{device_name}'")

    return None


async def stream_audio(
    url: str, api_key: str, show_vad: bool, meter: bool, device: int | None
):
    """Stream audio data to a WebSocket server."""
    print("Starting microphone recording...")
    print("Press Ctrl+C to stop recording")
    audio_queue = asyncio.Queue()

    loop = asyncio.get_event_loop()

    def audio_callback(indata, frames, time, status):
        loop.call_soon_threadsafe(
            audio_queue.put_nowait, indata[:, 0].astype(np.float32).copy()
        )

    # Start audio stream
    with sd.InputStream(
        samplerate=SAMPLE_RATE,
        channels=1,
        dtype="float32",
        callback=audio_callback,
        blocksize=1920,  # 80ms blocks
        device=device,
    ):
        headers = {"kyutai-api-key": api_key}
        # Instead of using the header, you can authenticate by adding `?auth_id={api_key}` to the URL
        async with websockets.connect(url, additional_headers=headers) as websocket:
            send_task = asyncio.create_task(send_messages(websocket, audio_queue, meter))
            receive_task = asyncio.create_task(
                receive_messages(websocket, show_vad=show_vad)
            )
            await asyncio.gather(send_task, receive_task)
            if meter:
                print()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Real-time microphone transcription")
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
        "--device",
        help="Input device index or name (use --list-devices to see options)",
    )
    parser.add_argument(
        "--device-name",
        help="Alternative way to select the input device using a substring match",
    )
    parser.add_argument(
        "--show-vad",
        action="store_true",
        help="Visualize the predictions of the semantic voice activity detector with a '|' symbol",
    )
    parser.add_argument(
        "--meter",
        action="store_true",
        help="Display a simple level meter for the incoming microphone audio",
    )

    args = parser.parse_args()

    def handle_sigint(signum, frame):
        print("Interrupted by user")  # Don't complain about KeyboardInterrupt
        exit(0)

    signal.signal(signal.SIGINT, handle_sigint)

    if args.list_devices:
        print("Available audio devices (index: name - (max input channels)):")
        for idx, info in enumerate(sd.query_devices()):
            print(f"  {idx:>3}: {info['name']} (in={info['max_input_channels']}, out={info['max_output_channels']})")
        exit(0)

    try:
        device_index = _resolve_input_device(args.device, args.device_name)
    except ValueError as err:
        print(err)
        exit(1)

    url = f"{args.url}/api/asr-streaming"
    if args.meter:
        print("Level meter enabled (updates every 80 ms)")

    asyncio.run(
        stream_audio(url, args.api_key, args.show_vad, args.meter, device_index)
    )
