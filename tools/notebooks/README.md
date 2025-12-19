# Jupyter Notebooks

This directory contains interactive Jupyter notebooks for exploring and testing Kyutai STT and TTS models.

## Available Notebooks

### stt_pytorch.ipynb
[![Open In Colab](https://colab.research.google.com/assets/colab-badge.svg)](https://colab.research.google.com/github/kyutai-labs/delayed-streams-modeling/blob/main/tools/notebooks/stt_pytorch.ipynb)

Interactive notebook demonstrating how to use the PyTorch STT implementation. Shows:
- Loading the STT model
- Processing audio files
- Extracting word-level timestamps
- Using the semantic VAD

### tts_pytorch.ipynb
[![Open In Colab](https://colab.research.google.com/assets/colab-badge.svg)](https://colab.research.google.com/github/kyutai-labs/delayed-streams-modeling/blob/main/tools/notebooks/tts_pytorch.ipynb)

Interactive notebook demonstrating how to use the PyTorch TTS implementation. Shows:
- Loading the TTS model
- Generating speech from text
- Voice selection and customization
- Streaming audio generation

## Running Locally

To run these notebooks locally:

```bash
# Install Jupyter if you haven't already
uv pip install jupyter

# Start Jupyter
jupyter notebook tools/notebooks/
```

## Running in Colab

Click the "Open in Colab" badges above to run the notebooks directly in Google Colab with GPU support.
