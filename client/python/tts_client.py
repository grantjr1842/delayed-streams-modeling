# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "msgpack",
#     "numpy",
#     "sphn",
#     "websockets",
#     "sounddevice",
#     "tqdm",
#     "PyJWT",
#     "python-dotenv",
# ]
# ///
import argparse
import asyncio
from datetime import datetime, timedelta, timezone
import os
import sys
import time
from urllib.parse import urlencode

from dotenv import load_dotenv
import jwt
import msgpack
import numpy as np
import sphn
import tqdm
import websockets

# Load environment variables from .env file
load_dotenv()

SAMPLE_RATE = 24000

TTS_TEXT = "Hello, this is a test of the moshi text to speech system, this should result in some nicely sounding generated voice."
DEFAULT_DSM_TTS_VOICE_REPO = "kyutai/tts-voices"


def generate_jwt_token(secret: str, user_id: str = "test-user", expiry_hours: int = 24) -> str:
    """
    Generate a JWT token for Better Auth authentication.
    Matches the BetterAuthClaims structure from moshi-server/src/auth.rs
    
    Args:
        secret: The BETTER_AUTH_SECRET from environment
        user_id: User ID to include in the token (default: "test-user")
        expiry_hours: Token validity period in hours (default: 24)
    
    Returns:
        JWT token string
    """
    now_dt = datetime.now(timezone.utc)
    exp_dt = now_dt + timedelta(hours=expiry_hours)
    
    # Match the BetterAuthClaims structure from moshi-server/src/auth.rs
    payload = {
        "session": {
            "id": f"test-session-{int(time.time())}",
            "userId": user_id,
            "createdAt": now_dt.isoformat(),
            "updatedAt": now_dt.isoformat(),
            "expiresAt": exp_dt.isoformat(),
            "token": "test-session-token",
            "ipAddress": "127.0.0.1",
            "userAgent": "tts-rust-client/0.1.0",
        },
        "user": {
            "id": user_id,
            "name": "Test User",
            "email": "test@example.com",
            "emailVerified": False,
            "image": None,
        },
        "iat": int(now_dt.timestamp()),
        "exp": int(exp_dt.timestamp()),
    }
    
    token = jwt.encode(payload, secret, algorithm="HS256")
    return token


async def receive_messages(websocket: websockets.ClientConnection, output_queue):
    with tqdm.tqdm(desc="Receiving audio", unit=" seconds generated") as pbar:
        accumulated_samples = 0
        last_seconds = 0

        async for message_bytes in websocket:
            msg = msgpack.unpackb(message_bytes, raw=False)

            if msg["type"] == "Audio":
                pcm = np.array(msg["pcm"]).astype(np.float32)
                await output_queue.put(pcm)

                accumulated_samples += len(msg["pcm"])
                current_seconds = accumulated_samples // SAMPLE_RATE
                if current_seconds > last_seconds:
                    pbar.update(current_seconds - last_seconds)
                    last_seconds = current_seconds

    print("End of audio.")
    await output_queue.put(None)  # Signal end of audio


async def output_audio(out: str, output_queue: asyncio.Queue[np.ndarray | None]):
    if out == "-":
        # This will fail with "OSError: PortAudio library not found" on servers with no
        # audio output, so only import if the user requests it.
        import sounddevice as sd

        should_exit = False

        def audio_callback(outdata, _a, _b, _c):
            nonlocal should_exit

            try:
                pcm_data = output_queue.get_nowait()
                if pcm_data is not None:
                    outdata[:, 0] = pcm_data
                else:
                    should_exit = True
                    outdata[:] = 0
            except asyncio.QueueEmpty:
                outdata[:] = 0

        with sd.OutputStream(
            samplerate=SAMPLE_RATE,
            blocksize=1920,
            channels=1,
            callback=audio_callback,
        ):
            while True:
                if should_exit:
                    break
                await asyncio.sleep(1)
    else:
        frames = []
        while True:
            item = await output_queue.get()
            if item is None:
                break
            frames.append(item)

        sphn.write_wav(out, np.concatenate(frames, -1), SAMPLE_RATE)
        print(f"Saved audio to {out}")


async def read_lines_from_stdin():
    reader = asyncio.StreamReader()
    protocol = asyncio.StreamReaderProtocol(reader)
    loop = asyncio.get_running_loop()
    await loop.connect_read_pipe(lambda: protocol, sys.stdin)
    while True:
        line = await reader.readline()
        if not line:
            break
        yield line.decode().rstrip()


async def read_lines_from_file(path: str):
    queue = asyncio.Queue()
    loop = asyncio.get_running_loop()

    def producer():
        with open(path, "r", encoding="utf-8") as f:
            for line in f:
                asyncio.run_coroutine_threadsafe(queue.put(line), loop)
        asyncio.run_coroutine_threadsafe(queue.put(None), loop)

    await asyncio.to_thread(producer)
    while True:
        line = await queue.get()
        if line is None:
            break
        yield line


async def get_lines(source: str):
    if source == "-":
        async for line in read_lines_from_stdin():
            yield line
    else:
        async for line in read_lines_from_file(source):
            yield line


async def websocket_client():
    parser = argparse.ArgumentParser(description="Use the TTS streaming API")
    parser.add_argument("inp", type=str, help="Input file, use - for stdin.")
    parser.add_argument(
        "out", type=str, help="Output file to generate, use - for playing the audio"
    )
    parser.add_argument(
        "--voice",
        default="expresso/ex03-ex01_happy_001_channel1_334s.wav",
        help="The voice to use, relative to the voice repo root. "
        f"See {DEFAULT_DSM_TTS_VOICE_REPO}",
    )
    parser.add_argument(
        "--url",
        help="The URL of the server to which to send the audio",
        default="ws://127.0.0.1:8080",
    )
    parser.add_argument(
        "--token",
        help="Better Auth JWT token for authentication (get from browser session or auto-generated from BETTER_AUTH_SECRET)",
        default=None,
    )
    args = parser.parse_args()

    # Auto-generate token from BETTER_AUTH_SECRET if not provided
    token = args.token
    if not token:
        secret = os.getenv("BETTER_AUTH_SECRET")
        if secret:
            token = generate_jwt_token(secret)
            print(f"Generated JWT token from BETTER_AUTH_SECRET")
        else:
            print("Note: No token provided and BETTER_AUTH_SECRET not set. Authentication may fail if server requires auth.")

    params = {"voice": args.voice, "format": "PcmMessagePack"}
    # Add token to query params if available for Better Auth JWT authentication
    if token:
        params["token"] = token
    uri = f"{args.url}/api/tts_streaming?{urlencode(params)}"
    print(uri)

    if args.inp == "-":
        if sys.stdin.isatty():  # Interactive
            print("Enter text to synthesize (Ctrl+D to end input):")

    async with websockets.connect(uri) as websocket:
        print("connected")

        async def send_loop():
            print("go send")
            async for line in get_lines(args.inp):
                for word in line.split():
                    await websocket.send(word)
            await websocket.send(b"\0")

        output_queue = asyncio.Queue()
        receive_task = asyncio.create_task(
            receive_messages(websocket, output_queue))
        output_audio_task = asyncio.create_task(
            output_audio(args.out, output_queue))
        send_task = asyncio.create_task(send_loop())
        await asyncio.gather(receive_task, output_audio_task, send_task)


if __name__ == "__main__":
    asyncio.run(websocket_client())
